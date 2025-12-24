//! Interpreter for executing Cadence programs
//!
//! Executes statements with side effects (audio, variable binding, control flow).

use crate::parser::ast::{Program, Statement, Value};
use crate::parser::environment::Environment;
use crate::parser::evaluator::Evaluator;
use anyhow::{Result, anyhow};

/// Control flow signals for break/continue/return
#[derive(Debug)]
pub enum ControlFlow {
    Normal,
    Break,
    Continue,
    Return(Option<Value>),
}

/// Interpreter for executing Cadence statements
pub struct Interpreter {
    /// Expression evaluator
    evaluator: Evaluator,
    /// Variable environment
    pub environment: Environment,
    /// Current tempo (BPM)
    pub tempo: f32,
    /// Current volume (0.0-1.0)
    pub volume: f32,
    /// Last evaluated expression result
    last_eval_result: Option<Value>,
}

impl Interpreter {
    /// Create a new interpreter
    pub fn new() -> Self {
        Interpreter {
            evaluator: Evaluator::new(),
            environment: Environment::new(),
            tempo: 120.0,
            volume: 0.5,
            last_eval_result: None,
        }
    }

    /// Run a complete program
    pub fn run_program(&mut self, program: &Program) -> Result<Option<Value>> {
        let mut last_value = None;

        for stmt in &program.statements {
            match self.run_statement(stmt)? {
                ControlFlow::Normal => {}
                ControlFlow::Return(val) => return Ok(val),
                ControlFlow::Break => return Err(anyhow!("Break outside of loop")),
                ControlFlow::Continue => return Err(anyhow!("Continue outside of loop")),
            }

            // Capture last expression result
            if let Statement::Expression(_) = stmt {
                last_value = self.last_eval_result.take();
            }
        }

        Ok(last_value)
    }

    /// Run a single statement
    pub fn run_statement(&mut self, stmt: &Statement) -> Result<ControlFlow> {
        match stmt {
            Statement::Let { name, value } => {
                let val = self.eval_expression(value)?;
                self.environment.define(name.clone(), val);
                Ok(ControlFlow::Normal)
            }

            Statement::Expression(expr) => {
                let val = self.eval_expression(expr)?;
                println!("{}", val); // REPL-style: print expression results
                self.last_eval_result = Some(val);
                Ok(ControlFlow::Normal)
            }

            Statement::Tempo(bpm) => {
                self.tempo = *bpm;
                println!("Tempo set to {} BPM", bpm);
                Ok(ControlFlow::Normal)
            }

            Statement::Volume(vol) => {
                self.volume = *vol;
                println!("Volume set to {:.0}%", vol * 100.0);
                Ok(ControlFlow::Normal)
            }

            Statement::Stop => {
                println!("Stopping playback");
                Ok(ControlFlow::Normal)
            }

            Statement::Play {
                target,
                looping,
                queue: _,
                duration: _,
            } => {
                let val = self.eval_expression(target)?;
                if *looping {
                    println!("Playing {} (looping)", val);
                } else {
                    println!("Playing {}", val);
                }
                Ok(ControlFlow::Normal)
            }

            Statement::Load(path) => {
                println!("Loading file: {}", path);
                // TODO: Implement file loading
                Ok(ControlFlow::Normal)
            }

            Statement::Loop { body } => loop {
                for stmt in body {
                    match self.run_statement(stmt)? {
                        ControlFlow::Normal => {}
                        ControlFlow::Break => return Ok(ControlFlow::Normal),
                        ControlFlow::Continue => break,
                        ControlFlow::Return(val) => return Ok(ControlFlow::Return(val)),
                    }
                }
            },

            Statement::Repeat { count, body } => {
                for _ in 0..*count {
                    self.environment.push_scope();
                    for stmt in body {
                        match self.run_statement(stmt)? {
                            ControlFlow::Normal => {}
                            ControlFlow::Break => {
                                self.environment.pop_scope();
                                return Ok(ControlFlow::Normal);
                            }
                            ControlFlow::Continue => break,
                            ControlFlow::Return(val) => {
                                self.environment.pop_scope();
                                return Ok(ControlFlow::Return(val));
                            }
                        }
                    }
                    self.environment.pop_scope();
                }
                Ok(ControlFlow::Normal)
            }

            Statement::If {
                condition,
                then_body,
                else_body,
            } => {
                let cond_val = self.eval_expression(condition)?;
                let is_true = match cond_val {
                    Value::Boolean(b) => b,
                    _ => return Err(anyhow!("Condition must be a boolean")),
                };

                let body = if is_true {
                    then_body
                } else {
                    match else_body {
                        Some(b) => b,
                        None => return Ok(ControlFlow::Normal),
                    }
                };

                self.environment.push_scope();
                for stmt in body {
                    match self.run_statement(stmt)? {
                        ControlFlow::Normal => {}
                        cf => {
                            self.environment.pop_scope();
                            return Ok(cf);
                        }
                    }
                }
                self.environment.pop_scope();
                Ok(ControlFlow::Normal)
            }

            Statement::Break => Ok(ControlFlow::Break),
            Statement::Continue => Ok(ControlFlow::Continue),
            Statement::Return(expr) => {
                let val = match expr {
                    Some(e) => Some(self.eval_expression(e)?),
                    None => None,
                };
                Ok(ControlFlow::Return(val))
            }

            Statement::Comment(_) => Ok(ControlFlow::Normal),

            Statement::Block(stmts) => {
                self.environment.push_scope();
                for stmt in stmts {
                    match self.run_statement(stmt)? {
                        ControlFlow::Normal => {}
                        cf => {
                            self.environment.pop_scope();
                            return Ok(cf);
                        }
                    }
                }
                self.environment.pop_scope();
                Ok(ControlFlow::Normal)
            }
        }
    }

    /// Evaluate an expression using the environment
    fn eval_expression(&self, expr: &crate::parser::ast::Expression) -> Result<Value> {
        // For now, delegate to the evaluator
        // In the future, this will handle variable lookup
        self.evaluator.eval(expr.clone())
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::statement_parser::parse_statements;

    #[test]
    fn test_let_and_tempo() {
        let mut interpreter = Interpreter::new();
        // Note: Using 120 because lexer uses i8 for numbers (max 127)
        let program = parse_statements("tempo 120").unwrap();
        interpreter.run_program(&program).unwrap();

        assert_eq!(interpreter.tempo, 120.0);
    }

    #[test]
    fn test_volume() {
        let mut interpreter = Interpreter::new();
        let program = parse_statements("volume 75").unwrap();
        interpreter.run_program(&program).unwrap();

        assert!((interpreter.volume - 0.75).abs() < 0.01);
    }
}
