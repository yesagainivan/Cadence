//! Binder - Walks the AST and populates the Symbol Table
//!
//! This is the bridge between parsing and language service features.
//! Call `bind()` after parsing to get a fresh SymbolTable.

use crate::parser::ast::{Expression, SpannedProgram, SpannedStatement, Statement};
use crate::parser::symbols::{FunctionSymbol, Span, SymbolTable, VariableSymbol};

/// Infer a type hint from an AST expression (without evaluation)
fn infer_type_from_expr(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Note(_) => Some("Note".to_string()),
        Expression::Chord(_) => Some("Chord".to_string()),
        Expression::Pattern(_) => Some("Pattern".to_string()),
        Expression::Number(_) => Some("Number".to_string()),
        Expression::Boolean(_) => Some("Boolean".to_string()),
        Expression::String(_) => Some("String".to_string()),
        Expression::Array(_) => Some("Chord".to_string()), // Arrays often become chords
        Expression::FunctionCall { name, .. } => {
            // Some built-in functions have known return types
            match name.as_str() {
                "major" | "minor" | "dim" | "aug" | "sus2" | "sus4" | "invert" | "bass" => {
                    Some("Chord".to_string())
                }
                "fast" | "slow" | "rev" | "every" => Some("Pattern".to_string()),
                "root" | "fifth" => Some("Note".to_string()),
                _ => None, // Unknown function, can't infer
            }
        }
        Expression::Transpose { target, .. } => infer_type_from_expr(target),
        _ => None,
    }
}

/// Binder walks the AST and extracts symbols
pub struct Binder {
    pub table: SymbolTable,
}

impl Binder {
    pub fn new() -> Self {
        Binder {
            table: SymbolTable::new(),
        }
    }

    /// Bind a SpannedProgram and return the SymbolTable
    pub fn bind(program: &SpannedProgram) -> SymbolTable {
        let mut binder = Binder::new();
        binder.bind_program(program);
        binder.table
    }

    fn bind_program(&mut self, program: &SpannedProgram) {
        for stmt in &program.statements {
            self.bind_statement(stmt);
        }
    }

    fn bind_statement(&mut self, spanned: &SpannedStatement) {
        let span = Span::with_utf16(
            spanned.start,
            spanned.end,
            spanned.utf16_start,
            spanned.utf16_end,
        );

        match &spanned.statement {
            Statement::FunctionDef { name, params, body } => {
                let mut func = FunctionSymbol::new(name.clone(), params.clone(), span);
                func.doc_comment = spanned.doc_comment.clone();
                self.table.add_function(func);

                // Bind nested statements inside the function body
                // (for nested function definitions)
                for inner_stmt in body {
                    // Create a temporary SpannedStatement for the inner stmt
                    // Note: In a real impl, body would also be SpannedStatements
                    // For now, we just capture top-level functions
                    let _ = inner_stmt; // TODO: handle nested properly
                }
            }

            Statement::Let { name, value } => {
                let inferred_type = infer_type_from_expr(value);
                let var = VariableSymbol::new(name.clone(), span)
                    .with_inferred_type(inferred_type)
                    .with_doc_comment(spanned.doc_comment.clone());
                self.table.add_variable(var);
            }

            Statement::Track { body, .. } => {
                // Recursively bind inside track blocks
                self.bind_inner_statement(body.as_ref());
            }

            Statement::Block(stmts) => {
                for stmt in stmts {
                    self.bind_inner_statement(stmt);
                }
            }

            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                for stmt in then_body {
                    self.bind_inner_statement(stmt);
                }
                if let Some(else_stmts) = else_body {
                    for stmt in else_stmts {
                        self.bind_inner_statement(stmt);
                    }
                }
            }

            Statement::Loop { body } | Statement::Repeat { body, .. } => {
                for stmt in body {
                    self.bind_inner_statement(stmt);
                }
            }

            Statement::For { body, .. } => {
                for stmt in body {
                    self.bind_inner_statement(stmt);
                }
            }

            // Other statements don't define symbols
            _ => {}
        }
    }

    /// Bind non-spanned statements (inside blocks, loops, etc.)
    /// Note: This is a simplified version that doesn't have precise spans
    fn bind_inner_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::FunctionDef { name, params, .. } => {
                // Use a placeholder span since we don't have precise spans for inner stmts
                let func = FunctionSymbol::new(name.clone(), params.clone(), Span::new(0, 0));
                self.table.add_function(func);
            }

            Statement::Let { name, .. } => {
                let var = VariableSymbol::new(name.clone(), Span::new(0, 0));
                self.table.add_variable(var);
            }

            Statement::Track { body, .. } => {
                self.bind_inner_statement(body.as_ref());
            }

            Statement::Block(stmts) => {
                for s in stmts {
                    self.bind_inner_statement(s);
                }
            }

            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                for s in then_body {
                    self.bind_inner_statement(s);
                }
                if let Some(else_stmts) = else_body {
                    for s in else_stmts {
                        self.bind_inner_statement(s);
                    }
                }
            }

            Statement::Loop { body } | Statement::Repeat { body, .. } => {
                for s in body {
                    self.bind_inner_statement(s);
                }
            }

            Statement::For { body, .. } => {
                for s in body {
                    self.bind_inner_statement(s);
                }
            }

            _ => {}
        }
    }
}

