//! Environment for variable scopes
//!
//! Provides scoped variable storage with support for nested scopes.
//! Used by the Interpreter to store variable bindings.

use crate::parser::ast::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Thread-safe shared environment for live-coding reactivity
/// This allows the playback thread to read variable values that may be updated
/// by the main thread during playback.
pub type SharedEnvironment = Arc<RwLock<Environment>>;

/// Scoped environment for variable storage
#[derive(Debug)]
pub struct Environment {
    /// Stack of scopes (inner scopes shadow outer ones)
    scopes: Vec<HashMap<String, Value>>,
}

impl Environment {
    /// Create a new environment with a global scope
    pub fn new() -> Self {
        Environment {
            scopes: vec![HashMap::new()],
        }
    }

    /// Clear all scopes and reset to a fresh global scope
    /// Used when reloading a script to ensure no stale imports/variables persist
    pub fn clear(&mut self) {
        self.scopes.clear();
        self.scopes.push(HashMap::new());
    }

    /// Push a new scope (e.g., when entering a block)
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the current scope (e.g., when exiting a block)
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
        // Never pop the global scope
    }

    /// Define a new variable in the current scope
    pub fn define(&mut self, name: String, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, value);
        }
    }

    /// Get a variable's value (searches from inner to outer scopes)
    pub fn get(&self, name: &str) -> Option<&Value> {
        // Search from innermost to outermost scope
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(value);
            }
        }
        None
    }

    /// Set a variable's value (updates in the scope where it's defined)
    pub fn set(&mut self, name: &str, value: Value) -> Result<(), String> {
        // Search from innermost to outermost scope
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return Ok(());
            }
        }
        Err(format!("Variable '{}' is not defined", name))
    }

    /// Check if a variable is defined in any scope
    pub fn is_defined(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Get all variable names in the current scope
    pub fn current_scope_names(&self) -> Vec<&String> {
        if let Some(scope) = self.scopes.last() {
            scope.keys().collect()
        } else {
            Vec::new()
        }
    }

    /// Get all variable names across all scopes (for debugging)
    pub fn all_names(&self) -> Vec<&String> {
        self.scopes.iter().flat_map(|scope| scope.keys()).collect()
    }

    /// Get all bindings across all scopes (for introspection/hover)
    /// Returns deduplicated bindings, with inner scopes shadowing outer ones
    pub fn all_bindings(&self) -> Vec<(&String, &Value)> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        // Iterate from innermost to outermost (rev) to respect shadowing
        for scope in self.scopes.iter().rev() {
            for (name, value) in scope.iter() {
                if seen.insert(name) {
                    result.push((name, value));
                }
            }
        }
        result
    }

    /// Current scope depth (1 = global only)
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Note;

    fn make_note_value(name: &str) -> Value {
        Value::Note(name.parse::<Note>().unwrap())
    }

    #[test]
    fn test_basic_define_and_get() {
        let mut env = Environment::new();

        env.define("x".to_string(), make_note_value("C"));

        assert!(env.is_defined("x"));
        assert!(!env.is_defined("y"));

        let val = env.get("x").unwrap();
        assert!(matches!(val, Value::Note(_)));
    }

    #[test]
    fn test_scope_shadowing() {
        let mut env = Environment::new();

        env.define("x".to_string(), make_note_value("C"));

        // Enter new scope
        env.push_scope();
        env.define("x".to_string(), make_note_value("D")); // Shadows outer x

        // Should get the inner value
        let val = env.get("x").unwrap();
        match val {
            Value::Note(n) => assert_eq!(n.pitch_class(), 2), // D = 2
            _ => panic!("Expected Note"),
        }

        // Exit inner scope
        env.pop_scope();

        // Should get the outer value again
        let val = env.get("x").unwrap();
        match val {
            Value::Note(n) => assert_eq!(n.pitch_class(), 0), // C = 0
            _ => panic!("Expected Note"),
        }
    }

    #[test]
    fn test_scope_access_outer() {
        let mut env = Environment::new();

        env.define("outer".to_string(), make_note_value("C"));

        env.push_scope();
        env.define("inner".to_string(), make_note_value("D"));

        // Can access both from inner scope
        assert!(env.is_defined("outer"));
        assert!(env.is_defined("inner"));

        env.pop_scope();

        // Only outer is accessible now
        assert!(env.is_defined("outer"));
        assert!(!env.is_defined("inner"));
    }

    #[test]
    fn test_set_variable() {
        let mut env = Environment::new();

        env.define("x".to_string(), make_note_value("C"));

        // Set to new value
        env.set("x", make_note_value("E")).unwrap();

        let val = env.get("x").unwrap();
        match val {
            Value::Note(n) => assert_eq!(n.pitch_class(), 4), // E = 4
            _ => panic!("Expected Note"),
        }
    }

    #[test]
    fn test_set_undefined_fails() {
        let mut env = Environment::new();

        let result = env.set("undefined", make_note_value("C"));
        assert!(result.is_err());
    }

    #[test]
    fn test_depth() {
        let mut env = Environment::new();

        assert_eq!(env.depth(), 1); // Global scope

        env.push_scope();
        assert_eq!(env.depth(), 2);

        env.push_scope();
        assert_eq!(env.depth(), 3);

        env.pop_scope();
        assert_eq!(env.depth(), 2);

        env.pop_scope();
        assert_eq!(env.depth(), 1);

        // Can't pop global scope
        env.pop_scope();
        assert_eq!(env.depth(), 1);
    }
}
