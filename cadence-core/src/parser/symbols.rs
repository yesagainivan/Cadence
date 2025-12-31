//! Symbol Table for Language Service Features
//!
//! Provides a symbol table that's rebuilt on every parse,
//! enabling reactive hover, autocomplete, and diagnostics.

use std::collections::HashMap;

/// Source location span (compatible with JavaScript UTF-16 offsets)
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    /// Byte offset start
    pub start: usize,
    /// Byte offset end
    pub end: usize,
    /// UTF-16 code unit offset (for JavaScript interop)
    pub utf16_start: usize,
    /// UTF-16 code unit end
    pub utf16_end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span {
            start,
            end,
            utf16_start: 0,
            utf16_end: 0,
        }
    }

    pub fn with_utf16(start: usize, end: usize, utf16_start: usize, utf16_end: usize) -> Self {
        Span {
            start,
            end,
            utf16_start,
            utf16_end,
        }
    }

    /// Check if a UTF-16 position is within this span
    pub fn contains_utf16(&self, pos: usize) -> bool {
        pos >= self.utf16_start && pos <= self.utf16_end
    }
}

/// A user-defined function symbol
#[derive(Debug, Clone)]
pub struct FunctionSymbol {
    /// Function name
    pub name: String,
    /// Parameter names
    pub params: Vec<String>,
    /// Where the function is defined
    pub span: Span,
    /// Doc comment (from /// lines)
    pub doc_comment: Option<String>,
    /// Return type annotation (from -> Type)
    pub return_type: Option<String>,
}

impl FunctionSymbol {
    pub fn new(name: String, params: Vec<String>, span: Span) -> Self {
        FunctionSymbol {
            name,
            params,
            span,
            doc_comment: None,
            return_type: None,
        }
    }

    /// Get the function signature (e.g., "fn major(root)" or "fn major(root) -> Chord")
    pub fn signature(&self) -> String {
        let base = format!("fn {}({})", self.name, self.params.join(", "));
        match &self.return_type {
            Some(rt) => format!("{} -> {}", base, rt),
            None => base,
        }
    }
}

/// A variable binding symbol
#[derive(Debug, Clone)]
pub struct VariableSymbol {
    /// Variable name
    pub name: String,
    /// Inferred or annotated type (optional)
    pub value_type: Option<String>,
    /// Where the variable is defined
    pub span: Span,
    /// Doc comment (from preceding /// lines)
    pub doc_comment: Option<String>,
}

impl VariableSymbol {
    pub fn new(name: String, span: Span) -> Self {
        VariableSymbol {
            name,
            span,
            value_type: None,
            doc_comment: None,
        }
    }

    pub fn with_type(name: String, value_type: String, span: Span) -> Self {
        VariableSymbol {
            name,
            span,
            value_type: Some(value_type),
            doc_comment: None,
        }
    }

    /// Builder method to add inferred type
    pub fn with_inferred_type(mut self, value_type: Option<String>) -> Self {
        self.value_type = value_type;
        self
    }

    /// Builder method to add doc comment
    pub fn with_doc_comment(mut self, doc_comment: Option<String>) -> Self {
        self.doc_comment = doc_comment;
        self
    }
}

/// Symbol kinds for generic queries
#[derive(Debug, Clone)]
pub enum Symbol {
    Function(FunctionSymbol),
    Variable(VariableSymbol),
}

impl Symbol {
    pub fn name(&self) -> &str {
        match self {
            Symbol::Function(f) => &f.name,
            Symbol::Variable(v) => &v.name,
        }
    }

    pub fn span(&self) -> &Span {
        match self {
            Symbol::Function(f) => &f.span,
            Symbol::Variable(v) => &v.span,
        }
    }
}

/// The Symbol Table - source of truth for defined symbols
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    /// User-defined functions
    pub functions: HashMap<String, FunctionSymbol>,
    /// Variable bindings
    pub variables: HashMap<String, VariableSymbol>,
}

impl SymbolTable {
    pub fn new() -> Self {
        SymbolTable {
            functions: HashMap::new(),
            variables: HashMap::new(),
        }
    }

    /// Add a function symbol
    pub fn add_function(&mut self, func: FunctionSymbol) {
        self.functions.insert(func.name.clone(), func);
    }

    /// Add a variable symbol
    pub fn add_variable(&mut self, var: VariableSymbol) {
        self.variables.insert(var.name.clone(), var);
    }

    /// Get a function by name
    pub fn get_function(&self, name: &str) -> Option<&FunctionSymbol> {
        self.functions.get(name)
    }

    /// Get a variable by name
    pub fn get_variable(&self, name: &str) -> Option<&VariableSymbol> {
        self.variables.get(name)
    }

    /// Get any symbol by name (function first, then variable)
    pub fn get(&self, name: &str) -> Option<Symbol> {
        if let Some(f) = self.functions.get(name) {
            return Some(Symbol::Function(f.clone()));
        }
        if let Some(v) = self.variables.get(name) {
            return Some(Symbol::Variable(v.clone()));
        }
        None
    }

    /// Find symbol at a UTF-16 position (for hover)
    pub fn get_at_position(&self, pos: usize) -> Option<Symbol> {
        // Check functions first
        for func in self.functions.values() {
            if func.span.contains_utf16(pos) {
                return Some(Symbol::Function(func.clone()));
            }
        }
        // Then variables
        for var in self.variables.values() {
            if var.span.contains_utf16(pos) {
                return Some(Symbol::Variable(var.clone()));
            }
        }
        None
    }

    /// Get all function symbols
    pub fn all_functions(&self) -> impl Iterator<Item = &FunctionSymbol> {
        self.functions.values()
    }

    /// Get all variable symbols
    pub fn all_variables(&self) -> impl Iterator<Item = &VariableSymbol> {
        self.variables.values()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty() && self.variables.is_empty()
    }

    /// Total symbol count
    pub fn len(&self) -> usize {
        self.functions.len() + self.variables.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_symbol() {
        let span = Span::with_utf16(0, 50, 0, 50);
        let func = FunctionSymbol::new("major".to_string(), vec!["root".to_string()], span);

        assert_eq!(func.name, "major");
        assert_eq!(func.params, vec!["root"]);
        assert_eq!(func.signature(), "fn major(root)");
    }

    #[test]
    fn test_symbol_table_add_get() {
        let mut table = SymbolTable::new();

        let func = FunctionSymbol::new(
            "major".to_string(),
            vec!["root".to_string()],
            Span::with_utf16(0, 50, 0, 50),
        );
        table.add_function(func);

        let var = VariableSymbol::new("Cmaj".to_string(), Span::with_utf16(52, 65, 52, 65));
        table.add_variable(var);

        assert!(table.get_function("major").is_some());
        assert!(table.get_variable("Cmaj").is_some());
        assert!(table.get("major").is_some());
        assert!(table.get("Cmaj").is_some());
        assert!(table.get("nonexistent").is_none());
    }

    #[test]
    fn test_get_at_position() {
        let mut table = SymbolTable::new();

        let func = FunctionSymbol::new(
            "major".to_string(),
            vec!["root".to_string()],
            Span::with_utf16(0, 50, 0, 50),
        );
        table.add_function(func);

        // Position inside function span
        let symbol = table.get_at_position(25);
        assert!(symbol.is_some());
        assert_eq!(symbol.unwrap().name(), "major");

        // Position outside
        let symbol = table.get_at_position(100);
        assert!(symbol.is_none());
    }
}
