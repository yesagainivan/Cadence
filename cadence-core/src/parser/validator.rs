use crate::parser::ast::{Expression, SpannedProgram, SpannedStatement, Statement};
use crate::parser::binder::Binder;
use crate::parser::error::CadenceError;
use crate::parser::lexer::Span;
use crate::types::{CommonProgressions, Pattern};

pub struct Validator<'a> {
    errors: Vec<CadenceError>,
    binder: &'a Binder,
}

impl<'a> Validator<'a> {
    pub fn new(binder: &'a Binder) -> Self {
        Validator {
            errors: Vec::new(),
            binder,
        }
    }

    pub fn validate(program: &SpannedProgram, binder: &'a Binder) -> Vec<CadenceError> {
        let mut validator = Validator::new(binder);
        validator.visit_program(program);
        validator.errors
    }

    fn visit_program(&mut self, program: &SpannedProgram) {
        for stmt in &program.statements {
            self.visit_statement(stmt);
        }
    }

    fn visit_statement(&mut self, stmt: &SpannedStatement) {
        let span = stmt.to_span();
        match &stmt.statement {
            Statement::Expression(expr) => self.visit_expression(expr, span),
            Statement::Let { value, .. } => self.visit_expression(value, span),
            Statement::Assign { value, .. } => self.visit_expression(value, span),
            Statement::FunctionDef { body, .. } => {
                // Validate function body
                for inner_stmt in body {
                    self.visit_unspanned_statement(inner_stmt, span.clone());
                }
            }
            Statement::If {
                condition,
                then_body,
                else_body,
            } => {
                self.visit_expression(condition, span.clone());
                // Validate then branch
                for inner_stmt in then_body {
                    self.visit_unspanned_statement(inner_stmt, span.clone());
                }
                // Validate else branch
                if let Some(else_stmts) = else_body {
                    for inner_stmt in else_stmts {
                        self.visit_unspanned_statement(inner_stmt, span.clone());
                    }
                }
            }
            Statement::Repeat { body, .. } | Statement::Loop { body } => {
                // Validate loop body
                for inner_stmt in body {
                    self.visit_unspanned_statement(inner_stmt, span.clone());
                }
            }
            Statement::For { body, .. } => {
                // Validate for loop body
                for inner_stmt in body {
                    self.visit_unspanned_statement(inner_stmt, span.clone());
                }
            }
            Statement::Block(body) => {
                // Validate block body
                for inner_stmt in body {
                    self.visit_unspanned_statement(inner_stmt, span.clone());
                }
            }
            Statement::Track { body, .. } => {
                // Validate track body (boxed statement)
                self.visit_unspanned_statement(body, span.clone());
            }
            Statement::Play { target, .. } => self.visit_expression(target, span),
            Statement::Tempo(expr) | Statement::Volume(expr) | Statement::Wait { beats: expr } => {
                self.visit_expression(expr, span);
            }
            Statement::Return(Some(expr)) => self.visit_expression(expr, span),
            _ => {}
        }
    }

    /// Visit an unspanned statement (used for nested bodies)
    /// Uses the parent span for error reporting
    fn visit_unspanned_statement(&mut self, stmt: &Statement, parent_span: Span) {
        match stmt {
            Statement::Expression(expr) => self.visit_expression(expr, parent_span),
            Statement::Let { value, .. } => self.visit_expression(value, parent_span),
            Statement::Assign { value, .. } => self.visit_expression(value, parent_span),
            Statement::FunctionDef { body, .. } => {
                for inner_stmt in body {
                    self.visit_unspanned_statement(inner_stmt, parent_span.clone());
                }
            }
            Statement::If {
                condition,
                then_body,
                else_body,
            } => {
                self.visit_expression(condition, parent_span.clone());
                for inner_stmt in then_body {
                    self.visit_unspanned_statement(inner_stmt, parent_span.clone());
                }
                if let Some(else_stmts) = else_body {
                    for inner_stmt in else_stmts {
                        self.visit_unspanned_statement(inner_stmt, parent_span.clone());
                    }
                }
            }
            Statement::Repeat { body, .. } | Statement::Loop { body } => {
                for inner_stmt in body {
                    self.visit_unspanned_statement(inner_stmt, parent_span.clone());
                }
            }
            Statement::For { body, .. } => {
                for inner_stmt in body {
                    self.visit_unspanned_statement(inner_stmt, parent_span.clone());
                }
            }
            Statement::Block(body) => {
                for inner_stmt in body {
                    self.visit_unspanned_statement(inner_stmt, parent_span.clone());
                }
            }
            Statement::Track { body, .. } => {
                self.visit_unspanned_statement(body, parent_span.clone());
            }
            Statement::Play { target, .. } => self.visit_expression(target, parent_span),
            Statement::Tempo(expr) | Statement::Volume(expr) | Statement::Wait { beats: expr } => {
                self.visit_expression(expr, parent_span);
            }
            Statement::Return(Some(expr)) => self.visit_expression(expr, parent_span),
            _ => {}
        }
    }

