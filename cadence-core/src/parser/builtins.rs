use crate::parser::ast::{Expression, Value};
use crate::parser::environment::Environment;
use crate::parser::evaluator::Evaluator;
use crate::types::{
    analyze_progression, Chord, CommonProgressions, Note, RomanNumeral, VoiceLeading,
};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub type BuiltinHandler =
    Arc<dyn Fn(&Evaluator, Vec<Expression>, Option<&Environment>) -> Result<Value> + Send + Sync>;

static REGISTRY: OnceLock<FunctionRegistry> = OnceLock::new();

pub fn get_registry() -> &'static FunctionRegistry {
    REGISTRY.get_or_init(FunctionRegistry::new)
}

pub struct BuiltinFunction {
    pub name: String,
    pub category: String, // e.g., "Core", "Math", "Pattern", "Audio"
    pub description: String,
    pub signature: String, // e.g., "fast(pattern: Pattern, factor: Number/Note) -> Pattern"
    pub handler: BuiltinHandler,
}

impl BuiltinFunction {
    pub fn arity(&self) -> usize {
        // Legacy: return the first valid arity default
        *self.valid_arities().first().unwrap_or(&0)
    }

    pub fn valid_arities(&self) -> Vec<usize> {
        let mut arities = Vec::new();
        // Split signature by " or " to handle overloads
        let parts: Vec<&str> = self.signature.split(" or ").collect();

        for part in parts {
            if let Some(start) = part.find('(') {
                if let Some(end) = part.find(')') {
                    let args_str = &part[start + 1..end];
                    let mut count = if args_str.trim().is_empty() {
                        0
                    } else {
                        args_str.chars().filter(|c| *c == ',').count() + 1
                    };

                    // Heuristic: If signature looks like a method call (e.g. "pattern.env(...)"),
                    // and the function logic expects the object as the first argument,
                    // we need to add 1 to the count for the implicit 'self'.
                    // Most builtins in Cadence that are documented as methods still take the object as first arg in the handler.
                    if part.trim().contains('.') && !part.trim().starts_with("fn") {
                        count += 1;
                    }

                    arities.push(count);
                }
            }
        }
        
        if arities.is_empty() {
            vec![0]
        } else {
            arities.sort();
            arities.dedup();
            arities
        }
    }
}

pub struct DocItem {
    pub name: String,
    pub category: String,
    pub description: String,
    pub signature: String,
}