impl Default for Binder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::statement_parser::parse_spanned_statements;

    #[test]
    fn test_bind_function() {
        let code = r#"
fn major(root) {
    return [root, root + 4, root + 7]
}
"#;
        let program = parse_spanned_statements(code).unwrap();
        let table = Binder::bind(&program);

        assert_eq!(table.functions.len(), 1);
        let func = table.get_function("major").unwrap();
        assert_eq!(func.name, "major");
        assert_eq!(func.params, vec!["root"]);
    }

    #[test]
    fn test_bind_variable() {
        let code = "let Cmaj = [C, E, G]";
        let program = parse_spanned_statements(code).unwrap();
        let table = Binder::bind(&program);

        assert_eq!(table.variables.len(), 1);
        assert!(table.get_variable("Cmaj").is_some());
    }

    #[test]
    fn test_bind_multiple() {
        let code = r#"
fn major(root) {
    return [root, root + 4, root + 7]
}

fn minor(root) {
    return [root, root + 3, root + 7]
}

let Cmaj = major(C)
let Am = minor(A)
"#;
        let program = parse_spanned_statements(code).unwrap();
        let table = Binder::bind(&program);

        assert_eq!(table.functions.len(), 2);
        assert_eq!(table.variables.len(), 2);
        assert!(table.get_function("major").is_some());
        assert!(table.get_function("minor").is_some());
        assert!(table.get_variable("Cmaj").is_some());
        assert!(table.get_variable("Am").is_some());
    }

    #[test]
    fn test_commented_function_not_bound() {
        let code = r#"
// fn major(root) {}
fn minor(root) {
    return [root, root + 3, root + 7]
}
"#;
        let program = parse_spanned_statements(code).unwrap();
        let table = Binder::bind(&program);

        // Commented function should NOT be in the symbol table
        assert_eq!(table.functions.len(), 1);
        assert!(table.get_function("major").is_none());
        assert!(table.get_function("minor").is_some());
    }

    #[test]
    fn test_bind_function_with_doc_comment() {
        let code = r#"
/// Builds a major chord from root note
fn major(root) {
    return [root, root + 4, root + 7]
}
"#;
        let program = parse_spanned_statements(code).unwrap();
        let table = Binder::bind(&program);

        let func = table.get_function("major").unwrap();
        assert_eq!(
            func.doc_comment,
            Some("Builds a major chord from root note".to_string())
        );
    }

    #[test]
    fn test_bind_variable_with_doc_comment() {
        let code = r#"
/// The C major chord
let Cmaj = [C, E, G]
"#;
        let program = parse_spanned_statements(code).unwrap();
        let table = Binder::bind(&program);

        let var = table.get_variable("Cmaj").unwrap();
        assert_eq!(var.doc_comment, Some("The C major chord".to_string()));
    }

    #[test]
    fn test_multiline_doc_comment() {
        let code = r#"
/// Builds a major chord
/// @param root - the root note
fn major(root) {
    return [root, root + 4, root + 7]
}
"#;
        let program = parse_spanned_statements(code).unwrap();
        let table = Binder::bind(&program);

        let func = table.get_function("major").unwrap();
        assert_eq!(
            func.doc_comment,
            Some("Builds a major chord\n@param root - the root note".to_string())
        );
    }

    #[test]
    fn test_regular_comment_not_attached() {
        let code = r#"
// This is a regular comment
fn major(root) {
    return [root, root + 4, root + 7]
}
"#;
        let program = parse_spanned_statements(code).unwrap();
        let table = Binder::bind(&program);

        let func = table.get_function("major").unwrap();
        // Regular // comments should NOT be attached as doc comments
        assert!(func.doc_comment.is_none());
    }
}
