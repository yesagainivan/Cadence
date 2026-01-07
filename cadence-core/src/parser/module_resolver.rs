//! Module resolution for Cadence's `use` statement
//!
//! This module provides the infrastructure for loading and caching modules,
//! with support for both native filesystem and WASM (via callbacks).

use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::parser::ast::{Program, Statement};
use crate::parser::environment::Environment;
use crate::parser::statement_parser::parse_statements;
use crate::parser::Value;

/// Trait for abstracting file system access
/// This allows native filesystem access in CLI and callback-based access in WASM
pub trait FileProvider: Send + Sync {
    /// Read the contents of a file at the given path
    fn read_file(&self, path: &str) -> Result<String>;

    /// Resolve a relative path from a base path
    /// Returns the canonical path to the module
    fn resolve_path(&self, from: &str, import_path: &str) -> Result<String>;
}

/// Native filesystem provider for CLI/REPL
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeFileProvider {
    /// Base directory for resolving relative paths
    pub base_dir: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeFileProvider {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn from_current_dir() -> Result<Self> {
        Ok(Self {
            base_dir: std::env::current_dir()?,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl FileProvider for NativeFileProvider {
    fn read_file(&self, path: &str) -> Result<String> {
        std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read module '{}': {}", path, e))
    }

    fn resolve_path(&self, from: &str, import_path: &str) -> Result<String> {
        let from_path = Path::new(from);
        let base = from_path.parent().unwrap_or(Path::new("."));

        let resolved = if import_path.starts_with("./") || import_path.starts_with("../") {
            // Relative path
            base.join(import_path)
        } else {
            // Absolute or project-relative path
            self.base_dir.join(import_path)
        };

        // Canonicalize to get absolute path and resolve symlinks
        let canonical = resolved
            .canonicalize()
            .map_err(|e| anyhow!("Cannot resolve module path '{}': {}", import_path, e))?;

        canonical
            .to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Invalid UTF-8 in path"))
    }
}

/// Exported definitions from a module
#[derive(Debug, Clone, Default)]
pub struct ModuleExports {
    /// Exported variable values (from `let` statements)
    pub values: HashMap<String, Value>,
    /// Exported function definitions (name -> (params, body))
    pub functions: HashMap<String, (Vec<String>, Vec<Statement>)>,
}

impl ModuleExports {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get an exported value by name
    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(val) = self.values.get(name) {
            return Some(val.clone());
        }
        if let Some((params, body)) = self.functions.get(name) {
            return Some(Value::Function {
                name: name.to_string(),
                params: params.clone(),
                body: body.clone(),
            });
        }
        None
    }

    /// Check if a name is exported
    pub fn has(&self, name: &str) -> bool {
        self.values.contains_key(name) || self.functions.contains_key(name)
    }

    /// Get all exported names
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.values.keys().cloned().collect();
        names.extend(self.functions.keys().cloned());
        names.sort();
        names
    }
}

/// Module resolver with caching and circular import detection
pub struct ModuleResolver {
    /// Cache of already-resolved modules
    cache: HashMap<String, ModuleExports>,
    /// Stack to detect circular imports (paths currently being loaded)
    loading_stack: HashSet<String>,
    /// File system abstraction
    file_provider: Box<dyn FileProvider>,
    /// Current file being processed (for relative path resolution)
    current_file: Option<String>,
}

impl ModuleResolver {
    /// Create a new module resolver with the given file provider
    pub fn new(file_provider: Box<dyn FileProvider>) -> Self {
        Self {
            cache: HashMap::new(),
            loading_stack: HashSet::new(),
            file_provider,
            current_file: None,
        }
    }

    /// Create a native file system resolver (CLI/REPL)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native() -> Result<Self> {
        Ok(Self::new(Box::new(NativeFileProvider::from_current_dir()?)))
    }

    /// Create a native file system resolver with a specific base directory
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native_with_base(base_dir: PathBuf) -> Self {
        Self::new(Box::new(NativeFileProvider::new(base_dir)))
    }

    /// Set the current file being processed (for relative path resolution)
    pub fn set_current_file(&mut self, path: Option<String>) {
        self.current_file = path;
    }

    /// Resolve and load a module, returning its exports
    pub fn resolve(&mut self, import_path: &str) -> Result<ModuleExports> {
        // Resolve the path relative to the current file
        let from = self.current_file.as_deref().unwrap_or(".");
        let canonical_path = self.file_provider.resolve_path(from, import_path)?;

        // Check cache first
        if let Some(exports) = self.cache.get(&canonical_path) {
            return Ok(exports.clone());
        }

        // Check for circular imports
        if self.loading_stack.contains(&canonical_path) {
            return Err(anyhow!(
                "Circular import detected: '{}' is already being loaded",
                import_path
            ));
        }

        // Mark as loading
        self.loading_stack.insert(canonical_path.clone());

        // Save current file and update for nested imports
        let prev_file = self.current_file.take();
        self.current_file = Some(canonical_path.clone());

        // Load and parse the module
        let result = self.load_module(&canonical_path);

        // Restore state
        self.current_file = prev_file;
        self.loading_stack.remove(&canonical_path);

        let exports = result?;

        // Cache the result
        self.cache.insert(canonical_path, exports.clone());

        Ok(exports)
    }

    /// Load and extract exports from a module file
    fn load_module(&mut self, path: &str) -> Result<ModuleExports> {
        // Read the file
        let contents = self.file_provider.read_file(path)?;

        // Parse the module
        let program =
            parse_statements(&contents).map_err(|e| anyhow!("Parse error in '{}': {}", path, e))?;

        // Extract exports (currently all top-level definitions are public)
        self.extract_exports(&program)
    }

    /// Extract exported definitions from a parsed program
    /// Currently, all top-level `let` and `fn` statements are considered exports
    fn extract_exports(&mut self, program: &Program) -> Result<ModuleExports> {
        let mut exports = ModuleExports::new();

        // Create a temporary environment to evaluate let statements
        let mut temp_env = Environment::new();
        let evaluator = crate::parser::Evaluator::new();

        for stmt in &program.statements {
            match stmt {
                Statement::Let { name, value } => {
                    // Evaluate the expression to get the value
                    match evaluator.eval_with_env(value.clone(), Some(crate::parser::evaluator::EnvironmentRef::Borrowed(&temp_env))) {
                        Ok(val) => {
                            temp_env.define(name.clone(), val.clone());
                            exports.values.insert(name.clone(), val);
                        }
                        Err(e) => {
                            // If we can't evaluate, store as a thunk for lazy evaluation
                            // For now, skip with a warning
                            eprintln!("Warning: Could not evaluate '{}' in module: {}", name, e);
                        }
                    }
                }
                Statement::FunctionDef {
                    name, params, body, ..
                } => {
                    // Store function definition
                    exports
                        .functions
                        .insert(name.clone(), (params.clone(), body.clone()));
                    // Also add to temp env for use by other definitions
                    temp_env.define(
                        name.clone(),
                        Value::Function {
                            name: name.clone(),
                            params: params.clone(),
                            body: body.clone(),
                        },
                    );
                }
                Statement::Use {
                    path,
                    imports,
                    alias,
                } => {
                    // Handle nested imports
                    let nested_exports = self.resolve(path)?;

                    match (imports, alias) {
                        // use "path" - import all to current scope
                        (None, None) => {
                            for (name, val) in &nested_exports.values {
                                exports.values.insert(name.clone(), val.clone());
                            }
                            for (name, (params, body)) in &nested_exports.functions {
                                exports
                                    .functions
                                    .insert(name.clone(), (params.clone(), body.clone()));
                            }
                        }
                        // use { a, b } from "path" - import specific items
                        (Some(names), None) => {
                            for name in names {
                                if let Some(val) = nested_exports.get(name) {
                                    match val {
                                        Value::Function { params, body, .. } => {
                                            exports.functions.insert(name.clone(), (params, body));
                                        }
                                        other => {
                                            exports.values.insert(name.clone(), other);
                                        }
                                    }
                                } else {
                                    return Err(anyhow!(
                                        "Module '{}' does not export '{}'",
                                        path,
                                        name
                                    ));
                                }
                            }
                        }
                        // use "path" as ns - not re-exported (namespace is local)
                        (None, Some(_)) | (Some(_), Some(_)) => {
                            // Namespaced imports are not re-exported
                        }
                    }
                }
                // Other statements are ignored for export purposes
                _ => {}
            }
        }

        Ok(exports)
    }

    /// Clear the module cache (useful for hot reloading)
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock file provider for testing
    struct MockFileProvider {
        files: HashMap<String, String>,
    }

    impl MockFileProvider {
        fn new() -> Self {
            Self {
                files: HashMap::new(),
            }
        }

        fn add_file(&mut self, path: &str, content: &str) {
            self.files.insert(path.to_string(), content.to_string());
        }
    }

    impl FileProvider for MockFileProvider {
        fn read_file(&self, path: &str) -> Result<String> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow!("File not found: {}", path))
        }

        fn resolve_path(&self, _from: &str, import_path: &str) -> Result<String> {
            // Simple resolution for tests - just return the import path
            Ok(import_path.to_string())
        }
    }

    #[test]
    fn test_resolve_simple_module() {
        let mut provider = MockFileProvider::new();
        provider.add_file(
            "drums.cadence",
            r#"
            let kick = "bd bd bd bd"
            fn hihat(rate) { return "hh hh hh hh" }
        "#,
        );

        let mut resolver = ModuleResolver::new(Box::new(provider));
        let exports = resolver.resolve("drums.cadence").unwrap();

        assert!(exports.has("kick"));
        assert!(exports.has("hihat"));
        assert_eq!(exports.names().len(), 2);
    }

    #[test]
    fn test_circular_import_detection() {
        let mut provider = MockFileProvider::new();
        provider.add_file("a.cadence", r#"use "b.cadence""#);
        provider.add_file("b.cadence", r#"use "a.cadence""#);

        let mut resolver = ModuleResolver::new(Box::new(provider));
        let result = resolver.resolve("a.cadence");

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular import"));
    }

    #[test]
    fn test_module_caching() {
        let mut provider = MockFileProvider::new();
        provider.add_file("utils.cadence", r#"let x = 42"#);

        let mut resolver = ModuleResolver::new(Box::new(provider));

        // First resolve
        let exports1 = resolver.resolve("utils.cadence").unwrap();
        // Second resolve should use cache
        let exports2 = resolver.resolve("utils.cadence").unwrap();

        assert!(exports1.has("x"));
        assert!(exports2.has("x"));
    }

    #[test]
    fn test_selective_import() {
        let mut provider = MockFileProvider::new();
        provider.add_file(
            "lib.cadence",
            r#"
            let a = 1
            let b = 2
            let c = 3
        "#,
        );
        provider.add_file(
            "main.cadence",
            r#"
            use { a, b } from "lib.cadence"
        "#,
        );

        let mut resolver = ModuleResolver::new(Box::new(provider));
        let exports = resolver.resolve("main.cadence").unwrap();

        assert!(exports.has("a"));
        assert!(exports.has("b"));
        assert!(!exports.has("c")); // c was not imported
    }
}
