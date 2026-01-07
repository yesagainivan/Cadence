use crate::{
    parser::ast::{Expression, Statement, Value},
    types::{Chord, CommonProgressions, Note},
};
// use crate::types::{chord::Chord, note::Note};
use anyhow::{anyhow, Result};
use std::cell::RefCell;
use std::collections::HashSet;

// Thread-local set to track variables currently being evaluated (for cycle detection)
thread_local! {
    static EVALUATING_VARS: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// Evaluates parsed expressions into values
pub struct Evaluator;

impl Evaluator {
    /// Create a new evaluator
    pub fn new() -> Self {
        Evaluator
    }

    /// Evaluate an expression and return the result (no environment)
    pub fn eval(&self, expr: Expression) -> Result<Value> {
        self.eval_with_env(expr, None)
    }

    /// Evaluate an expression with optional environment for variable resolution
    pub fn eval_with_env(
        &self,
        expr: Expression,
        env: Option<&crate::parser::environment::Environment>,
    ) -> Result<Value> {
        match expr {
            Expression::Note(note) => Ok(Value::Note(note)),
            Expression::Chord(chord) => Ok(Value::Chord(chord)),
            Expression::Pattern(pattern) => {
                // Resolve any variable references in the pattern
                if pattern.has_variables() {
                    if let Some(environment) = env {
                        let resolved = pattern.resolve_variables_with(|name| {
                            // Look up the variable in the environment
                            if let Some(value) = environment.get(name) {
                                // Convert Value to PatternStep(s)
                                value_to_pattern_steps(value)
                            } else {
                                None
                            }
                        })?;
                        Ok(Value::Pattern(resolved))
                    } else {
                        // No environment, can't resolve variables
                        let vars = pattern.get_variable_names();
                        Err(anyhow!("Pattern contains unresolved variables: {:?}", vars))
                    }
                } else {
                    Ok(Value::Pattern(pattern))
                }
            }
            Expression::String(s) => Ok(Value::String(s)),
            Expression::Number(n) => Ok(Value::Number(n)),
            Expression::Transpose { target, semitones } => {
                let target_value = self.eval_with_env(*target, env)?;
                match target_value {
                    Value::Note(note) => {
                        let transposed = note + semitones;
                        Ok(Value::Note(transposed))
                    }
                    Value::Chord(chord) => {
                        let transposed = chord + semitones;
                        Ok(Value::Chord(transposed))
                    }
                    Value::Pattern(pattern) => {
                        let transposed = pattern + semitones;
                        Ok(Value::Pattern(transposed))
                    }
                    Value::Boolean(_) => Err(anyhow!("Cannot transpose a boolean value")),
                    Value::Number(n) => {
                        // Numeric addition: n + semitones
                        Ok(Value::Number(n + semitones as i32))
                    }
                    Value::String(_) => Err(anyhow!("Cannot transpose a string")),
                    Value::Function { .. } => Err(anyhow!("Cannot transpose a function")),
                    Value::Unit => Err(anyhow!("Cannot transpose unit")),
                    Value::Array(_) => Err(anyhow!("Cannot transpose an array")),
                    Value::EveryPattern(every) => {
                        // Transpose both the base and transformed patterns
                        use crate::types::EveryPattern;
                        let transposed = EveryPattern::new(
                            every.interval,
                            every.base.clone() + semitones,
                            every.transformed.clone() + semitones,
                        );
                        Ok(Value::EveryPattern(Box::new(transposed)))
                    }
                    Value::Thunk {
                        expression,
                        env: thunk_env,
                    } => {
                        // Evaluate thunk first, then transpose the result
                        let env_guard = thunk_env.read().map_err(|e| anyhow!("{}", e))?;
                        let resolved = self.eval_with_env(*expression, Some(&env_guard))?;
                        // Recursively transpose the resolved value
                        self.eval_with_env(
                            Expression::Transpose {
                                target: Box::new(Expression::Value(Box::new(resolved))),
                                semitones,
                            },
                            env,
                        )
                    }
                }
            }
            Expression::Intersection { left, right } => {
                let left_value = self.eval_with_env(*left, env)?;
                let right_value = self.eval_with_env(*right, env)?;

                match (left_value, right_value) {
                    (Value::Chord(left_chord), Value::Chord(right_chord)) => {
                        let intersection = left_chord & right_chord;
                        Ok(Value::Chord(intersection))
                    }
                    _ => Err(anyhow!("Intersection only supported between chords")),
                }
            }
            Expression::Union { left, right } => {
                let left_value = self.eval_with_env(*left, env)?;
                let right_value = self.eval_with_env(*right, env)?;

                match (left_value, right_value) {
                    (Value::Chord(left_chord), Value::Chord(right_chord)) => {
                        let union = left_chord | right_chord;
                        Ok(Value::Chord(union))
                    }
                    _ => Err(anyhow!("Union only supported between chords")),
                }
            }
            Expression::Difference { left, right } => {
                let left_value = self.eval_with_env(*left, env)?;
                let right_value = self.eval_with_env(*right, env)?;

                match (left_value, right_value) {
                    (Value::Chord(left_chord), Value::Chord(right_chord)) => {
                        let difference = left_chord ^ right_chord;
                        Ok(Value::Chord(difference))
                    }
                    _ => Err(anyhow!("Difference only supported between chords")),
                }
            }
            Expression::FunctionCall { name, args } => {
                self.eval_function_with_env(&name, args, env)
            }
            Expression::Variable(name) => match env {
                Some(e) => {
                    match e.get(&name).cloned() {
                        Some(Value::Thunk {
                            expression,
                            env: thunk_env,
                        }) => {
                            // Check for recursive evaluation (e.g., let x = x)
                            let is_recursive =
                                EVALUATING_VARS.with(|ev| ev.borrow().contains(&name));

                            if is_recursive {
                                return Err(anyhow!(
                                    "Recursive variable definition: '{}' references itself",
                                    name
                                ));
                            }

                            // Mark variable as being evaluated
                            EVALUATING_VARS.with(|ev| {
                                ev.borrow_mut().insert(name.clone());
                            });

                            // Re-evaluate thunk with its captured environment
                            let env_guard = thunk_env.read().map_err(|e| anyhow!("{}", e))?;
                            let result = self.eval_with_env(*expression, Some(&env_guard));

                            // Remove from evaluation stack (even on error)
                            EVALUATING_VARS.with(|ev| {
                                ev.borrow_mut().remove(&name);
                            });

                            result
                        }
                        Some(v) => Ok(v),
                        None => Err(anyhow!("Variable '{}' is not defined", name)),
                    }
                }
                None => Err(anyhow!(
                    "Variable '{}' cannot be resolved (no environment)",
                    name
                )),
            },
            Expression::Boolean(b) => Ok(Value::Boolean(b)),
            Expression::Comparison {
                left,
                right,
                operator,
            } => {
                let left_val = self.eval_with_env(*left, env)?;
                let right_val = self.eval_with_env(*right, env)?;

                // For numeric comparisons, extract numbers
                let result = match operator {
                    crate::parser::ast::ComparisonOp::Equal => left_val == right_val,
                    crate::parser::ast::ComparisonOp::NotEqual => left_val != right_val,
                    crate::parser::ast::ComparisonOp::Less
                    | crate::parser::ast::ComparisonOp::Greater
                    | crate::parser::ast::ComparisonOp::LessEqual
                    | crate::parser::ast::ComparisonOp::GreaterEqual => {
                        // Extract numeric values
                        let left_num = match &left_val {
                            Value::Number(n) => *n as f32,
                            _ => {
                                return Err(anyhow!(
                                    "Comparison requires numeric values, got {:?}",
                                    left_val
                                ))
                            }
                        };
                        let right_num = match &right_val {
                            Value::Number(n) => *n as f32,
                            _ => {
                                return Err(anyhow!(
                                    "Comparison requires numeric values, got {:?}",
                                    right_val
                                ))
                            }
                        };

                        match operator {
                            crate::parser::ast::ComparisonOp::Less => left_num < right_num,
                            crate::parser::ast::ComparisonOp::Greater => left_num > right_num,
                            crate::parser::ast::ComparisonOp::LessEqual => left_num <= right_num,
                            crate::parser::ast::ComparisonOp::GreaterEqual => left_num >= right_num,
                            _ => unreachable!(),
                        }
                    }
                };
                Ok(Value::Boolean(result))
            }

            // Logical AND with short-circuit evaluation
            Expression::LogicalAnd { left, right } => {
                let left_val = self.eval_with_env(*left, env)?;
                match left_val {
                    Value::Boolean(false) => Ok(Value::Boolean(false)), // Short-circuit
                    Value::Boolean(true) => {
                        let right_val = self.eval_with_env(*right, env)?;
                        match right_val {
                            Value::Boolean(b) => Ok(Value::Boolean(b)),
                            _ => Err(anyhow!(
                                "Logical AND requires boolean values, got {:?}",
                                right_val
                            )),
                        }
                    }
                    _ => Err(anyhow!(
                        "Logical AND requires boolean values, got {:?}",
                        left_val
                    )),
                }
            }

            // Logical OR with short-circuit evaluation
            Expression::LogicalOr { left, right } => {
                let left_val = self.eval_with_env(*left, env)?;
                match left_val {
                    Value::Boolean(true) => Ok(Value::Boolean(true)), // Short-circuit
                    Value::Boolean(false) => {
                        let right_val = self.eval_with_env(*right, env)?;
                        match right_val {
                            Value::Boolean(b) => Ok(Value::Boolean(b)),
                            _ => Err(anyhow!(
                                "Logical OR requires boolean values, got {:?}",
                                right_val
                            )),
                        }
                    }
                    _ => Err(anyhow!(
                        "Logical OR requires boolean values, got {:?}",
                        left_val
                    )),
                }
            }

            // Logical NOT
            Expression::LogicalNot(expr) => {
                let val = self.eval_with_env(*expr, env)?;
                match val {
                    Value::Boolean(b) => Ok(Value::Boolean(!b)),
                    _ => Err(anyhow!("Logical NOT requires boolean value, got {:?}", val)),
                }
            }

            // Index operation: pattern[0], chord[1], array[-1]
            Expression::Index { target, index } => {
                let target_val = self.eval_with_env(*target, env)?;
                let index_val = self.eval_with_env(*index, env)?;

                let idx = match index_val {
                    Value::Number(n) => n,
                    _ => return Err(anyhow!("Index must be a number, got {:?}", index_val)),
                };

                match target_val {
                    Value::Pattern(pattern) => {
                        let len = pattern.steps.len() as i32;
                        if len == 0 {
                            return Err(anyhow!("Cannot index into empty pattern"));
                        }
                        // Handle negative indices (from end)
                        let actual_idx = if idx < 0 { len + idx } else { idx };
                        if actual_idx < 0 || actual_idx >= len {
                            return Err(anyhow!(
                                "Index {} out of bounds for pattern with {} steps",
                                idx,
                                len
                            ));
                        }
                        // Return the step at index as appropriate Value
                        use crate::types::PatternStep;
                        fn step_to_value(step: &PatternStep) -> Result<Value> {
                            match step {
                                PatternStep::Note(n) => Ok(Value::Note(*n)),
                                PatternStep::Chord(c) => Ok(Value::Chord(c.clone())),
                                PatternStep::Rest => {
                                    // Return a pattern with just a rest
                                    Ok(Value::Pattern(crate::types::Pattern::with_steps(vec![
                                        PatternStep::Rest,
                                    ])))
                                }
                                PatternStep::Drum(d) => {
                                    Ok(Value::String(d.short_name().to_string()))
                                }
                                PatternStep::Variable(_) => {
                                    Err(anyhow!("Cannot index unresolved variable"))
                                }
                                PatternStep::Group(steps) => {
                                    // Return as pattern containing the group
                                    Ok(Value::Pattern(crate::types::Pattern::with_steps(
                                        steps.clone(),
                                    )))
                                }
                                PatternStep::Repeat(inner, count) => {
                                    // Return as pattern containing the repeat
                                    Ok(Value::Pattern(crate::types::Pattern::with_steps(vec![
                                        PatternStep::Repeat(inner.clone(), *count),
                                    ])))
                                }
                                PatternStep::Weighted(inner, _) => {
                                    // Unwrap weighted step and return its value
                                    step_to_value(inner)
                                }
                                PatternStep::Alternation(steps) => {
                                    // Return as pattern containing the alternation steps
                                    Ok(Value::Pattern(crate::types::Pattern::with_steps(
                                        steps.clone(),
                                    )))
                                }
                                PatternStep::Euclidean(inner, pulses, steps) => {
                                    // Return as pattern containing the euclidean step
                                    Ok(Value::Pattern(crate::types::Pattern::with_steps(vec![
                                        PatternStep::Euclidean(inner.clone(), *pulses, *steps),
                                    ])))
                                }
                            }
                        }
                        step_to_value(&pattern.steps[actual_idx as usize])
                    }
                    Value::Chord(chord) => {
                        let notes = chord.notes_vec();
                        let len = notes.len() as i32;
                        if len == 0 {
                            return Err(anyhow!("Cannot index into empty chord"));
                        }
                        let actual_idx = if idx < 0 { len + idx } else { idx };
                        if actual_idx < 0 || actual_idx >= len {
                            return Err(anyhow!(
                                "Index {} out of bounds for chord with {} notes",
                                idx,
                                len
                            ));
                        }
                        Ok(Value::Note(notes[actual_idx as usize].clone()))
                    }
                    Value::Array(arr) => {
                        let len = arr.len() as i32;
                        if len == 0 {
                            return Err(anyhow!("Cannot index into empty array"));
                        }
                        let actual_idx = if idx < 0 { len + idx } else { idx };
                        if actual_idx < 0 || actual_idx >= len {
                            return Err(anyhow!(
                                "Index {} out of bounds for array with {} elements",
                                idx,
                                len
                            ));
                        }
                        Ok(arr[actual_idx as usize].clone())
                    }
                    Value::String(s) => {
                        // Allow indexing into strings to get char (as string)
                        let chars: Vec<char> = s.chars().collect();
                        let len = chars.len() as i32;
                        if len == 0 {
                            return Err(anyhow!("Cannot index into empty string"));
                        }
                        let actual_idx = if idx < 0 { len + idx } else { idx };
                        if actual_idx < 0 || actual_idx >= len {
                            return Err(anyhow!(
                                "Index {} out of bounds for string with {} chars",
                                idx,
                                len
                            ));
                        }
                        Ok(Value::String(chars[actual_idx as usize].to_string()))
                    }
                    _ => Err(anyhow!(
                        "Cannot index into {:?} - only Pattern, Chord, Array, or String supported",
                        target_val
                    )),
                }
            }

            // Binary arithmetic operations: +, -, *, /, %
            Expression::BinaryOp {
                left,
                right,
                operator,
            } => {
                use crate::parser::ast::ArithmeticOp;

                let left_val = self.eval_with_env(*left, env)?;
                let right_val = self.eval_with_env(*right, env)?;

                match (left_val, right_val) {
                    // Numeric arithmetic
                    (Value::Number(l), Value::Number(r)) => {
                        let result = match operator {
                            ArithmeticOp::Add => l + r,
                            ArithmeticOp::Subtract => l - r,
                            ArithmeticOp::Multiply => l * r,
                            ArithmeticOp::Divide => {
                                if r == 0 {
                                    return Err(anyhow!("Division by zero"));
                                }
                                l / r
                            }
                            ArithmeticOp::Modulo => {
                                if r == 0 {
                                    return Err(anyhow!("Modulo by zero"));
                                }
                                l % r
                            }
                        };
                        Ok(Value::Number(result))
                    }
                    // Runtime transposition: Note +/- Number
                    (Value::Note(note), Value::Number(n)) => {
                        let semitones = match operator {
                            ArithmeticOp::Add => n as i8,
                            ArithmeticOp::Subtract => -(n as i8),
                            _ => return Err(anyhow!("Only +/- supported for note transposition")),
                        };
                        Ok(Value::Note(note + semitones))
                    }
                    // Runtime transposition: Chord +/- Number
                    (Value::Chord(chord), Value::Number(n)) => {
                        let semitones = match operator {
                            ArithmeticOp::Add => n as i8,
                            ArithmeticOp::Subtract => -(n as i8),
                            _ => return Err(anyhow!("Only +/- supported for chord transposition")),
                        };
                        Ok(Value::Chord(chord + semitones))
                    }
                    // Runtime transposition: Pattern +/- Number
                    (Value::Pattern(pattern), Value::Number(n)) => {
                        let semitones = match operator {
                            ArithmeticOp::Add => n as i8,
                            ArithmeticOp::Subtract => -(n as i8),
                            _ => {
                                return Err(anyhow!("Only +/- supported for pattern transposition"))
                            }
                        };
                        Ok(Value::Pattern(pattern + semitones))
                    }
                    (l, r) => Err(anyhow!(
                        "Arithmetic operations require numeric values, got {:?} and {:?}",
                        l,
                        r
                    )),
                }
            }

            // Pre-evaluated value - just unwrap it
            Expression::Value(v) => Ok(*v),

            // Array of expressions - evaluate all elements
            // If all elements are notes, construct a Chord; otherwise, return Array
            Expression::Array(elements) => {
                let values: Vec<Value> = elements
                    .into_iter()
                    .map(|e| self.eval_with_env(e, env))
                    .collect::<Result<Vec<_>>>()?;

                // Check if ALL values are notes → construct a Chord
                if values.iter().all(|v| matches!(v, Value::Note(_))) {
                    let notes: Vec<Note> = values
                        .into_iter()
                        .map(|v| match v {
                            Value::Note(n) => n,
                            _ => unreachable!(),
                        })
                        .collect();
                    Ok(Value::Chord(Chord::from_notes(notes)))
                } else {
                    // Otherwise, return as Array
                    Ok(Value::Array(values))
                }
            }
        }
    }

    /// Evaluate a function call with optional environment for variable resolution
    fn eval_function_with_env(
        &self,
        name: &str,
        args: Vec<Expression>,
        env: Option<&crate::parser::environment::Environment>,
    ) -> Result<Value> {
        // First, check for user-defined functions in the environment
        if let Some(environment) = env {
            if let Some(func_value) = environment.get(name) {
                if let Value::Function {
                    params,
                    body,
                    name: func_name,
                } = func_value.clone()
                {
                    // Check argument count
                    if args.len() != params.len() {
                        return Err(anyhow!(
                            "{}() expects {} arguments, got {}",
                            func_name,
                            params.len(),
                            args.len()
                        ));
                    }

                    // Evaluate arguments
                    let mut arg_values = Vec::new();
                    for arg in args {
                        arg_values.push(self.eval_with_env(arg, env)?);
                    }

                    // Create a new environment with parameters bound to argument values
                    let mut local_env = crate::parser::environment::Environment::new();

                    // Copy outer environment bindings
                    for var_name in environment.all_names() {
                        if let Some(val) = environment.get(var_name) {
                            local_env.define(var_name.clone(), val.clone());
                        }
                    }

                    // Push new scope for locals
                    local_env.push_scope();

                    // Bind parameters to arguments
                    for (param, value) in params.iter().zip(arg_values.into_iter()) {
                        local_env.define(param.clone(), value);
                    }

                    // Execute body statements
                    return self.run_statements_in_local_env(&body, &mut local_env);
                }
            }
        }

        // Check built-in function registry
        if let Some(builtin) = crate::parser::builtins::get_registry().get(name) {
            return (builtin.handler)(self, args, env);
        }

        // Dynamic progression handling (patterns like I-V-vi-IV)
        if CommonProgressions::is_valid_progression(name)
            || CommonProgressions::is_numeric_progression(name)
            || CommonProgressions::is_roman_numeral_progression(name)
        {
            if args.len() != 1 {
                return Err(anyhow!("Progression {} expects 1 key argument", name));
            }

            let key_value = self.eval_with_env(args[0].clone(), env)?;
            if let Value::Note(key) = key_value {
                let pattern = CommonProgressions::get_progression(name, key)?;
                return Ok(Value::Pattern(pattern));
            } else {
                return Err(anyhow!("Progression {} expects a key (note)", name));
            }
        }

        Err(anyhow!("Unknown function: {}", name))
    }

    /// Call a function by name with already-evaluated Value arguments.
    /// This enables dynamic dispatch - passing functions as arguments to higher-order functions like map().
    ///
    /// # Arguments
    /// * `name` - The function name (can be builtin or user-defined)
    /// * `arg_values` - Already-evaluated argument values
    /// * `env` - Optional environment for user-defined function lookup
    pub fn call_function_by_name(
        &self,
        name: &str,
        arg_values: Vec<Value>,
        env: Option<&crate::parser::environment::Environment>,
    ) -> Result<Value> {
        // Convert Values to Expressions by wrapping them
        let args: Vec<Expression> = arg_values
            .into_iter()
            .map(|v| Expression::Value(Box::new(v)))
            .collect();

        // Delegate to the existing function evaluation logic
        self.eval_function_with_env(name, args, env)
    }

    /// Execute a list of statements in a local environment and return the result.
    /// Used for user-defined function body execution.
    ///
    /// Supports: let, if/else, repeat, loop, return, expressions
    /// Returns: Value::Unit for void functions, or the returned/last expression value
    fn run_statements_in_local_env(
        &self,
        statements: &[Statement],
        local_env: &mut crate::parser::environment::Environment,
    ) -> Result<Value> {
        use crate::parser::ast::Statement;

        let mut last_value = Value::Unit;

        for stmt in statements {
            match stmt {
                Statement::Let { name, value } => {
                    let val = self.eval_with_env(value.clone(), Some(local_env))?;
                    local_env.define(name.clone(), val);
                }

                Statement::Assign { name, value } => {
                    let val = self.eval_with_env(value.clone(), Some(local_env))?;
                    if local_env.is_defined(name) {
                        local_env.set(name, val).map_err(|e| anyhow!("{}", e))?;
                    } else {
                        return Err(anyhow!("Cannot assign to undefined variable '{}'", name));
                    }
                }

                Statement::Expression(expr) => {
                    last_value = self.eval_with_env(expr.clone(), Some(local_env))?;
                }

                Statement::Return(expr_opt) => {
                    return match expr_opt {
                        Some(expr) => self.eval_with_env(expr.clone(), Some(local_env)),
                        None => Ok(Value::Unit),
                    };
                }

                Statement::Block(block_stmts) => {
                    local_env.push_scope();
                    let result = self.run_statements_in_local_env(block_stmts, local_env);
                    local_env.pop_scope();
                    // If block returned, propagate
                    if let Ok(Value::Unit) = &result {
                        // Continue, don't update last_value
                    } else {
                        return result;
                    }
                }

                Statement::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    let cond_val = self.eval_with_env(condition.clone(), Some(local_env))?;
                    let is_true = match cond_val {
                        Value::Boolean(b) => b,
                        _ => return Err(anyhow!("Condition must be a boolean")),
                    };

                    let branch = if is_true {
                        then_body
                    } else {
                        match else_body {
                            Some(b) => b,
                            None => continue,
                        }
                    };

                    local_env.push_scope();
                    let result = self.run_statements_in_local_env(branch, local_env);
                    local_env.pop_scope();
                    if let Ok(Value::Unit) = &result {
                        // Continue
                    } else {
                        return result;
                    }
                }

                Statement::Repeat { count, body } => {
                    for _ in 0..*count {
                        local_env.push_scope();
                        let result = self.run_statements_in_local_env(body, local_env);
                        local_env.pop_scope();
                        // Note: we don't support break/continue in pure evaluation
                        if let Ok(Value::Unit) = &result {
                            // Continue loop
                        } else if let Ok(_) = &result {
                            return result; // Return from function
                        } else {
                            return result; // Error
                        }
                    }
                }

                Statement::Loop { body } => {
                    // Infinite loop - can only exit via return
                    loop {
                        local_env.push_scope();
                        let result = self.run_statements_in_local_env(body, local_env);
                        local_env.pop_scope();
                        match result {
                            Ok(Value::Unit) => continue,
                            Ok(v) => return Ok(v), // Return from function
                            Err(e) => return Err(e),
                        }
                    }
                }

                Statement::For {
                    var,
                    start,
                    end,
                    body,
                } => {
                    let start_val = self.eval_with_env(start.clone(), Some(local_env))?;
                    let end_val = self.eval_with_env(end.clone(), Some(local_env))?;

                    let start_num = match start_val {
                        Value::Number(n) => n,
                        _ => return Err(anyhow!("For loop start must be a number")),
                    };
                    let end_num = match end_val {
                        Value::Number(n) => n,
                        _ => return Err(anyhow!("For loop end must be a number")),
                    };

                    for i in start_num..end_num {
                        local_env.push_scope();
                        local_env.define(var.clone(), Value::Number(i));
                        let result = self.run_statements_in_local_env(body, local_env);
                        local_env.pop_scope();
                        if let Err(e) = result {
                            return Err(e);
                        }
                        if let Ok(Value::Unit) = result {
                            continue;
                        } else if let Ok(v) = result {
                            return Ok(v);
                        }
                    }
                }

                Statement::FunctionDef {
                    name, params, body, ..
                } => {
                    // Define nested function in local scope
                    let func = Value::Function {
                        name: name.clone(),
                        params: params.clone(),
                        body: body.clone(),
                    };
                    local_env.define(name.clone(), func);
                }

                // Side-effect statements are not supported in pure function evaluation
                // They would need an Interpreter to execute properly
                Statement::Play { .. } => {
                    return Err(anyhow!("play is not supported inside pure functions. Use the Interpreter for side effects."));
                }
                Statement::Tempo(_) => {
                    return Err(anyhow!("tempo is not supported inside pure functions"));
                }
                Statement::Volume(_) => {
                    return Err(anyhow!("volume is not supported inside pure functions"));
                }
                Statement::Waveform(_) => {
                    return Err(anyhow!("waveform is not supported inside pure functions"));
                }
                Statement::Stop => {
                    return Err(anyhow!("stop is not supported inside pure functions"));
                }
                Statement::Load(_) => {
                    return Err(anyhow!("load is not supported inside functions"));
                }
                Statement::Track { .. } => {
                    return Err(anyhow!("track is not supported inside pure functions"));
                }
                Statement::Use { .. } => {
                    return Err(anyhow!("use/import is not supported inside functions"));
                }

                // No-ops
                Statement::Break => {
                    // In pure evaluation, break just exits current iteration
                    // But since we don't have proper control flow, treat as warning
                }
                Statement::Continue => {
                    // Same as break
                }
                Statement::Comment(_) => {
                    // No-op
                }

                // Wait is a side-effect (virtual time advancement) - no-op in pure evaluation
                Statement::Wait { .. } => {
                    // In pure function evaluation, wait is ignored
                    // It only has meaning in the interpreter context
                }
            }
        }

        Ok(last_value)
    }

    /// Format numeric progression names for display with proper chord types
    pub fn format_numeric_progression_name(numeric: &str) -> String {
        let roman_numerals = ["I", "ii", "iii", "IV", "V", "vi", "vii°"];

        let formatted: Vec<String> = numeric
            .chars()
            .filter_map(|c| {
                if let Some(digit) = c.to_digit(10) {
                    if digit >= 1 && digit <= 7 {
                        Some(roman_numerals[(digit - 1) as usize].to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        if formatted.is_empty() {
            numeric.to_string()
        } else {
            formatted.join("-")
        }
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to parse and evaluate a string expression
pub fn eval(input: &str) -> Result<Value> {
    let expr = crate::parser::parse(input)?;
    let evaluator = Evaluator::new();
    evaluator.eval(expr)
}

/// Convert a Value to PatternStep(s) for variable resolution in patterns
fn value_to_pattern_steps(value: &Value) -> Option<Vec<crate::types::PatternStep>> {
    use crate::types::PatternStep;
    match value {
        Value::Note(note) => Some(vec![PatternStep::Note(*note)]),
        Value::Chord(chord) => Some(vec![PatternStep::Chord(chord.clone())]),
        Value::Pattern(pattern) => {
            // Return the pattern's steps directly
            Some(pattern.steps.clone())
        }
        Value::Thunk { expression, env } => {
            // Evaluate the thunk and recursively convert the result
            let evaluator = Evaluator::new();
            if let Ok(env_guard) = env.read() {
                if let Ok(resolved) = evaluator.eval_with_env(*expression.clone(), Some(&env_guard))
                {
                    return value_to_pattern_steps(&resolved);
                }
            }
            None
        }
        // Other values can't be converted to pattern steps
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn test_eval_note() {
        let expr = parse("C").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Note(note) => assert_eq!(note.pitch_class(), 0),
            _ => panic!("Expected note value"),
        }
    }

    #[test]
    fn test_eval_chord() {
        let expr = parse("[C, E, G]").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                assert_eq!(chord.len(), 3);
                assert!(chord.contains(&"C".parse().unwrap()));
                assert!(chord.contains(&"E".parse().unwrap()));
                assert!(chord.contains(&"G".parse().unwrap()));
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_transpose_note() {
        let expr = parse("C + 2").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Note(note) => assert_eq!(note.pitch_class(), 2), // D
            _ => panic!("Expected note value"),
        }
    }

    #[test]
    fn test_eval_transpose_chord() {
        let expr = parse("[C, E, G] + 2").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                let pitch_classes: Vec<u8> = chord.notes().map(|n| n.pitch_class()).collect();
                assert!(pitch_classes.contains(&2)); // D
                assert!(pitch_classes.contains(&6)); // F#
                assert!(pitch_classes.contains(&9)); // A
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_intersection() {
        let expr = parse("[C, E, G] & [A, C, E]").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                assert_eq!(chord.len(), 2); // C and E
                assert!(chord.contains(&"C".parse().unwrap()));
                assert!(chord.contains(&"E".parse().unwrap()));
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_progression() {
        let expr = parse("[[C, E, G], [F, A, C]]").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(pattern) => {
                let chords = pattern.as_chords().expect("Should be chord-only pattern");
                assert_eq!(chords.len(), 2);
                assert!(chords[0].contains(&"C".parse().unwrap()));
                assert!(chords[1].contains(&"F".parse().unwrap()));
            }
            _ => panic!("Expected pattern value"),
        }
    }

    #[test]
    fn test_eval_progression_transpose() {
        let expr = parse("[[C, E, G], [F, A, C]] + 2").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(pattern) => {
                // First chord should be D major (C major + 2)
                let chords = pattern.as_chords().expect("Should be chord-only pattern");
                let first_chord = &chords[0];
                let pitch_classes: Vec<u8> = first_chord.notes().map(|n| n.pitch_class()).collect();
                assert!(pitch_classes.contains(&2)); // D
                assert!(pitch_classes.contains(&6)); // F#
                assert!(pitch_classes.contains(&9)); // A
            }
            _ => panic!("Expected pattern value"),
        }
    }

    #[test]
    fn test_eval_retrograde_function() {
        let expr = parse("retrograde([[C, E, G], [F, A, C], [G, B, D]])").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(pattern) => {
                let chords = pattern.as_chords().expect("Should be chord-only pattern");
                assert_eq!(chords.len(), 3);
                // First chord should now be G major (was last)
                assert!(chords[0].contains(&"G".parse().unwrap()));
                assert!(chords[0].contains(&"B".parse().unwrap()));
                assert!(chords[0].contains(&"D".parse().unwrap()));
            }
            _ => panic!("Expected pattern value"),
        }
    }

    #[test]
    fn test_eval_map_function() {
        let expr = parse("map(invert, [[C, E, G], [F, A, C]])").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(pattern) => {
                let chords = pattern.as_chords().expect("Should be chord-only pattern");
                assert_eq!(chords.len(), 2);
                // First chord should be C major first inversion (E in bass, C root)
                // Compare pitch_class because octave changes during inversion
                assert_eq!(chords[0].bass().unwrap().pitch_class(), 4); // E
                assert_eq!(chords[0].root().unwrap().pitch_class(), 0); // C
            }
            _ => panic!("Expected pattern value"),
        }
    }

    #[test]
    fn test_eval_union() {
        let expr = parse("[C, E, G] | [A, C, E]").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                assert_eq!(chord.len(), 4); // A, C, E, G
                assert!(chord.contains(&"A".parse().unwrap()));
                assert!(chord.contains(&"C".parse().unwrap()));
                assert!(chord.contains(&"E".parse().unwrap()));
                assert!(chord.contains(&"G".parse().unwrap()));
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_difference() {
        let expr = parse("[C, E, G] ^ [A, C, E]").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                assert_eq!(chord.len(), 2); // A and G
                assert!(chord.contains(&"A".parse().unwrap()));
                assert!(chord.contains(&"G".parse().unwrap()));
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_invert_function() {
        let expr = parse("invert([C, E, G])").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                // After inversion, bass becomes E (pitch class 4), root is still C (pitch class 0)
                // We compare pitch_class because the octave changes during inversion
                assert_eq!(chord.bass().unwrap().pitch_class(), 4); // E in bass
                assert_eq!(chord.root().unwrap().pitch_class(), 0); // C still root
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_root_function() {
        let expr = parse("root([C, E, G])").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Note(note) => assert_eq!(note, "C".parse().unwrap()),
            _ => panic!("Expected note value"),
        }
    }

    #[test]
    fn test_eval_bass_function() {
        let expr = parse("bass(invert([C, E, G]))").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Note(note) => assert_eq!(note, "E".parse().unwrap()),
            _ => panic!("Expected note value"),
        }
    }

    #[test]
    fn test_eval_convenience_function() {
        let result = eval("[C, E, G] + 2").unwrap();

        match result {
            Value::Chord(chord) => {
                let pitch_classes: Vec<u8> = chord.notes().map(|n| n.pitch_class()).collect();
                assert!(pitch_classes.contains(&2)); // D
                assert!(pitch_classes.contains(&6)); // F#
                assert!(pitch_classes.contains(&9)); // A
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_error_unknown_function() {
        let expr = parse("unknown([C, E, G])").unwrap();
        let result = Evaluator::new().eval(expr);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown function"));
    }

    #[test]
    fn test_eval_error_wrong_argument_count() {
        let expr = parse("invert([C, E, G], [F, A, C])").unwrap();
        let result = Evaluator::new().eval(expr);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("expects 1 argument"));
    }

    #[test]
    fn test_eval_error_wrong_argument_type() {
        let expr = parse("invert(C)").unwrap();
        let result = Evaluator::new().eval(expr);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("only works on chords"));
    }

    #[test]
    fn test_eval_arithmetic_multiply() {
        let result = eval("3 * 4").unwrap();
        assert_eq!(result, Value::Number(12));
    }

    #[test]
    fn test_eval_arithmetic_divide() {
        let result = eval("10 / 2").unwrap();
        assert_eq!(result, Value::Number(5));
    }

    #[test]
    fn test_eval_arithmetic_modulo() {
        let result = eval("10 % 3").unwrap();
        assert_eq!(result, Value::Number(1));
    }

    #[test]
    fn test_eval_arithmetic_add() {
        let result = eval("3 + 4").unwrap();
        assert_eq!(result, Value::Number(7));
    }

    #[test]
    fn test_eval_arithmetic_subtract() {
        let result = eval("10 - 3").unwrap();
        assert_eq!(result, Value::Number(7));
    }

    #[test]
    fn test_eval_arithmetic_bodmas_precedence() {
        // 2 + 3 * 4 = 2 + 12 = 14 (not 20)
        let result = eval("2 + 3 * 4").unwrap();
        assert_eq!(result, Value::Number(14));
    }

    #[test]
    fn test_eval_arithmetic_parentheses_override() {
        // (2 + 3) * 4 = 5 * 4 = 20
        let result = eval("(2 + 3) * 4").unwrap();
        assert_eq!(result, Value::Number(20));
    }

    #[test]
    fn test_eval_arithmetic_complex() {
        // 100 + 10 * 5 - 20 / 2 = 100 + 50 - 10 = 140
        let result = eval("100 + 10 * 5 - 20 / 2").unwrap();
        assert_eq!(result, Value::Number(140));
    }

    #[test]
    fn test_eval_arithmetic_division_by_zero() {
        let result = eval("10 / 0");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Division by zero"));
    }

    #[test]
    fn test_eval_arithmetic_modulo_by_zero() {
        let result = eval("10 % 0");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Modulo by zero"));
    }
}

#[cfg(test)]
mod evaluator_numeric_tests {
    use super::*;
    use crate::parser::parse;
    use crate::types::analyze_progression;

    #[test]
    fn test_eval_numeric_progression() {
        // 251(C) now works again - parser treats Number+LeftParen as function call
        let expr = parse("251(C)").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(progression) => {
                assert_eq!(progression.len(), 3);
                // Should be ii-V-I in C: Dm - G - C
                let key = "C".parse().unwrap();
                let analysis = analyze_progression(&progression, key).unwrap();
                assert_eq!(analysis[0].to_string(), "ii");
                assert_eq!(analysis[1].to_string(), "V");
                assert_eq!(analysis[2].to_string(), "I");
            }
            _ => panic!("Expected progression value"),
        }
    }

    #[test]
    fn test_eval_long_numeric_progression() {
        let expr = parse("16251(F)").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(progression) => {
                assert_eq!(progression.len(), 5);
                let key = "F".parse().unwrap();
                let analysis = analyze_progression(&progression, key).unwrap();
                assert_eq!(analysis[0].to_string(), "I");
                assert_eq!(analysis[1].to_string(), "vi");
                assert_eq!(analysis[2].to_string(), "ii");
                assert_eq!(analysis[3].to_string(), "V");
                assert_eq!(analysis[4].to_string(), "I");
            }
            _ => panic!("Expected progression value"),
        }
    }

    #[test]
    fn test_eval_invalid_numeric_progression() {
        let expr = parse("189(C)").unwrap();
        let result = Evaluator::new().eval(expr);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid scale degree"));
    }

    #[test]
    fn test_eval_invert_progression() {
        // Test that invert works on progressions
        let expr = parse("invert([[C, E, G], [F, A, C]])").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(prog) => {
                assert_eq!(prog.len(), 2);
            }
            _ => panic!("Expected Progression value"),
        }
    }

    #[test]
    fn test_self_referential_variable_error() {
        // Test that `let x = x` correctly detects the cycle and errors
        use crate::parser::interpreter::Interpreter;
        use crate::parser::statement_parser::parse_statements;

        let program = parse_statements("let x = x").unwrap();
        let mut interpreter = Interpreter::new();
        interpreter.run_program(&program).unwrap();

        // Now try to access x - should get an error, not infinite loop
        let env = interpreter.environment.read().unwrap();
        let evaluator = Evaluator::new();

        // Get 'x' and try to evaluate it
        let result = evaluator.eval_with_env(
            crate::parser::ast::Expression::Variable("x".to_string()),
            Some(&env),
        );

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Recursive variable definition"),
            "Expected recursive error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_self_referential_with_operation_error() {
        // Test that `let a = a + 1` also detects the cycle
        use crate::parser::interpreter::Interpreter;
        use crate::parser::statement_parser::parse_statements;

        let program = parse_statements("let a = a + 1").unwrap();
        let mut interpreter = Interpreter::new();
        interpreter.run_program(&program).unwrap();

        let env = interpreter.environment.read().unwrap();
        let evaluator = Evaluator::new();

        let result = evaluator.eval_with_env(
            crate::parser::ast::Expression::Variable("a".to_string()),
            Some(&env),
        );

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Recursive variable definition"),
            "Expected recursive error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_valid_reassignment_still_works() {
        // Test that valid reassignment pattern `let a = [...]; a = a.something()` still works
        use crate::parser::interpreter::Interpreter;
        use crate::parser::statement_parser::parse_statements;

        let program = parse_statements("let a = [C, E, G]\na = a + 12").unwrap();
        let mut interpreter = Interpreter::new();
        let result = interpreter.run_program(&program);

        // This should succeed - reassignment is different from self-referential definition
        assert!(
            result.is_ok(),
            "Reassignment should work, got: {:?}",
            result
        );
    }
}