pub struct FunctionRegistry {
    functions: HashMap<String, BuiltinFunction>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        let mut registry = FunctionRegistry {
            functions: HashMap::new(),
        };
        registry.register_all();
        registry
    }

    fn register(
        &mut self,
        name: &str,
        category: &str,
        description: &str,
        signature: &str,
        handler: BuiltinHandler,
    ) {
        self.functions.insert(
            name.to_string(),
            BuiltinFunction {
                name: name.to_string(),
                category: category.to_string(),
                description: description.to_string(),
                signature: signature.to_string(),
                handler,
            },
        );
    }

    pub fn get_documentation(&self) -> Vec<DocItem> {
        let mut docs: Vec<DocItem> = self
            .functions
            .values()
            .map(|f| DocItem {
                name: f.name.clone(),
                category: f.category.clone(),
                description: f.description.clone(),
                signature: f.signature.clone(),
            })
            .collect();

        docs.sort_by(|a, b| a.name.cmp(&b.name));
        docs
    }

    pub fn get(&self, name: &str) -> Option<&BuiltinFunction> {
        self.functions.get(name)
    }

    fn register_all(&mut self) {
        // --- Pattern Functions ---

        self.register(
            "fast",
            "Pattern",
            "Speeds up a pattern by a given factor.",
            "fast(pattern: Pattern, factor: Number | Note) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("fast() expects 2 arguments: pattern, factor"));
                }

                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let factor_value = evaluator.eval_with_env(args[1].clone(), env)?;

                let factor = match factor_value {
                    Value::Note(note) => (note.pitch_class() as usize).max(1),
                    Value::Number(n) => (n as usize).max(1),
                    _ => return Err(anyhow!("fast() factor must be a note or number")),
                };

                match pattern_value {
                    Value::Pattern(p) => Ok(Value::Pattern(p.fast(factor))),
                    Value::String(s) => {
                        let pattern = crate::types::Pattern::parse(&s)
                            .map_err(|e| anyhow!("fast(): invalid pattern string: {}", e))?;
                        Ok(Value::Pattern(pattern.fast(factor)))
                    }
                    Value::EveryPattern(every) => {
                        // Apply fast to both base and transformed patterns
                        let fast_every = crate::types::EveryPattern::new(
                            every.interval,
                            every.base.clone().fast(factor),
                            every.transformed.clone().fast(factor),
                        );
                        Ok(Value::EveryPattern(Box::new(fast_every)))
                    }
                    // Auto-wrap Note/Chord into single-step patterns for method chaining
                    Value::Note(n) => {
                        let pattern = crate::types::Pattern::with_steps(vec![
                            crate::types::PatternStep::Note(n)
                        ]);
                        Ok(Value::Pattern(pattern.fast(factor)))
                    }
                    Value::Chord(c) => {
                        let pattern = crate::types::Pattern::with_steps(vec![
                            crate::types::PatternStep::Chord(c)
                        ]);
                        Ok(Value::Pattern(pattern.fast(factor)))
                    }
                    _ => Err(anyhow!("fast() first argument must be a pattern, note, chord, or pattern string")),
                }
            }),
        );

        self.register(
            "slow",
            "Pattern",
            "Slows down a pattern by a given factor.",
            "slow(pattern: Pattern, factor: Number | Note) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("slow() expects 2 arguments: pattern, factor"));
                }

                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let factor_value = evaluator.eval_with_env(args[1].clone(), env)?;

                let factor = match factor_value {
                    Value::Note(note) => (note.pitch_class() as usize).max(1),
                    Value::Number(n) => (n as usize).max(1),
                    _ => return Err(anyhow!("slow() factor must be a note or number")),
                };

                match pattern_value {
                    Value::Pattern(p) => Ok(Value::Pattern(p.slow(factor))),
                    Value::String(s) => {
                        let pattern = crate::types::Pattern::parse(&s)
                            .map_err(|e| anyhow!("slow(): invalid pattern string: {}", e))?;
                        Ok(Value::Pattern(pattern.slow(factor)))
                    }
                    Value::EveryPattern(every) => {
                        // Apply slow to both base and transformed patterns
                        let slow_every = crate::types::EveryPattern::new(
                            every.interval,
                            every.base.clone().slow(factor),
                            every.transformed.clone().slow(factor),
                        );
                        Ok(Value::EveryPattern(Box::new(slow_every)))
                    }
                    // Auto-wrap Note/Chord into single-step patterns for method chaining
                    Value::Note(n) => {
                        let pattern = crate::types::Pattern::with_steps(vec![
                            crate::types::PatternStep::Note(n)
                        ]);
                        Ok(Value::Pattern(pattern.slow(factor)))
                    }
                    Value::Chord(c) => {
                        let pattern = crate::types::Pattern::with_steps(vec![
                            crate::types::PatternStep::Chord(c)
                        ]);
                        Ok(Value::Pattern(pattern.slow(factor)))
                    }
                    _ => Err(anyhow!("slow() first argument must be a pattern, note, chord, or pattern string")),
                }
            }),
        );

        // at() - Index into a pattern, chord, or array
        self.register(
            "at",
            "Pattern",
            "Returns the element at the specified index (0-based). Negative indices count from the end.",
            "at(pattern: Pattern | Chord | Array, index: Number) -> Note | Chord | Value",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("at() expects 2 arguments: target, index"));
                }

                let target_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let index_value = evaluator.eval_with_env(args[1].clone(), env)?;

                let idx = match index_value {
                    Value::Number(n) => n,
                    _ => return Err(anyhow!("at() index must be a number")),
                };

                match target_value {
                    Value::Pattern(pattern) => {
                        let len = pattern.steps.len() as i32;
                        if len == 0 {
                            return Err(anyhow!("Cannot index into empty pattern"));
                        }
                        let actual_idx = if idx < 0 { len + idx } else { idx };
                        if actual_idx < 0 || actual_idx >= len {
                            return Err(anyhow!(
                                "Index {} out of bounds for pattern with {} steps",
                                idx,
                                len
                            ));
                        }
                        use crate::types::PatternStep;
                        fn step_to_value(step: &PatternStep) -> Result<Value> {
                            match step {
                                PatternStep::Note(n) => Ok(Value::Note(*n)),
                                PatternStep::Chord(c) => Ok(Value::Chord(c.clone())),
                                PatternStep::Rest => Ok(Value::Pattern(
                                    crate::types::Pattern::with_steps(vec![PatternStep::Rest]),
                                )),
                                PatternStep::Drum(d) => Ok(Value::String(d.short_name().to_string())),
                                PatternStep::Variable(_) => {
                                    Err(anyhow!("Cannot index unresolved variable"))
                                }
                                PatternStep::Group(steps) => Ok(Value::Pattern(
                                    crate::types::Pattern::with_steps(steps.clone()),
                                )),
                                PatternStep::Repeat(inner, count) => Ok(Value::Pattern(
                                    crate::types::Pattern::with_steps(vec![PatternStep::Repeat(
                                        inner.clone(),
                                        *count,
                                    )]),
                                )),
                                PatternStep::Weighted(inner, _) => step_to_value(inner),
                                PatternStep::Alternation(steps) => Ok(Value::Pattern(
                                    crate::types::Pattern::with_steps(steps.clone()),
                                )),
                                PatternStep::Euclidean(inner, pulses, steps) => Ok(Value::Pattern(
                                    crate::types::Pattern::with_steps(vec![PatternStep::Euclidean(
                                        inner.clone(),
                                        *pulses,
                                        *steps,
                                    )]),
                                )),
                                PatternStep::Polyrhythm(sub_patterns) => Ok(Value::Pattern(
                                    crate::types::Pattern::with_steps(vec![PatternStep::Polyrhythm(
                                        sub_patterns.clone(),
                                    )]),
                                )),
                                PatternStep::Velocity(inner, _) => step_to_value(inner),
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
                    _ => Err(anyhow!(
                        "at() requires Pattern, Chord, or Array as first argument"
                    )),
                }
            }),
        );

        // beat() - Returns the current global beat
        self.register(
            "beat",
            "Time",
            "Returns the current global beat (0-based). Use modulo for periodic patterns.",
            "beat() -> Number",
            Arc::new(|_evaluator, args, env| {
                if !args.is_empty() {
                    return Err(anyhow!("beat() takes no arguments"));
                }
                // Read _beat from environment if available
                if let Some(e) = env {
                    if let Some(Value::Number(n)) = e.get("_beat") {
                        return Ok(Value::Number(*n));
                    }
                }
                Ok(Value::Number(0)) // Default if not in playback context
            }),
        );

        self.register(
            "rev",
            "Pattern",
            "Reverses a pattern.",
            "rev(pattern: Pattern) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 1 {
                    return Err(anyhow!("rev() expects 1 argument: pattern"));
                }

                let arg_value = evaluator.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Pattern(p) => Ok(Value::Pattern(p.rev())),
                    Value::String(s) => {
                        let pattern = crate::types::Pattern::parse(&s)
                            .map_err(|e| anyhow!("rev(): invalid pattern string: {}", e))?;
                        Ok(Value::Pattern(pattern.rev()))
                    }
                    Value::EveryPattern(every) => {
                        // When reversing an EveryPattern, reverse both base and transformed
                        let reversed = crate::types::EveryPattern::new(
                            every.interval,
                            every.base.clone().rev(),
                            every.transformed.clone().rev(),
                        );
                        Ok(Value::EveryPattern(Box::new(reversed)))
                    }
                    _ => Err(anyhow!("rev() only works on patterns")),
                }
            }),
        );

        self.register(
            "rotate",
            "Pattern",
            "Rotates pattern steps by n positions. Positive rotates right, negative rotates left.",
            "rotate(pattern: Pattern, n: Number) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("rotate() expects 2 arguments: pattern, n"));
                }

                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let n_value = evaluator.eval_with_env(args[1].clone(), env)?;

                let pattern = match pattern_value {
                    Value::Pattern(p) => p,
                    Value::String(s) => crate::types::Pattern::parse(&s)
                        .map_err(|e| anyhow!("rotate(): invalid pattern: {}", e))?,
                    _ => return Err(anyhow!("rotate() first argument must be a pattern")),
                };

                let n = match n_value {
                    Value::Number(n) => n,
                    Value::Note(note) => note.pitch_class() as i32,
                    _ => return Err(anyhow!("rotate() second argument must be a number")),
                };

                Ok(Value::Pattern(pattern.rotate(n)))
            }),
        );

        self.register(
            "take",
            "Pattern",
            "Takes the first n steps of a pattern.",
            "take(pattern: Pattern, n: Number) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("take() expects 2 arguments: pattern, n"));
                }

                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let n_value = evaluator.eval_with_env(args[1].clone(), env)?;

                let pattern = match pattern_value {
                    Value::Pattern(p) => p,
                    Value::String(s) => crate::types::Pattern::parse(&s)
                        .map_err(|e| anyhow!("take(): invalid pattern: {}", e))?,
                    _ => return Err(anyhow!("take() first argument must be a pattern")),
                };

                let n = match n_value {
                    Value::Number(n) => n.max(0) as usize,
                    Value::Note(note) => note.pitch_class() as usize,
                    _ => return Err(anyhow!("take() second argument must be a number")),
                };

                Ok(Value::Pattern(pattern.take(n)))
            }),
        );

        self.register(
            "chunk",
            "Pattern",
            "Takes the first n steps of a pattern (alias for take).",
            "chunk(pattern: Pattern, n: Number) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("chunk() expects 2 arguments: pattern, n"));
                }

                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let n_value = evaluator.eval_with_env(args[1].clone(), env)?;

                let pattern = match pattern_value {
                    Value::Pattern(p) => p,
                    Value::String(s) => crate::types::Pattern::parse(&s)
                        .map_err(|e| anyhow!("chunk(): invalid pattern: {}", e))?,
                    _ => return Err(anyhow!("chunk() first argument must be a pattern")),
                };

                let n = match n_value {
                    Value::Number(n) => n.max(0) as usize,
                    Value::Note(note) => note.pitch_class() as usize,
                    _ => return Err(anyhow!("chunk() second argument must be a number")),
                };

                Ok(Value::Pattern(pattern.take(n)))
            }),
        );

        self.register(
            "drop",
            "Pattern",
            "Drops the first n steps of a pattern.",
            "drop(pattern: Pattern, n: Number) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("drop() expects 2 arguments: pattern, n"));
                }

                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let n_value = evaluator.eval_with_env(args[1].clone(), env)?;

                let pattern = match pattern_value {
                    Value::Pattern(p) => p,
                    Value::String(s) => crate::types::Pattern::parse(&s)
                        .map_err(|e| anyhow!("drop(): invalid pattern: {}", e))?,
                    _ => return Err(anyhow!("drop() first argument must be a pattern")),
                };

                let n = match n_value {
                    Value::Number(n) => n.max(0) as usize,
                    Value::Note(note) => note.pitch_class() as usize,
                    _ => return Err(anyhow!("drop() second argument must be a number")),
                };

                Ok(Value::Pattern(pattern.drop_steps(n)))
            }),
        );

        self.register(
            "palindrome",
            "Pattern",
            "Creates a palindrome: pattern followed by its reverse.",
            "palindrome(pattern: Pattern) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 1 {
                    return Err(anyhow!("palindrome() expects 1 argument: pattern"));
                }

                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;

                match pattern_value {
                    Value::Pattern(p) => Ok(Value::Pattern(p.palindrome())),
                    Value::String(s) => {
                        let pattern = crate::types::Pattern::parse(&s)
                            .map_err(|e| anyhow!("palindrome(): invalid pattern: {}", e))?;
                        Ok(Value::Pattern(pattern.palindrome()))
                    }
                    Value::EveryPattern(every) => {
                        // Apply palindrome to both base and transformed patterns
                        let palindrome_every = crate::types::EveryPattern::new(
                            every.interval,
                            every.base.clone().palindrome(),
                            every.transformed.clone().palindrome(),
                        );
                        Ok(Value::EveryPattern(Box::new(palindrome_every)))
                    }
                    _ => Err(anyhow!("palindrome() argument must be a pattern")),
                }
            }),
        );

        self.register(
            "stutter",
            "Pattern",
            "Repeats each step n times.",
            "stutter(pattern: Pattern, n: Number) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("stutter() expects 2 arguments: pattern, n"));
                }

                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let n_value = evaluator.eval_with_env(args[1].clone(), env)?;

                let n = match n_value {
                    Value::Number(n) => n.max(1) as usize,
                    Value::Note(note) => (note.pitch_class() as usize).max(1),
                    _ => return Err(anyhow!("stutter() second argument must be a number")),
                };

                match pattern_value {
                    Value::Pattern(p) => Ok(Value::Pattern(p.stutter(n))),
                    Value::String(s) => {
                        let pattern = crate::types::Pattern::parse(&s)
                            .map_err(|e| anyhow!("stutter(): invalid pattern: {}", e))?;
                        Ok(Value::Pattern(pattern.stutter(n)))
                    }
                    Value::EveryPattern(every) => {
                        // Apply stutter to both base and transformed patterns
                        let stutter_every = crate::types::EveryPattern::new(
                            every.interval,
                            every.base.clone().stutter(n),
                            every.transformed.clone().stutter(n),
                        );
                        Ok(Value::EveryPattern(Box::new(stutter_every)))
                    }
                    _ => Err(anyhow!("stutter() first argument must be a pattern")),
                }
            }),
        );

        self.register(
            "len",
            "Core",
            "Returns the length of a pattern, chord, or array.",
            "len(target: Pattern | Chord | Array) -> Number",
            Arc::new(|evaluator, args, env| {
                if args.len() != 1 {
                    return Err(anyhow!("len() expects 1 argument"));
                }

                let value = evaluator.eval_with_env(args[0].clone(), env)?;

                let length = match value {
                    Value::Pattern(p) => p.len() as i32,
                    Value::String(s) => {
                        let pattern = crate::types::Pattern::parse(&s)
                            .map_err(|e| anyhow!("len(): invalid pattern: {}", e))?;
                        pattern.len() as i32
                    }
                    Value::Chord(c) => c.len() as i32,
                    Value::Array(a) => a.len() as i32,
                    _ => return Err(anyhow!("len() argument must be a pattern, chord, or array")),
                };

                Ok(Value::Number(length))
            }),
        );

        // cat - variadic pattern concatenation (replaces concat)
        self.register(
            "cat",
            "Pattern",
            "Concatenates multiple patterns together in sequence.",
            "cat(p1: Pattern, p2: Pattern, ...) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() < 2 {
                    return Err(anyhow!("cat() expects at least 2 arguments"));
                }

                // Convert all args to patterns
                let mut patterns: Vec<crate::types::Pattern> = Vec::new();
                for (i, arg) in args.into_iter().enumerate() {
                    let val = evaluator.eval_with_env(arg, env)?;
                    let pattern = match val {
                        Value::Pattern(p) => p,
                        Value::String(s) => crate::types::Pattern::parse(&s)
                            .map_err(|e| anyhow!("cat(): invalid pattern at position {}: {}", i + 1, e))?,
                        Value::Note(n) => crate::types::Pattern::with_steps(vec![
                            crate::types::PatternStep::Note(n),
                        ]),
                        Value::Chord(c) => crate::types::Pattern::with_steps(vec![
                            crate::types::PatternStep::Chord(c),
                        ]),
                        _ => return Err(anyhow!("cat(): argument {} must be a pattern, note, or chord", i + 1)),
                    };
                    patterns.push(pattern);
                }

                // Fold all patterns together
                let result = patterns.into_iter().reduce(|acc, p| acc.concat(p))
                    .unwrap(); // Safe: we checked len >= 2
                Ok(Value::Pattern(result))
            }),
        );

        // Keep concat as alias for backwards compatibility
        self.register(
            "concat",
            "Pattern",
            "Concatenates two patterns (alias for cat).",
            "concat(p1: Pattern, p2: Pattern) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("concat() expects 2 arguments"));
                }
                let p1_val = evaluator.eval_with_env(args[0].clone(), env)?;
                let p2_val = evaluator.eval_with_env(args[1].clone(), env)?;
                let p1 = match p1_val {
                    Value::Pattern(p) => p,
                    Value::String(s) => crate::types::Pattern::parse(&s)
                        .map_err(|e| anyhow!("concat(): invalid first pattern: {}", e))?,
                    _ => return Err(anyhow!("concat(): first argument must be a pattern")),
                };
                let p2 = match p2_val {
                    Value::Pattern(p) => p,
                    Value::String(s) => crate::types::Pattern::parse(&s)
                        .map_err(|e| anyhow!("concat(): invalid second pattern: {}", e))?,
                    _ => return Err(anyhow!("concat(): second argument must be a pattern")),
                };
                Ok(Value::Pattern(p1.concat(p2)))
            }),
        );

        // stack - layer patterns to play simultaneously at the same speed
        self.register(
            "stack",
            "Pattern",
            "Layers multiple patterns to play simultaneously (same cycle speed).",
            "stack(p1: Pattern, p2: Pattern, ...) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() < 2 {
                    return Err(anyhow!("stack() expects at least 2 arguments"));
                }

                // Convert all args to patterns
                let mut patterns: Vec<crate::types::Pattern> = Vec::new();
                for (i, arg) in args.into_iter().enumerate() {
                    let val = evaluator.eval_with_env(arg, env)?;
                    let pattern = match val {
                        Value::Pattern(p) => p,
                        Value::String(s) => crate::types::Pattern::parse(&s)
                            .map_err(|e| anyhow!("stack(): invalid pattern at position {}: {}", i + 1, e))?,
                        Value::Note(n) => crate::types::Pattern::with_steps(vec![
                            crate::types::PatternStep::Note(n),
                        ]),
                        Value::Chord(c) => crate::types::Pattern::with_steps(vec![
                            crate::types::PatternStep::Chord(c),
                        ]),
                        _ => return Err(anyhow!("stack(): argument {} must be a pattern, note, or chord", i + 1)),
                    };
                    patterns.push(pattern);
                }

                // Create a stacked pattern by merging steps
                let result = crate::types::Pattern::stack(patterns);
                Ok(Value::Pattern(result))
            }),
        );

        self.register(
            "transpose",
            "Pattern",
            "Transposes all notes in a pattern by n semitones.",
            "transpose(pattern: Pattern, semitones: Number) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "transpose() expects 2 arguments: pattern, semitones"
                    ));
                }

                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let semitones_value = evaluator.eval_with_env(args[1].clone(), env)?;

                let pattern = match pattern_value {
                    Value::Pattern(p) => p,
                    Value::String(s) => crate::types::Pattern::parse(&s)
                        .map_err(|e| anyhow!("transpose(): invalid pattern: {}", e))?,
                    _ => return Err(anyhow!("transpose() first argument must be a pattern")),
                };

                let semitones = match semitones_value {
                    Value::Number(n) => n as i8,
                    Value::Note(note) => note.pitch_class() as i8,
                    _ => return Err(anyhow!("transpose() second argument must be a number")),
                };

                Ok(Value::Pattern(pattern.transpose(semitones)))
            }),
        );

        self.register(
            "every",
            "Pattern",
            "Applies a transformation every n cycles during playback. Returns a pattern combinator that alternates between base and transformed patterns based on cycle position.",
            "every(n: Number, transform: String | Function, pattern: Pattern) -> EveryPattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 3 {
                    return Err(anyhow!(
                        "every() expects 3 arguments: interval, function_name, pattern"
                    ));
                }

                // Detect calling convention based on first argument type:
                // - Function style: every(n, transform, pattern)
                // - Method style:   every(pattern, n, transform) (desugared from pattern.every(n, transform))
                let first_val = evaluator.eval_with_env(args[0].clone(), env)?;
                
                let (n, transform_arg_idx, pattern_val) = match first_val {
                    // Function call style: every(n, transform, pattern)
                    Value::Number(num) => {
                        let n = num.max(1) as usize;
                        let pattern_val = evaluator.eval_with_env(args[2].clone(), env)?;
                        (n, 1usize, pattern_val)
                    }
                    Value::Note(note) => {
                        let n = note.pitch_class() as usize;
                        let pattern_val = evaluator.eval_with_env(args[2].clone(), env)?;
                        (n, 1usize, pattern_val)
                    }
                    // Method call style: every(pattern, n, transform)
                    Value::Pattern(_) | Value::String(_) => {
                        let n_val = evaluator.eval_with_env(args[1].clone(), env)?;
                        let n = match n_val {
                            Value::Number(num) => num.max(1) as usize,
                            Value::Note(note) => note.pitch_class() as usize,
                            _ => return Err(anyhow!("every() expects interval as second argument when called as method")),
                        };
                        (n, 2usize, first_val)
                    }
                    _ => return Err(anyhow!("every() first argument must be a number (function style) or pattern (method style)")),
                };

                // Extract the transform function name from the appropriate argument
                let transform_name = match &args[transform_arg_idx] {
                    Expression::Variable(name) => name.clone(),
                    Expression::String(s) => s.clone(),
                    Expression::Pattern(p) => p.to_string(),
                    Expression::FunctionCall {
                        name,
                        args: internal_args,
                    } if internal_args.is_empty() => name.clone(),
                    _ => {
                        return Err(anyhow!(
                            "every() expects a function name as transform argument"
                        ));
                    }
                };

                // Parse the base pattern
                let base_pattern = match pattern_val {
                    Value::Pattern(p) => p,
                    Value::String(s) => match crate::types::Pattern::parse(&s) {
                        Ok(p) => p,
                        Err(_) => match crate::parser::parse(&s) {
                            Ok(expr) => match evaluator.eval_with_env(expr, env)? {
                                Value::Pattern(p) => p,
                                _ => {
                                    return Err(anyhow!(
                                        "String \"{}\" evaluated to non-pattern in every()",
                                        s
                                    ));
                                }
                            },
                            Err(_) => {
                                return Err(
                                    anyhow!("every() expects a pattern or pattern string"),
                                );
                            }
                        },
                    },
                    _ => return Err(anyhow!("every() expects a pattern")),
                };

                // 4. Pre-compute the transformed pattern by calling the transform function
                let call_expr = Expression::FunctionCall {
                    name: transform_name.clone(),
                    args: vec![Expression::Pattern(base_pattern.clone())],
                };
                let transformed_val = evaluator.eval_with_env(call_expr, env)?;
                let transformed_pattern = match transformed_val {
                    Value::Pattern(p) => p,
                    _ => {
                        return Err(anyhow!(
                            "Transform function '{}' must return a pattern",
                            transform_name
                        ));
                    }
                };

                // 5. Return the EveryPattern combinator
                use crate::types::EveryPattern;
                Ok(Value::EveryPattern(Box::new(EveryPattern::new(
                    n,
                    base_pattern,
                    transformed_pattern,
                ))))
            }),
        );

        // --- Chord/Note Functions ---

        self.register(
            "invert",
            "Chord",
            "Inverts a chord or all chords in a pattern.",
            "invert(target: Chord | Pattern) -> Chord | Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 1 {
                    return Err(anyhow!("invert() expects 1 argument, got {}", args.len()));
                }

                let arg_value = evaluator.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Chord(chord) => {
                        let inverted = chord.invert();
                        Ok(Value::Chord(inverted))
                    }
                    Value::Pattern(pattern) => {
                        let inverted = pattern.map_chords(|chord| chord.invert());
                        Ok(Value::Pattern(inverted))
                    }
                    _ => Err(anyhow!("invert() only works on chords or progressions")),
                }
            }),
        );

        self.register(
            "invert_n",
            "Chord",
            "Inverts a chord n times.",
            "invert_n(chord: Chord, n: Note | Number) -> Chord",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "invert_n() expects 2 arguments, got {}",
                        args.len()
                    ));
                }

                let mut arg_iter = args.into_iter();
                let chord_expr = arg_iter.next().unwrap();
                let n_expr = arg_iter.next().unwrap();

                let chord_value = evaluator.eval_with_env(chord_expr, env)?;
                let n_value = evaluator.eval_with_env(n_expr, env)?;

                match (chord_value, n_value) {
                    (Value::Chord(chord), Value::Note(note)) => {
                        let n = note.pitch_class() as usize;
                        let inverted = chord.invert_n(n);
                        Ok(Value::Chord(inverted))
                    }
                    (Value::Chord(chord), Value::Number(n)) => {
                        let n = n as usize;
                        let inverted = chord.invert_n(n);
                        Ok(Value::Chord(inverted))
                    }
                    _ => Err(anyhow!("invert_n() expects (chord, note/number) arguments")),
                }
            }),
        );

        self.register(
            "root",
            "Chord",
            "Returns the root note of a chord.",
            "root(chord: Chord) -> Note",
            Arc::new(|evaluator, args, env| {
                if args.len() != 1 {
                    return Err(anyhow!("root() expects 1 argument, got {}", args.len()));
                }

                let arg_value = evaluator.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Chord(chord) => {
                        if let Some(root_note) = chord.root() {
                            Ok(Value::Note(root_note))
                        } else {
                            Err(anyhow!("Cannot determine root of empty chord"))
                        }
                    }
                    _ => Err(anyhow!("root() only works on chords")),
                }
            }),
        );

        self.register(
            "bass",
            "Chord",
            "Returns the bass (lowest) note of a chord.",
            "bass(chord: Chord) -> Note",
            Arc::new(|evaluator, args, env| {
                if args.len() != 1 {
                    return Err(anyhow!("bass() expects 1 argument, got {}", args.len()));
                }

                let arg_value = evaluator.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Chord(chord) => {
                        if let Some(bass_note) = chord.bass() {
                            Ok(Value::Note(bass_note))
                        } else {
                            Err(anyhow!("Cannot determine bass of empty chord"))
                        }
                    }
                    _ => Err(anyhow!("bass() only works on chords")),
                }
            }),
        );

        // --- Transformation/Analysis Functions ---

        self.register(
            "retrograde",
            "Pattern",
            "Reverses the order of steps in a pattern (same as rev).",
            "retrograde(progression: Pattern) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 1 {
                    return Err(anyhow!(
                        "retrograde() expects 1 argument, got {}",
                        args.len()
                    ));
                }

                let arg_value = evaluator.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Pattern(progression) => {
                        let retrograded = progression.retrograde();
                        Ok(Value::Pattern(retrograded))
                    }
                    _ => Err(anyhow!("retrograde() only works on progressions")),
                }
            }),
        );

        self.register(
            "map",
            "Pattern",
            "Applies a function to every chord in a pattern. Works with any function that takes a chord/note.",
            "map(function: Function, progression: Pattern) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("map() expects 2 arguments, got {}", args.len()));
                }

                let mut arg_iter = args.into_iter();
                let function_expr = arg_iter.next().unwrap();
                let progression_expr = arg_iter.next().unwrap();

                let func_name = match &function_expr {
                    Expression::Variable(name) => name.clone(),
                    Expression::FunctionCall {
                        name,
                        args: func_args,
                    } if func_args.is_empty() => name.clone(),
                    Expression::String(s) => s.clone(),
                    _ => {
                        return Err(anyhow!("map() first argument must be a function name"));
                    }
                };

                let progression_value = evaluator.eval_with_env(progression_expr, env)?;
                if let Value::Pattern(pattern) = progression_value {
                    // Extract chords from pattern
                    if let Some(chords) = pattern.as_chords() {
                        // Apply the function to each chord using dynamic dispatch
                        let mut mapped_chords = Vec::new();
                        for chord in chords {
                            let result = evaluator.call_function_by_name(
                                &func_name,
                                vec![Value::Chord(chord.clone())],
                                env,
                            )?;
                            
                            // Extract the chord from the result
                            match result {
                                Value::Chord(c) => mapped_chords.push(c),
                                Value::Note(n) => {
                                    // Single note returned - wrap in chord
                                    mapped_chords.push(crate::types::Chord::from_notes(vec![n]));
                                }
                                Value::Pattern(p) => {
                                    // If function returned a pattern, extract its chords
                                    if let Some(inner_chords) = p.as_chords() {
                                        mapped_chords.extend(inner_chords);
                                    } else {
                                        return Err(anyhow!(
                                            "map(): function '{}' returned non-chord pattern",
                                            func_name
                                        ));
                                    }
                                }
                                _ => {
                                    return Err(anyhow!(
                                        "map(): function '{}' must return a chord, got {:?}",
                                        func_name,
                                        result
                                    ));
                                }
                            }
                        }
                        
                        // Rebuild pattern from mapped chords
                        let mut result = crate::types::Pattern::from_chords(mapped_chords);
                        result.beats_per_cycle = pattern.beats_per_cycle;
                        result.envelope = pattern.envelope;
                        result.waveform = pattern.waveform;
                        result.pan = pattern.pan;
                        Ok(Value::Pattern(result))
                    } else {
                        // Pattern has non-chord steps - fall back to whole-pattern operations
                        // Try calling the function on the whole pattern
                        let result = evaluator.call_function_by_name(
                            &func_name,
                            vec![Value::Pattern(pattern)],
                            env,
                        )?;
                        Ok(result)
                    }
                } else {
                    Err(anyhow!("map() second argument must be a pattern"))
                }
            }),
        );


        // Voice Leading

        self.register(
            "voice_leading",
            "Voice Leading",
            "Analyzes voice leading between two chords.",
            "voice_leading(chord1: Chord, chord2: Chord) -> Chord",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "voice_leading() expects 2 arguments, got {}",
                        args.len()
                    ));
                }

                let mut arg_iter = args.into_iter();
                let chord1_expr = arg_iter.next().unwrap();
                let chord2_expr = arg_iter.next().unwrap();

                let chord1_value = evaluator.eval_with_env(chord1_expr, env)?;
                let chord2_value = evaluator.eval_with_env(chord2_expr, env)?;

                match (chord1_value, chord2_value) {
                    (Value::Chord(chord1), Value::Chord(chord2)) => {
                        let voice_leading = VoiceLeading::analyze(&chord1, &chord2);

                        let movement_info = format!(
                            "Voice leading: {} common tones, {} total movement, {}",
                            voice_leading.common_tones.len(),
                            voice_leading.total_movement,
                            voice_leading.voice_leading_type()
                        );

                        println!("{}", movement_info);

                        if !voice_leading.common_tones.is_empty() {
                            Ok(Value::Chord(Chord::from_notes(voice_leading.common_tones)))
                        } else {
                            Ok(Value::Chord(Chord::new()))
                        }
                    }
                    _ => Err(anyhow!("voice_leading() expects two chords")),
                }
            }),
        );

        self.register(
            "common_tones",
            "Voice Leading",
            "Returns the common tones between two chords.",
            "common_tones(chord1: Chord, chord2: Chord) -> Chord",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "common_tones() expects 2 arguments, got {}",
                        args.len()
                    ));
                }

                let mut arg_iter = args.into_iter();
                let chord1_expr = arg_iter.next().unwrap();
                let chord2_expr = arg_iter.next().unwrap();

                let chord1_value = evaluator.eval_with_env(chord1_expr, env)?;
                let chord2_value = evaluator.eval_with_env(chord2_expr, env)?;

                match (chord1_value, chord2_value) {
                    (Value::Chord(chord1), Value::Chord(chord2)) => {
                        let voice_leading = VoiceLeading::analyze(&chord1, &chord2);
                        Ok(Value::Chord(Chord::from_notes(voice_leading.common_tones)))
                    }
                    _ => Err(anyhow!("common_tones() expects two chords")),
                }
            }),
        );

        // Register alias 'ct' manually pointing to same handler logic if needed,
        // or just register another one.
        // For simplicity, I'll allow duplicates in registry or just handle it here.
        // Let's register 'ct' as alias.

        // Actually, Arc<closure> can be cloned.
        // But closures are unique types. I can share the code via a helper or just duplicate the Arc block.
        // Duplicating is easy.
        self.register(
            "ct",
            "Voice Leading",
            "Alias for common_tones.",
            "ct(chord1: Chord, chord2: Chord) -> Chord",
            Arc::new(|evaluator, args, env| {
                // Same logic as common_tones
                if args.len() != 2 {
                    return Err(anyhow!("ct() expects 2 arguments, got {}", args.len()));
                }
                let mut arg_iter = args.into_iter();
                let chord1 = evaluator.eval_with_env(arg_iter.next().unwrap(), env)?;
                let chord2 = evaluator.eval_with_env(arg_iter.next().unwrap(), env)?;
                match (chord1, chord2) {
                    (Value::Chord(c1), Value::Chord(c2)) => {
                        let vl = VoiceLeading::analyze(&c1, &c2);
                        Ok(Value::Chord(Chord::from_notes(vl.common_tones)))
                    }
                    _ => Err(anyhow!("ct() expects two chords")),
                }
            }),
        );

        self.register(
            "smooth_voice_leading",
            "Voice Leading",
            "Optimizes a pattern for smooth voice leading.",
            "smooth_voice_leading(pattern: Pattern) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 1 {
                    return Err(anyhow!(
                        "smooth_voice_leading() expects 1 argument, got {}",
                        args.len()
                    ));
                }

                let arg_value = evaluator.eval_with_env(args.into_iter().next().unwrap(), env)?;

                let pattern = match arg_value {
                    Value::Pattern(p) => p,
                    _ => return Err(anyhow!("smooth_voice_leading() only works on patterns")),
                };

                // Save original timing/envelope before optimization
                let original_beats_per_cycle = pattern.beats_per_cycle;
                let original_envelope = pattern.envelope;

                let optimized = pattern.optimize_voice_leading();

                let mut result_pattern = optimized;
                result_pattern.beats_per_cycle = original_beats_per_cycle;
                result_pattern.envelope = original_envelope;
                Ok(Value::Pattern(result_pattern))
            }),
        );

        self.register(
            "smooth",
            "Voice Leading",
            "Alias for smooth_voice_leading.",
            "smooth(pattern: Pattern) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 1 {
                    return Err(anyhow!("smooth() expects 1 argument"));
                }
                let arg_value = evaluator.eval_with_env(args.into_iter().next().unwrap(), env)?;
                let pattern = match arg_value {
                    Value::Pattern(p) => p,
                    _ => return Err(anyhow!("smooth() only works on patterns")),
                };
                // Save original timing/envelope before optimization
                let original_beats_per_cycle = pattern.beats_per_cycle;
                let original_envelope = pattern.envelope;
                let optimized = pattern.optimize_voice_leading();
                let mut result = optimized;
                result.beats_per_cycle = original_beats_per_cycle;
                result.envelope = original_envelope;
                Ok(Value::Pattern(result))
            }),
        );

        self.register(
            "analyze_voice_leading",
            "Voice Leading",
            "Analyzes the voice leading of a progression.",
            "analyze_voice_leading(progression: Pattern) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 1 {
                    return Err(anyhow!(
                        "analyze_voice_leading() expects 1 argument, got {}",
                        args.len()
                    ));
                }

                let arg_value = evaluator.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Pattern(progression) => {
                        let analysis = progression.detailed_voice_leading_analysis();

                        println!("Voice Leading Analysis:");
                        println!("======================");
                        for analysis_item in &analysis {
                            println!("{}", analysis_item);
                            println!("  {}", analysis_item.voice_leading);
                        }

                        let avg_quality = progression.average_voice_leading_quality();
                        let has_good_vl = progression.has_good_voice_leading();

                        println!("\nOverall Analysis:");
                        println!("  Average quality score: {:.1}", avg_quality);
                        println!(
                            "  Good voice leading: {}",
                            if has_good_vl {
                                "✓ Yes"
                            } else {
                                "⚠ Needs work"
                            }
                        );

                        Ok(Value::Pattern(progression))
                    }
                    _ => Err(anyhow!(
                        "analyze_voice_leading() only works on progressions"
                    )),
                }
            }),
        );

        self.register(
            "voice_leading_quality",
            "Voice Leading",
            "Returns the voice leading quality score.",
            "voice_leading_quality(progression: Pattern) -> Note",
            Arc::new(|evaluator, args, env| {
                if args.len() != 1 {
                    return Err(anyhow!(
                        "voice_leading_quality() expects 1 argument, got {}",
                        args.len()
                    ));
                }

                let arg_value = evaluator.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Pattern(progression) => {
                        let quality = progression.average_voice_leading_quality();
                        println!("Voice leading quality score: {:.1}", quality);

                        let quality_note = Note::new((quality.abs() as u8) % 12)?;
                        Ok(Value::Note(quality_note))
                    }
                    _ => Err(anyhow!(
                        "voice_leading_quality() only works on progressions"
                    )),
                }
            }),
        );

        // Progressions

        self.register(
            "roman_numeral",
            "Analysis",
            "Performs Roman Numeral Analysis on a chord in a key.",
            "roman_numeral(chord: Chord, key: Note) -> Chord",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("roman_numeral() expects 2 arguments: chord, key"));
                }

                let chord_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let key_value = evaluator.eval_with_env(args[1].clone(), env)?;

                match (chord_value, key_value) {
                    (Value::Chord(chord), Value::Note(key)) => {
                        match RomanNumeral::analyze_with_suggestions(&chord, key) {
                            Ok(analysis) => {
                                println!("{}", analysis.detailed_analysis());
                                Ok(Value::Chord(chord))
                            }
                            Err(e) => {
                                println!("Analysis failed: {}", e);
                                match RomanNumeral::analyze_with_context(&chord, key) {
                                    Ok(analyses) => {
                                        println!("Multiple interpretations found:");
                                        for (i, analysis) in analyses.iter().enumerate() {
                                            println!(
                                                "  {}: {}",
                                                i + 1,
                                                analysis.detailed_analysis()
                                            );
                                        }
                                        Ok(Value::Chord(chord))
                                    }
                                    Err(_) => Err(e),
                                }
                            }
                        }
                    }
                    _ => Err(anyhow!("roman_numeral() expects (chord, key)")),
                }
            }),
        );

        self.register(
            "rn",
            "Analysis",
            "Alias for roman_numeral.",
            "rn(chord: Chord, key: Note) -> Chord",
            Arc::new(|evaluator, args, env| {
                // Duplicate logic for alias
                if args.len() != 2 {
                    return Err(anyhow!("rn() expects 2 arguments: chord, key"));
                }
                let chord_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let key_value = evaluator.eval_with_env(args[1].clone(), env)?;
                match (chord_value, key_value) {
                    (Value::Chord(chord), Value::Note(key)) => {
                        match RomanNumeral::analyze_with_suggestions(&chord, key) {
                            Ok(a) => {
                                println!("{}", a.detailed_analysis());
                                Ok(Value::Chord(chord))
                            }
                            Err(_) => {
                                // Simple failover logic for brevity in alias
                                Err(anyhow!("Analysis failed"))
                            }
                        }
                    }
                    _ => Err(anyhow!("rn() expects (chord, key)")),
                }
            }),
        );

        self.register(
            "progression",
            "Progression",
            "Generates a chord progression by name and key.",
            "progression(name: String, key: Note) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() == 2 {
                    let prog_name = match &args[0] {
                        Expression::FunctionCall {
                            name,
                            args: inner_args,
                        } if inner_args.is_empty() => {
                            if CommonProgressions::is_roman_numeral_progression(name) {
                                name.clone()
                            } else {
                                name.replace("_", "-")
                            }
                        }
                        _ => return Err(anyhow!("progression() expects (progression_name, key)")),
                    };

                    let key_value = evaluator.eval_with_env(args[1].clone(), env)?;
                    if let Value::Note(key) = key_value {
                        let underscore_name = prog_name.replace("-", "_");
                        let pattern = CommonProgressions::get_progression(&prog_name, key)
                            .or_else(|_| {
                                CommonProgressions::get_progression(&underscore_name, key)
                            })?;

                        Ok(Value::Pattern(pattern))
                    } else {
                        Err(anyhow!("progression() expects (name, key)"))
                    }
                } else {
                    Err(anyhow!("progression() expects 2 arguments: name, key"))
                }
            }),
        );

        self.register(
            "list_progressions",
            "Progression",
            "Lists all available common progressions.",
            "list_progressions() -> Pattern",
            Arc::new(|_evaluator, args, _env| {
                if !args.is_empty() {
                    return Err(anyhow!("list_progressions() takes no arguments"));
                }

                println!("Available progressions:");
                for prog in CommonProgressions::list_progressions() {
                    println!("  {}", prog);
                }
                println!("\nUsage examples:");
                println!("  I_V_vi_IV(C)              # Named progression");
                println!("  I-V-vi-IV(C)              # Roman numeral progression");
                println!("  1564(C)                   # Numeric progression");
                println!("  progression(I-V-vi-IV, C) # Function call");

                Ok(Value::Pattern(crate::types::Pattern::new()))
            }),
        );

        self.register(
            "analyze_progression",
            "Analysis",
            "Analyzes a progression in a given key.",
            "analyze_progression(progression: Pattern, key: Note) -> Pattern",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "analyze_progression() expects 2 arguments: progression, key"
                    ));
                }

                let prog_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let key_value = evaluator.eval_with_env(args[1].clone(), env)?;

                match (prog_value, key_value) {
                    (Value::Pattern(progression), Value::Note(key)) => {
                        match analyze_progression(&progression, key) {
                            Ok(analysis) => {
                                println!("Roman Numeral Analysis in {} major:", key);
                                for (i, rn) in analysis.iter().enumerate() {
                                    println!("  {}: {} ({})", i + 1, rn, rn.function_description());
                                }

                                if progression.len() > 1 {
                                    let vl_quality = progression.average_voice_leading_quality();
                                    println!("\nVoice leading quality: {:.1}", vl_quality);
                                }
                            }
                            Err(e) => {
                                println!("Analysis failed: {}", e);
                                println!(
                                    "Try analyzing in a different key or check chord spellings."
                                );
                            }
                        }

                        Ok(Value::Pattern(progression))
                    }
                    _ => Err(anyhow!("analyze_progression() expects (progression, key)")),
                }
            }),
        );

        // --- Keywords (Documentation Only) ---

        let dummy_handler: BuiltinHandler =
            Arc::new(|_, _, _| Err(anyhow!("This is a keyword, not a function")));

        self.register(
            "tempo",
            "Keyword",
            "Sets the global tempo in BPM.",
            "tempo <bpm>",
            dummy_handler.clone(),
        );

        self.register(
            "play",
            "Keyword",
            "Plays a pattern, chord, or note on a track.",
            "play <expression> [loop] [queue]",
            dummy_handler.clone(),
        );

        self.register(
            "volume",
            "Keyword",
            "Sets the volume for the current track (0-100).",
            "volume <level>",
            dummy_handler.clone(),
        );

        self.register(
            "waveform",
            "Keyword",
            "Sets the waveform for the current track.",
            "waveform <name> (sine, saw, square, triangle)",
            dummy_handler.clone(),
        );

        self.register(
            "loop",
            "Keyword",
            "Infinite loop. Use 'break' to exit.",
            "loop { <statements> }",
            dummy_handler.clone(),
        );

        self.register(
            "repeat",
            "Keyword",
            "Repeats statements a fixed number of times.",
            "repeat <count> { <statements> }",
            dummy_handler.clone(),
        );

        self.register(
            "if",
            "Keyword",
            "Conditional execution.",
            "if <condition> { <statements> } [else { <statements> }]",
            dummy_handler.clone(),
        );

        self.register(
            "else",
            "Keyword",
            "Alternative branch for if statement.",
            "if <condition> { ... } else { <statements> }",
            dummy_handler.clone(),
        );

        self.register(
            "let",
            "Keyword",
            "Declares a new variable.",
            "let <name> = <expression>",
            dummy_handler.clone(),
        );

        self.register(
            "fn",
            "Keyword",
            "Defines a user function.",
            "fn <name>(<params>) { <statements>; return <expr> }",
            dummy_handler.clone(),
        );

        self.register(
            "on",
            "Keyword",
            "Schedules playback on a specific track.",
            "on <track_id> { <statements> } or on <track_id> play <expr>",
            dummy_handler.clone(),
        );

        self.register(
            "track",
            "Keyword",
            "Alias for 'on'. Schedules playback on a specific track.",
            "track <track_id> { <statements> }",
            dummy_handler.clone(),
        );

        self.register(
            "stop",
            "Keyword",
            "Stops playback. In a track block, stops that track only.",
            "stop",
            dummy_handler.clone(),
        );

        self.register(
            "queue",
            "Keyword",
            "Queues a pattern to start on the next beat/bar/cycle.",
            "play <expr> queue [beat|bar|cycle|<n>] loop",
            dummy_handler.clone(),
        );

        self.register(
            "load",
            "Keyword",
            "Loads and executes a Cadence script file.",
            "load \"<filepath>\"",
            dummy_handler.clone(),
        );

        self.register(
            "use",
            "Keyword",
            "Imports definitions from another Cadence module.",
            "use \"<filepath>\" [as <alias>] or use { <names> } from \"<filepath>\"",
            dummy_handler.clone(),
        );

        self.register(
            "break",
            "Keyword",
            "Exits the current loop early.",
            "break",
            dummy_handler.clone(),
        );

        self.register(
            "continue",
            "Keyword",
            "Skips to the next iteration of a loop.",
            "continue",
            dummy_handler.clone(),
        );

        self.register(
            "return",
            "Keyword",
            "Returns a value from a function.",
            "return <expression>",
            dummy_handler.clone(),
        );

        // Also register 'wave' as a function since it's used as .wave()
        self.register(
            "wave",
            "Audio",
            "Sets the waveform for a pattern.",
            "pattern.wave(name)",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("wave() expects 2 arguments: pattern, name"));
                }
                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let name_value = evaluator.eval_with_env(args[1].clone(), env)?;

                let mut pattern = match pattern_value {
                    Value::Pattern(p) => p,
                    _ => return Err(anyhow!("wave() first argument must be a pattern")),
                };

                let wave_name = match name_value {
                    Value::String(s) => s,
                    _ => return Err(anyhow!("wave() expects a string name")),
                };

                let waveform = crate::types::Waveform::from_str(&wave_name)
                    .ok_or_else(|| anyhow!("Unknown waveform: {}", wave_name))?;

                pattern.waveform = Some(waveform);
                Ok(Value::Pattern(pattern))
            }),
        );

        // Pan function for stereo positioning
        self.register(
            "pan",
            "Audio",
            "Sets the stereo pan for a pattern (0=left, 50=center, 100=right).",
            "pattern.pan(value)",
            Arc::new(|evaluator, args, env| {
                if args.len() != 2 {
                    return Err(anyhow!("pan() expects 2 arguments: pattern, value"));
                }
                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;
                let pan_value = evaluator.eval_with_env(args[1].clone(), env)?;

                let mut pattern = match pattern_value {
                    Value::Pattern(p) => p,
                    _ => return Err(anyhow!("pan() first argument must be a pattern")),
                };

                let pan = match pan_value {
                    Value::Number(n) => (n as f32 / 100.0).clamp(0.0, 1.0),
                    // Small numbers (0-11) are parsed as notes, extract pitch class
                    Value::Note(n) => (n.pitch_class() as f32 / 100.0).clamp(0.0, 1.0),
                    _ => return Err(anyhow!("pan() expects a number (0-100)")),
                };

                pattern.pan = Some(pan);
                Ok(Value::Pattern(pattern))
            }),
        );

        self.register(
            "env",
            "Audio",
            "Sets the ADSR envelope for a pattern using preset or custom values.",
            "pattern.env(\"preset\") or pattern.env(attack, decay, sustain, release)",
            Arc::new(|evaluator, args, env| {
                if args.is_empty() || args.len() > 5 {
                    return Err(anyhow!(
                        "env() expects 1-5 arguments: pattern, [attack, decay, sustain, release] or pattern, preset_name"
                    ));
                }

                let pattern_value = evaluator.eval_with_env(args[0].clone(), env)?;

                // Helper to apply envelope to a pattern
                let apply_env = |p: crate::types::Pattern, preset: Option<&str>, adsr: Option<(f32, f32, f32, f32)>| -> crate::types::Pattern {
                    match (preset, adsr) {
                        (Some(name), _) => p.env_preset(name),
                        (_, Some((a, d, s, r))) => p.env(a, d, s, r),
                        _ => p,
                    }
                };

                if args.len() == 2 {
                    // Preset mode: env(pattern, "pluck")
                    let preset_val = evaluator.eval_with_env(args[1].clone(), env)?;
                    let preset_name = match preset_val {
                        Value::String(s) => s,
                        _ => return Err(anyhow!("env() with 2 arguments expects a preset name string")),
                    };

                    match pattern_value {
                        Value::Pattern(p) => Ok(Value::Pattern(apply_env(p, Some(&preset_name), None))),
                        Value::EveryPattern(every) => {
                            let env_every = crate::types::EveryPattern::new(
                                every.interval,
                                apply_env(every.base.clone(), Some(&preset_name), None),
                                apply_env(every.transformed.clone(), Some(&preset_name), None),
                            );
                            Ok(Value::EveryPattern(Box::new(env_every)))
                        }
                        _ => Err(anyhow!("env() first argument must be a pattern")),
                    }
                } else if args.len() == 5 {
                    // Custom ADSR: env(pattern, attack, decay, sustain, release)
                    let attack = match evaluator.eval_with_env(args[1].clone(), env)? {
                        Value::Number(n) => n as f32 / 100.0,
                        Value::Note(n) => n.pitch_class() as f32 / 100.0,
                        _ => return Err(anyhow!("env() attack must be a number")),
                    };
                    let decay = match evaluator.eval_with_env(args[2].clone(), env)? {
                        Value::Number(n) => n as f32 / 100.0,
                        Value::Note(n) => n.pitch_class() as f32 / 100.0,
                        _ => return Err(anyhow!("env() decay must be a number")),
                    };
                    let sustain = match evaluator.eval_with_env(args[3].clone(), env)? {
                        Value::Number(n) => (n as f32 / 100.0).clamp(0.0, 1.0),
                        Value::Note(n) => (n.pitch_class() as f32 / 12.0).clamp(0.0, 1.0),
                        _ => return Err(anyhow!("env() sustain must be a number")),
                    };
                    let release = match evaluator.eval_with_env(args[4].clone(), env)? {
                        Value::Number(n) => n as f32 / 100.0,
                        Value::Note(n) => n.pitch_class() as f32 / 100.0,
                        _ => return Err(anyhow!("env() release must be a number")),
                    };

                    match pattern_value {
                        Value::Pattern(p) => Ok(Value::Pattern(apply_env(p, None, Some((attack, decay, sustain, release))))),
                        Value::EveryPattern(every) => {
                            let env_every = crate::types::EveryPattern::new(
                                every.interval,
                                apply_env(every.base.clone(), None, Some((attack, decay, sustain, release))),
                                apply_env(every.transformed.clone(), None, Some((attack, decay, sustain, release))),
                            );
                            Ok(Value::EveryPattern(Box::new(env_every)))
                        }
                        _ => Err(anyhow!("env() first argument must be a pattern")),
                    }
                } else {
                    Err(anyhow!(
                        "env() expects either (pattern, preset_name) or (pattern, a, d, s, r)"
                    ))
                }
            }),
        );
    }
}