    fn visit_expression(&mut self, expr: &Expression, span: Span) {
        match expr {
            Expression::FunctionCall { name, args } => {
                self.check_function_call(name, args, span.clone());
                for arg in args {
                    self.visit_expression(arg, span.clone());
                }
            }
            Expression::BinaryOp { left, right, .. }
            | Expression::LogicalAnd { left, right }
            | Expression::LogicalOr { left, right }
            | Expression::Intersection { left, right }
            | Expression::Union { left, right }
            | Expression::Difference { left, right }
            | Expression::Comparison { left, right, .. } => {
                self.visit_expression(left, span.clone());
                self.visit_expression(right, span.clone());
            }
            Expression::LogicalNot(expr) | Expression::Transpose { target: expr, .. } => {
                self.visit_expression(expr, span.clone());
            }
            Expression::Index { target, index } => {
                self.visit_expression(target, span.clone());
                self.visit_expression(index, span.clone());
            }
            Expression::Array(elements) => {
                for elem in elements {
                    self.visit_expression(elem, span.clone());
                }
            }
            // Pre-validate pattern strings for syntax errors
            Expression::String(s) => {
                self.check_pattern_string(s, span);
            }
            _ => {}
        }
    }

    fn check_function_call(&mut self, name: &str, args: &[Expression], span: Span) {
        // 1. Check user-defined functions (Binder)
        if let Some(symbol) = self.binder.table.get_function(name) {
            let expected = symbol.params.len();
            let got = args.len();
            if expected != got {
                self.errors.push(CadenceError::new(
                    format!(
                        "Function '{}' expects {} arguments, got {}",
                        name, expected, got
                    ),
                    span,
                ));
            }
            return;
        }

        // 2. Check built-in functions
        if let Some(builtin) = crate::parser::builtins::get_registry().get(name) {
            let valid_arities = builtin.valid_arities();
            let got = args.len();
            if !valid_arities.contains(&got) {
                // Determine error message format
                let msg = if valid_arities.len() == 1 {
                    format!(
                        "Function '{}' expects {} arguments, got {}",
                        name, valid_arities[0], got
                    )
                } else {
                    format!(
                        "Function '{}' expects {:?} arguments, got {}",
                        name, valid_arities, got
                    )
                };

                self.errors.push(CadenceError::new(msg, span));
            }
            return;
        }

        // 3. Check common progressions (I-IV-V etc)
        if CommonProgressions::is_valid_progression(name)
            || CommonProgressions::is_numeric_progression(name)
            || CommonProgressions::is_roman_numeral_progression(name)
        {
            if args.len() != 1 {
                self.errors.push(CadenceError::new(
                    format!("Progression '{}' expects 1 key argument", name),
                    span,
                ));
            }
            return;
        }
    }

    /// Pre-validate pattern strings for syntax errors
    /// This catches malformed patterns at parse time rather than runtime
    fn check_pattern_string(&mut self, s: &str, span: Span) {
        // Try to parse the string as a pattern
        // This will catch syntax errors like unclosed brackets, invalid notes, etc.
        // Variable references are allowed (they're resolved at runtime)
        if let Err(e) = Pattern::parse(s) {
            let msg = e.to_string();
            // Only report if it's a real syntax error, not a "single word" error
            // since single words might be valid function args like "pluck"
            if !msg.contains("Single word") {
                self.errors
                    .push(CadenceError::new(format!("Pattern error: {}", msg), span));
            }
        }
    }
}

trait ToSpan {
    fn to_span(&self) -> Span;
}

impl ToSpan for SpannedStatement {
    fn to_span(&self) -> Span {
        Span::full(
            0, // Line/Col not stored in SpannedStatement directly, simplified
            0,
            self.start,
            self.end - self.start,
            self.utf16_start,
            self.utf16_end - self.utf16_start,
        )
    }
}
