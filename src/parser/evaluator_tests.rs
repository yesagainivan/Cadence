#[cfg(test)]
mod pattern_operator_tests {
    use crate::parser::parse;
    use crate::parser::{Evaluator, Value};

    #[test]
    fn test_eval_fast() {
        // fast("C E", 2) -> pattern with 2.0 beats per cycle (was 4.0)
        let expr = parse("fast(\"C E\", 2)").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(p) => assert_eq!(p.beats_per_cycle, 2.0),
            _ => panic!("Expected pattern value"),
        }
    }

    #[test]
    fn test_eval_slow() {
        // slow("C E", 2) -> pattern with 8.0 beats per cycle (was 4.0)
        let expr = parse("slow(\"C E\", 2)").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(p) => assert_eq!(p.beats_per_cycle, 8.0),
            _ => panic!("Expected pattern value"),
        }
    }

    #[test]
    fn test_eval_rev() {
        // rev("C D E") -> E D C
        let expr = parse("rev(\"C D E\")").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(p) => {
                let steps = p.steps;
                assert_eq!(steps.len(), 3);
                // First step should be E (pitch class 4)
                match &steps[0] {
                    crate::types::PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4),
                    _ => panic!("Expected Note E"),
                }
                // Last step should be C (pitch class 0)
                match &steps[2] {
                    crate::types::PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0),
                    _ => panic!("Expected Note C"),
                }
            }
            _ => panic!("Expected pattern value"),
        }
    }
    #[test]
    fn test_eval_every_rev() {
        use crate::parser::ast::Value;
        use crate::parser::evaluator::Evaluator;

        // Mock environment with _cycle = 0
        let mut env = crate::parser::environment::Environment::new();
        env.define("_cycle".to_string(), Value::Number(0));

        let expr = parse("every(2, \"rev\", \"C D E\")").unwrap();
        // Cycle 0: 0 % 2 == 0 => rev applied => "E D C"
        let result = Evaluator::new()
            .eval_with_env(expr.clone(), Some(&env))
            .unwrap();
        match result {
            Value::Pattern(p) => {
                let _events = p.to_events();
                // First event should be E using to_events which might not be ordered by time?
                // to_events returns (frequencies, duration, is_rest).
                // Actually Pattern logic: rev reverses the steps.
                // "C D E" steps: C, D, E.
                // Rev steps: E, D, C.
                // So first step is E.
                let steps = p.steps;
                match &steps[0] {
                    crate::types::PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
                    _ => panic!("Expected Note E"),
                }
            }
            _ => panic!("Expected pattern"),
        }

        // Cycle 1: 1 % 2 != 0 => normal => "C D E"
        env.set("_cycle", Value::Number(1)).unwrap();
        let result = Evaluator::new().eval_with_env(expr, Some(&env)).unwrap();
        match result {
            Value::Pattern(p) => {
                let steps = p.steps;
                match &steps[0] {
                    crate::types::PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
                    _ => panic!("Expected Note C"),
                }
            }
            _ => panic!("Expected pattern"),
        }
    }
}
