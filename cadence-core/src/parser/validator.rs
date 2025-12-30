use crate::parser::ast::{Expression, SpannedProgram, SpannedStatement, Statement};
use crate::parser::binder::Binder;
use crate::parser::error::CadenceError;
use crate::parser::lexer::Span;
use crate::types::CommonProgressions;

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
            Statement::FunctionDef { .. } => {
                // TODO: Validate body
            }
            Statement::If {
                condition,
                then_body,
                else_body,
            } => {
                self.visit_expression(condition, span);
                // TODO: visit body
            }
            Statement::Repeat { body, .. } | Statement::Loop { body } => {
                // TODO: visit body
                let _ = body;
            }
            Statement::Play { target, .. } => self.visit_expression(target, span),
            _ => {}
        }
    }

    fn visit_expression(&mut self, expr: &Expression, span: Span) {
        match expr {
            Expression::FunctionCall { name, args } => {
                self.check_function_call(name, args, span.clone());
                for arg in args {
                    self.visit_expression(arg, span.clone()); // Propagate span is approx
                }
            }
            Expression::BinaryOp { left, right, .. }
            | Expression::LogicalAnd { left, right }
            | Expression::LogicalOr { left, right }
            | Expression::Intersection { left, right }
            | Expression::Union { left, right }
            | Expression::Difference { left, right } => {
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
