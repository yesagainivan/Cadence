#[cfg(test)]
mod tests {
    use cadence_core::parser::ast::{Expression, Value};
    use cadence_core::parser::environment::Environment;
    use cadence_core::parser::evaluator::{EnvironmentRef, Evaluator};
    use cadence_core::types::pattern::Pattern;
    use cadence_core::types::PatternStep;
    use std::sync::{Arc, RwLock};

    #[test]
    fn test_parse_empty_braces_pattern() {
        // This should return Err, not panic
        let result = Pattern::parse("{}");
        assert!(result.is_err(), "Expected error for empty polyrhythm");
    }

    #[test]
    fn test_parse_empty_group_pattern() {
        // This is valid: []
        // But implementation says "Single word '[]' is not a valid pattern"
        let result = Pattern::parse("[]");
        assert!(result.is_err(), "Empty group is rejected by pattern parser");
    }

    #[test]
    fn test_unresolved_variable_panic() {
        // Create a pattern with a variable manually
        let p = Pattern::with_steps(vec![PatternStep::Variable("v1".to_string())]);

        // Calling to_rich_events on this SHOULD panic strict usage
        let result = std::panic::catch_unwind(move || {
            p.to_rich_events();
        });

        assert!(
            result.is_err(),
            "Should panic for unresolved variables in to_rich_events"
        );
    }

    #[test]
    fn test_recursive_rwlock_read() {
        let lock = Arc::new(RwLock::new(0));
        let lock2 = lock.clone();

        let _guard1 = lock.read().unwrap();
        // This should NOT panic on host (macOS pthread), but might on some WASM impls
        let result = std::panic::catch_unwind(move || {
            let _guard2 = lock2.read().unwrap();
        });

        assert!(
            result.is_ok(),
            "Recursive read lock should be allowed on host"
        );
    }

    #[test]
    fn test_evaluator_thunk_recursion_lock() {
        // Simulate: let x = C; play x;
        // tick() holds env.read(). eval(Expression::Variable("x")).
        // "x" resolves to Value::Thunk.
        // Thunk eval takes thunk_env.read().
        // Recursive read on same lock.

        let env = Arc::new(RwLock::new(Environment::new()));

        // Define x = C as a Thunk
        {
            let mut writer = env.write().unwrap();
            // Use 0 (C) for Note::new which expects pitch class 0-11
            let note_c = cadence_core::types::Note::new(0).unwrap();
            let thunk = Value::Thunk {
                expression: Box::new(Expression::Note(note_c)),
                env: env.clone(),
            };
            writer.define("x".to_string(), thunk);
        }

        let evaluator = Evaluator::new();

        // Simulate tick()
        {
            // Lock environment (like tick does)
            let reader = env.read().unwrap();

            // Eval "x" using environment
            // This will trigger thunk eval, which attempts to lock env.read() AGAIN.
            let expr = Expression::Variable("x".to_string());

            let result = evaluator.eval_with_env(expr, Some(EnvironmentRef::Borrowed(&reader)));

            assert!(result.is_ok(), "Thunk evaluation under lock should succeed");
            match result.unwrap() {
                Value::Note(n) => assert_eq!(n.pitch_class(), 0), // C
                _ => panic!("Expected Note"),
            }
        }
    }
}
