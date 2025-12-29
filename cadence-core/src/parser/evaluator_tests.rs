#[cfg(test)]
mod pattern_operator_tests {
    use crate::parser::parse;
    use crate::parser::{Evaluator, Value};

    #[test]
    fn test_eval_fast() {
        use crate::types::beats;
        // fast("C E", 2) -> pattern with 2 beats per cycle (was 4)
        let expr = parse("fast(\"C E\", 2)").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(p) => assert_eq!(p.beats_per_cycle, beats(2)),
            _ => panic!("Expected pattern value"),
        }
    }

    #[test]
    fn test_eval_slow() {
        use crate::types::beats;
        // slow("C E", 2) -> pattern with 8 beats per cycle (was 4)
        let expr = parse("slow(\"C E\", 2)").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Pattern(p) => assert_eq!(p.beats_per_cycle, beats(8)),
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

        let expr = parse("every(2, \"rev\", \"C D E\")").unwrap();
        let result = Evaluator::new().eval(expr.clone()).unwrap();

        // every() now returns an EveryPattern, not a Pattern
        match result {
            Value::EveryPattern(every) => {
                // Test cycle 0: (0 + 1) % 2 != 0, so base pattern "C D E"
                let p0 = every.get_pattern_for_cycle(0);
                match &p0.steps[0] {
                    crate::types::PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
                    _ => panic!("Expected Note C at cycle 0"),
                }

                // Test cycle 1: (1 + 1) % 2 == 0, so transformed pattern "E D C"
                let p1 = every.get_pattern_for_cycle(1);
                match &p1.steps[0] {
                    crate::types::PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
                    _ => panic!("Expected Note E at cycle 1"),
                }
            }
            _ => panic!("Expected EveryPattern"),
        }
    }
}
