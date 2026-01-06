//! Tests for pattern module.

use super::core::Pattern;
use super::euclidean::bjorklund;
use super::every::EveryPattern;
use super::step::PatternStep;
use crate::types::time::beats;
use crate::types::Chord;
use num_rational::Ratio;

#[test]
fn test_parse_simple_notes() {
    let p = Pattern::parse("C E G").unwrap();
    assert_eq!(p.steps.len(), 3);
    assert!(matches!(&p.steps[0], PatternStep::Note(_)));
}

#[test]
fn test_parse_rest() {
    let p = Pattern::parse("C _ E _").unwrap();
    assert_eq!(p.steps.len(), 4);
    assert!(matches!(&p.steps[1], PatternStep::Rest));
    assert!(matches!(&p.steps[3], PatternStep::Rest));
}

#[test]
fn test_parse_repeat() {
    let p = Pattern::parse("C*3 E").unwrap();
    assert_eq!(p.steps.len(), 2);
    match &p.steps[0] {
        PatternStep::Repeat(_, count) => assert_eq!(*count, 3),
        _ => panic!("Expected Repeat"),
    }
}

#[test]
fn test_parse_group() {
    let p = Pattern::parse("[C E] G").unwrap();
    assert_eq!(p.steps.len(), 2);
    match &p.steps[0] {
        PatternStep::Group(inner) => assert_eq!(inner.len(), 2),
        _ => panic!("Expected Group"),
    }
}

#[test]
fn test_parse_chord() {
    let p = Pattern::parse("[C, E, G] _").unwrap();
    assert_eq!(p.steps.len(), 2);
    assert!(matches!(&p.steps[0], PatternStep::Chord(_)));
}

#[test]
fn test_step_beats() {
    let p = Pattern::parse("C E G _").unwrap();
    assert_eq!(p.beats_per_cycle, beats(4));
    assert_eq!(p.step_beats(), beats(1)); // 4 steps, 4 beats = 1 beat each
}

#[test]
fn test_fast() {
    let p = Pattern::parse("C E").unwrap().fast(2);
    assert_eq!(p.beats_per_cycle, beats(2)); // Now plays in 2 beats
}

#[test]
fn test_slow() {
    let p = Pattern::parse("C E").unwrap().slow(2);
    assert_eq!(p.beats_per_cycle, beats(8)); // Now takes 8 beats
}

#[test]
fn test_rev() {
    let p = Pattern::parse("C D E").unwrap().rev();
    // E D C now
    match &p.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
        _ => panic!("Expected Note"),
    }
}

#[test]
fn test_to_events() {
    let p = Pattern::parse("C E G _").unwrap();
    let events = p.to_events();
    assert_eq!(events.len(), 4);
    assert!(!events[0].2); // Not a rest
    assert!(events[3].2); // Is a rest
}

#[test]
fn test_display() {
    let p = Pattern::parse("C E G").unwrap();
    assert_eq!(format!("{}", p), "\"C E G\"");
}

#[test]
fn test_parse_flat_notes() {
    let p = Pattern::parse("Bb Eb Ab").unwrap();
    assert_eq!(p.steps.len(), 3);
    // Bb should be pitch class 10 (A#/Bb)
    match &p.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 10),
        _ => panic!("Expected Note"),
    }
    // Eb should be pitch class 3 (D#/Eb)
    match &p.steps[1] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 3),
        _ => panic!("Expected Note"),
    }
}

#[test]
fn test_chord_repeat_expansion() {
    // Test that [C,E,G]*2 [F,A,C] gives 3 events (2 C majors + 1 F major)
    let p = Pattern::parse("[C,E,G]*2 [F,A,C]").unwrap();
    assert_eq!(p.steps.len(), 2); // 2 steps: Repeat(Chord) and Chord

    // Check the first step is a Repeat containing a Chord
    match &p.steps[0] {
        PatternStep::Repeat(inner, count) => {
            assert_eq!(*count, 2);
            assert!(matches!(**inner, PatternStep::Chord(_)));
        }
        _ => panic!("Expected Repeat"),
    }

    // Check to_events expands correctly
    let events = p.to_events();
    assert_eq!(
        events.len(),
        3,
        "Should have 3 events: [C,E,G], [C,E,G], [F,A,C]"
    );

    // First two events should be C major (3 notes)
    assert_eq!(events[0].0.len(), 3);
    assert_eq!(events[1].0.len(), 3);
    // Last event should be F major (3 notes)
    assert_eq!(events[2].0.len(), 3);

    // Durations should be 2.0 beats for each (cycle=4, 2 slots, each slot has 2 and 1 events)
    // Step 1: Repeat(Chord)*2 = 2 events at step_beats/2 each
    // Step 2: Chord = 1 event at step_beats
    // With 2 steps and 4 beat cycle, step_beats = 2.0
    // Repeat expands to 2 events at 2.0/2 = 1.0 each
    // Last chord at 2.0
    assert_eq!(events[0].1, 1.0);
    assert_eq!(events[1].1, 1.0);
    assert_eq!(events[2].1, 2.0);
}

#[test]
fn test_voice_leading_frequency_order_in_playback() {
    // This test verifies that after voice leading optimization,
    // the frequencies sent to MIDI/audio are in the correct order

    let c_maj = Chord::from_note_strings(vec!["C4", "E4", "G4"]).unwrap();
    let f_maj = Chord::from_note_strings(vec!["F4", "A4", "C4"]).unwrap();
    let pattern = Pattern::from_chords(vec![c_maj.clone(), f_maj.clone()]);

    // Optimize voice leading
    let optimized = pattern.optimize_voice_leading();

    // Get the playback events
    let events = optimized.to_events();
    assert_eq!(events.len(), 2, "Should have 2 chord events");

    // Get the chord at index 1 for verification
    let chords = optimized.as_chords().expect("Should be chord-only pattern");
    let second_chord = &chords[1];
    let notes = second_chord.notes_vec();

    // Calculate expected frequencies
    let expected_freqs: Vec<f32> = notes.iter().map(|n| n.frequency()).collect();

    // Check the frequencies of the second chord
    let (freqs, _duration, _is_rest) = &events[1];
    assert_eq!(freqs.len(), 3, "Second chord should have 3 frequencies");

    // Frequencies should match the optimized chord order
    let tolerance = 0.1; // Small tolerance for float comparison
    for (i, (actual, expected)) in freqs.iter().zip(expected_freqs.iter()).enumerate() {
        assert!(
            (actual - expected).abs() < tolerance,
            "Frequency {} should be ~{}, got {}",
            i,
            expected,
            actual
        );
    }
}

#[test]
fn test_parse_pattern_with_variables() {
    // "C cmaj E" should parse with a Variable step in the middle
    let p = Pattern::parse("C cmaj E").unwrap();
    assert_eq!(p.steps.len(), 3);
    assert!(matches!(&p.steps[0], PatternStep::Note(_)));
    assert!(matches!(&p.steps[1], PatternStep::Variable(name) if name == "cmaj"));
    assert!(matches!(&p.steps[2], PatternStep::Note(_)));
    assert!(p.has_variables());
}

#[test]
fn test_parse_single_variable_fails() {
    // Single-word variable patterns should fail (treated as plain string)
    // This prevents "pluck" or "rev" from being parsed as patterns
    assert!(Pattern::parse("pluck").is_err());
    assert!(Pattern::parse("rev").is_err());
    assert!(Pattern::parse("cmaj").is_err());
}

#[test]
fn test_parse_multi_variable_pattern() {
    // Multi-word variable-only patterns should succeed
    let p = Pattern::parse("cmaj fmaj").unwrap();
    assert_eq!(p.steps.len(), 2);
    assert!(matches!(&p.steps[0], PatternStep::Variable(name) if name == "cmaj"));
    assert!(matches!(&p.steps[1], PatternStep::Variable(name) if name == "fmaj"));
    assert!(p.has_variables());
}

#[test]
fn test_resolve_variables() {
    // "C myvar E" with myvar=[D, F]
    let p = Pattern::parse("C myvar E").unwrap();
    assert!(p.has_variables());

    let resolved = p
        .resolve_variables_with(|name| {
            if name == "myvar" {
                // Resolve to [D, F] chord
                let chord = Chord::from_note_strings(vec!["D", "F"]).unwrap();
                Some(vec![PatternStep::Chord(chord)])
            } else {
                None
            }
        })
        .unwrap();

    assert!(!resolved.has_variables());
    assert_eq!(resolved.steps.len(), 3);
    // First should be C
    assert!(matches!(&resolved.steps[0], PatternStep::Note(_)));
    // Second should now be the chord
    assert!(matches!(&resolved.steps[1], PatternStep::Chord(_)));
    // Third should be E
    assert!(matches!(&resolved.steps[2], PatternStep::Note(_)));
}

#[test]
fn test_resolve_undefined_variable_fails() {
    let p = Pattern::parse("C undefined E").unwrap();
    let result = p.resolve_variables_with(|_| None);
    assert!(result.is_err());
}

#[test]
fn test_get_variable_names() {
    let p = Pattern::parse("C foo E bar G").unwrap();
    let vars = p.get_variable_names();
    assert_eq!(vars, vec!["foo", "bar"]);
}

// Tests for new pattern manipulation methods
#[test]
fn test_rotate_right() {
    let p = Pattern::parse("C D E F").unwrap().rotate(1);
    // Should rotate right: F C D E
    match &p.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 5), // F
        _ => panic!("Expected Note F"),
    }
    match &p.steps[1] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
        _ => panic!("Expected Note C"),
    }
}

#[test]
fn test_rotate_left() {
    let p = Pattern::parse("C D E F").unwrap().rotate(-1);
    // Should rotate left: D E F C
    match &p.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 2), // D
        _ => panic!("Expected Note D"),
    }
    match &p.steps[3] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
        _ => panic!("Expected Note C"),
    }
}

#[test]
fn test_take() {
    let p = Pattern::parse("C D E F").unwrap().take(2);
    assert_eq!(p.steps.len(), 2);
    match &p.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
        _ => panic!("Expected Note C"),
    }
    match &p.steps[1] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 2), // D
        _ => panic!("Expected Note D"),
    }
}

#[test]
fn test_drop_steps() {
    let p = Pattern::parse("C D E F").unwrap().drop_steps(2);
    assert_eq!(p.steps.len(), 2);
    match &p.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
        _ => panic!("Expected Note E"),
    }
    match &p.steps[1] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 5), // F
        _ => panic!("Expected Note F"),
    }
}

#[test]
fn test_palindrome() {
    let p = Pattern::parse("C D E").unwrap().palindrome();
    assert_eq!(p.steps.len(), 6); // C D E E D C
    assert_eq!(p.beats_per_cycle, beats(8)); // Doubled from 4
    match &p.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
        _ => panic!("Expected Note C"),
    }
    // Last three: E D C
    match &p.steps[3] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
        _ => panic!("Expected Note E"),
    }
    match &p.steps[5] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
        _ => panic!("Expected Note C"),
    }
}

#[test]
fn test_stutter() {
    let p = Pattern::parse("C D").unwrap().stutter(3);
    assert_eq!(p.steps.len(), 6); // C C C D D D
    for i in 0..3 {
        match &p.steps[i] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
            _ => panic!("Expected Note C"),
        }
    }
    for i in 3..6 {
        match &p.steps[i] {
            PatternStep::Note(n) => assert_eq!(n.pitch_class(), 2), // D
            _ => panic!("Expected Note D"),
        }
    }
}

#[test]
fn test_concat() {
    let p1 = Pattern::parse("C D").unwrap();
    let p2 = Pattern::parse("E F").unwrap();
    let combined = p1.concat(p2);
    assert_eq!(combined.steps.len(), 4);
    assert_eq!(combined.beats_per_cycle, beats(8)); // 4 + 4
    match &combined.steps[2] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
        _ => panic!("Expected Note E"),
    }
}

// ========================================================================
// Alternation Tests
// ========================================================================

#[test]
fn test_parse_alternation_simple() {
    // <C D E> should parse to Alternation with 3 notes
    let p = Pattern::parse("<C D E>").unwrap();
    assert_eq!(p.steps.len(), 1);
    match &p.steps[0] {
        PatternStep::Alternation(steps) => {
            assert_eq!(steps.len(), 3);
            // First should be C
            match &steps[0] {
                PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
                _ => panic!("Expected Note C"),
            }
            // Second should be D
            match &steps[1] {
                PatternStep::Note(n) => assert_eq!(n.pitch_class(), 2), // D
                _ => panic!("Expected Note D"),
            }
            // Third should be E
            match &steps[2] {
                PatternStep::Note(n) => assert_eq!(n.pitch_class(), 4), // E
                _ => panic!("Expected Note E"),
            }
        }
        _ => panic!("Expected Alternation"),
    }
}

#[test]
fn test_parse_alternation_mixed() {
    // C <D E> F should parse to Note, Alternation, Note
    let p = Pattern::parse("C <D E> F").unwrap();
    assert_eq!(p.steps.len(), 3);
    assert!(matches!(&p.steps[0], PatternStep::Note(_)));
    assert!(matches!(&p.steps[1], PatternStep::Alternation(_)));
    assert!(matches!(&p.steps[2], PatternStep::Note(_)));
}

#[test]
fn test_parse_alternation_with_repeat() {
    // <C D>*2 should parse to alternation with repeat modifier
    let p = Pattern::parse("<C D>*2").unwrap();
    assert_eq!(p.steps.len(), 1);
    match &p.steps[0] {
        PatternStep::Repeat(inner, count) => {
            assert_eq!(*count, 2);
            assert!(matches!(**inner, PatternStep::Alternation(_)));
        }
        _ => panic!("Expected Repeat"),
    }
}

#[test]
fn test_alternation_display() {
    let p = Pattern::parse("<C D E>").unwrap();
    let display = format!("{}", p);
    assert!(display.contains("<C"));
    assert!(display.contains(">"));
}

#[test]
fn test_alternation_cycle_selection() {
    // Test that to_step_info_for_cycle returns different elements for different cycles
    let p = Pattern::parse("<C D E>").unwrap();
    let alt_step = &p.steps[0];

    // Cycle 0 should return C (pitch_class 0)
    let info_0 = alt_step.to_step_info_for_cycle(0);
    assert_eq!(info_0.len(), 1);
    assert_eq!(info_0[0].0[0].pitch_class, 0); // C

    // Cycle 1 should return D (pitch_class 2)
    let info_1 = alt_step.to_step_info_for_cycle(1);
    assert_eq!(info_1.len(), 1);
    assert_eq!(info_1[0].0[0].pitch_class, 2); // D

    // Cycle 2 should return E (pitch_class 4)
    let info_2 = alt_step.to_step_info_for_cycle(2);
    assert_eq!(info_2.len(), 1);
    assert_eq!(info_2[0].0[0].pitch_class, 4); // E

    // Cycle 3 should wrap back to C
    let info_3 = alt_step.to_step_info_for_cycle(3);
    assert_eq!(info_3[0].0[0].pitch_class, 0); // C
}

#[test]
fn test_alternation_rich_events_for_cycle() {
    // Test that to_rich_events_for_cycle produces correct events
    let p = Pattern::parse("<C D E>").unwrap();

    // Cycle 0: should have C
    let events_0 = p.to_rich_events_for_cycle(0);
    assert_eq!(events_0.len(), 1);
    assert_eq!(events_0[0].notes[0].pitch_class, 0); // C

    // Cycle 1: should have D
    let events_1 = p.to_rich_events_for_cycle(1);
    assert_eq!(events_1[0].notes[0].pitch_class, 2); // D
}

#[test]
fn test_alternation_empty_fails() {
    let result = Pattern::parse("<>");
    assert!(result.is_err());
}

#[test]
fn test_alternation_unclosed_fails() {
    let result = Pattern::parse("<C D E");
    assert!(result.is_err());
}

// ========================================================================
// EveryPattern Tests
// ========================================================================

#[test]
fn test_every_pattern_creation() {
    let base = Pattern::parse("C D E F").unwrap();
    let transformed = base.clone().rev();
    let every = EveryPattern::new(2, base.clone(), transformed.clone());

    assert_eq!(every.interval, 2);
    assert_eq!(every.base.steps.len(), 4);
    assert_eq!(every.transformed.steps.len(), 4);

    // Verify base pattern is C D E F
    match &every.base.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0), // C
        _ => panic!("Expected Note C"),
    }

    // Verify transformed pattern is F E D C (reversed)
    match &every.transformed.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 5), // F
        _ => panic!("Expected Note F"),
    }
}

#[test]
fn test_every_pattern_cycle_selection_interval_2() {
    let base = Pattern::parse("C D E F").unwrap();
    let transformed = base.clone().rev(); // F E D C
    let every = EveryPattern::new(2, base.clone(), transformed.clone());

    // New behavior: every(2) = base, transformed, base, transformed...
    // Cycle 0: base ((0+1) % 2 = 1, not 0)
    let pattern_at_0 = every.get_pattern_for_cycle(0);
    match &pattern_at_0.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0, "Cycle 0 should be base (C)"),
        _ => panic!("Expected Note"),
    }

    // Cycle 1: transformed ((1+1) % 2 = 0)
    let pattern_at_1 = every.get_pattern_for_cycle(1);
    match &pattern_at_1.steps[0] {
        PatternStep::Note(n) => {
            assert_eq!(n.pitch_class(), 5, "Cycle 1 should be transformed (F)")
        }
        _ => panic!("Expected Note"),
    }

    // Cycle 2: base ((2+1) % 2 = 1, not 0)
    let pattern_at_2 = every.get_pattern_for_cycle(2);
    match &pattern_at_2.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0, "Cycle 2 should be base (C)"),
        _ => panic!("Expected Note"),
    }

    // Cycle 3: transformed ((3+1) % 2 = 0)
    let pattern_at_3 = every.get_pattern_for_cycle(3);
    match &pattern_at_3.steps[0] {
        PatternStep::Note(n) => {
            assert_eq!(n.pitch_class(), 5, "Cycle 3 should be transformed (F)")
        }
        _ => panic!("Expected Note"),
    }

    // Cycle 4: base ((4+1) % 2 = 1, not 0)
    let pattern_at_4 = every.get_pattern_for_cycle(4);
    match &pattern_at_4.steps[0] {
        PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0, "Cycle 4 should be base (C)"),
        _ => panic!("Expected Note"),
    }
}

#[test]
fn test_every_pattern_cycle_selection_interval_3() {
    let base = Pattern::parse("C D E F").unwrap();
    let transformed = base.clone().rev();
    let every = EveryPattern::new(3, base.clone(), transformed.clone());

    // Interval 3: every(3) = base, base, transformed, base, base, transformed...
    // Cycle 0: base ((0+1) % 3 = 1, not 0)
    assert_eq!(every.get_pattern_for_cycle(0).steps[0], every.base.steps[0]);
    // Cycle 1: base ((1+1) % 3 = 2, not 0)
    assert_eq!(every.get_pattern_for_cycle(1).steps[0], every.base.steps[0]);
    // Cycle 2: transformed ((2+1) % 3 = 0)
    assert_eq!(
        every.get_pattern_for_cycle(2).steps[0],
        every.transformed.steps[0]
    );
    // Cycle 3: base ((3+1) % 3 = 1, not 0)
    assert_eq!(every.get_pattern_for_cycle(3).steps[0], every.base.steps[0]);
    // Cycle 4: base ((4+1) % 3 = 2, not 0)
    assert_eq!(every.get_pattern_for_cycle(4).steps[0], every.base.steps[0]);
    // Cycle 5: transformed ((5+1) % 3 = 0)
    assert_eq!(
        every.get_pattern_for_cycle(5).steps[0],
        every.transformed.steps[0]
    );
    // Cycle 6: base ((6+1) % 3 = 1, not 0)
    assert_eq!(every.get_pattern_for_cycle(6).steps[0], every.base.steps[0]);
}

#[test]
fn test_every_pattern_interval_1_always_transformed() {
    let base = Pattern::parse("C D").unwrap();
    let transformed = base.clone().rev();
    let every = EveryPattern::new(1, base.clone(), transformed.clone());

    // Interval 1: every(1) = transform every cycle (all cycles have (cycle+1) % 1 == 0)
    for cycle in 0..10 {
        assert_eq!(
            every.get_pattern_for_cycle(cycle).steps[0],
            every.transformed.steps[0],
            "Interval 1 should return transformed at cycle {}",
            cycle
        );
    }
}

#[test]
fn test_every_pattern_interval_0_becomes_1() {
    // Interval 0 should be clamped to 1 to avoid division by zero
    let base = Pattern::parse("C D").unwrap();
    let transformed = base.clone().rev();
    let every = EveryPattern::new(0, base.clone(), transformed.clone());

    assert_eq!(every.interval, 1, "Interval 0 should be clamped to 1");
}

#[test]
fn test_every_pattern_display() {
    let base = Pattern::parse("C D").unwrap();
    let transformed = base.clone().rev();
    let every = EveryPattern::new(2, base, transformed);

    let display = format!("{}", every);
    assert!(display.contains("every(2"), "Display should show interval");
}

#[test]
fn test_every_pattern_pattern_for_cycle_clone() {
    let base = Pattern::parse("C D E F").unwrap();
    let transformed = base.clone().rev();
    let every = EveryPattern::new(2, base.clone(), transformed.clone());

    // Test pattern_for_cycle (returns clone)
    // Cycle 0 returns base, cycle 1 returns transformed (new logic)
    let cloned_base = every.pattern_for_cycle(0);
    assert_eq!(cloned_base.steps.len(), 4);
    assert_eq!(cloned_base.steps[0], every.base.steps[0]);

    let cloned_transformed = every.pattern_for_cycle(1);
    assert_eq!(cloned_transformed.steps.len(), 4);
    assert_eq!(cloned_transformed.steps[0], every.transformed.steps[0]);
}

// ========================================================================
// Weighted Steps Tests
// ========================================================================

#[test]
fn test_weighted_parse_simple() {
    let p = Pattern::parse("C@2 D").unwrap();
    assert_eq!(p.steps.len(), 2);
    // First step should be Weighted(Note(C), 2)
    match &p.steps[0] {
        PatternStep::Weighted(inner, weight) => {
            assert_eq!(*weight, 2);
            assert!(matches!(**inner, PatternStep::Note(_)));
        }
        _ => panic!("Expected Weighted step"),
    }
    // Second step should be Note(D) with weight 1
    assert!(matches!(&p.steps[1], PatternStep::Note(_)));
    assert_eq!(p.steps[1].weight(), 1);
}

#[test]
fn test_weighted_durations() {
    let p = Pattern::parse("C@2 D").unwrap();
    let events = p.to_rich_events();

    // Total weight = 2 + 1 = 3
    // C gets 2/3 of 4 beats = 8/3 beats
    // D gets 1/3 of 4 beats = 4/3 beats
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].duration, Ratio::new(8, 3));
    assert_eq!(events[1].duration, Ratio::new(4, 3));

    // Check start times
    assert_eq!(events[0].start_beat, Ratio::from_integer(0));
    assert_eq!(events[1].start_beat, Ratio::new(8, 3));
}

#[test]
fn test_weighted_chord() {
    let p = Pattern::parse("[C,E]@3 G").unwrap();
    let events = p.to_rich_events();

    // Total weight = 3 + 1 = 4
    // Chord gets 3/4 of 4 beats = 3 beats
    // G gets 1/4 of 4 beats = 1 beat
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].duration, beats(3));
    assert_eq!(events[1].duration, beats(1));
}

#[test]
fn test_weighted_rest() {
    let p = Pattern::parse("C@2 _@2 D").unwrap();
    let events = p.to_rich_events();

    // Total weight = 2 + 2 + 1 = 5
    // C gets 2/5 of 4 = 8/5 beats
    // Rest gets 2/5 of 4 = 8/5 beats
    // D gets 1/5 of 4 = 4/5 beats
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].duration, Ratio::new(8, 5));
    assert_eq!(events[1].duration, Ratio::new(8, 5));
    assert!(events[1].is_rest);
    assert_eq!(events[2].duration, Ratio::new(4, 5));
}

#[test]
fn test_weighted_with_repeat() {
    // C@2*3 means: weight 2, repeated 3 times
    let p = Pattern::parse("C@2*3 D").unwrap();

    // Should have 2 steps: Repeat(Weighted(C, 2), 3) and Note(D)
    assert_eq!(p.steps.len(), 2);
    match &p.steps[0] {
        PatternStep::Repeat(inner, count) => {
            assert_eq!(*count, 3);
            match inner.as_ref() {
                PatternStep::Weighted(note, weight) => {
                    assert_eq!(*weight, 2);
                    assert!(matches!(**note, PatternStep::Note(_)));
                }
                _ => panic!("Expected Weighted inside Repeat"),
            }
        }
        _ => panic!("Expected Repeat step"),
    }
}

#[test]
fn test_weighted_display() {
    let p = Pattern::parse("C@2 D").unwrap();
    let display = format!("{}", p);
    assert!(
        display.contains("C@2"),
        "Display should show weight: got {}",
        display
    );
    assert!(
        display.contains("D"),
        "Display should show D: got {}",
        display
    );
}

#[test]
fn test_weighted_transpose() {
    let p = Pattern::parse("C@2 D").unwrap();
    let transposed = p.transpose(2);

    // Weight should be preserved after transpose
    match &transposed.steps[0] {
        PatternStep::Weighted(inner, weight) => {
            assert_eq!(*weight, 2);
            // D (C + 2) should be pitch class 2
            match inner.as_ref() {
                PatternStep::Note(n) => assert_eq!(n.pitch_class(), 2),
                _ => panic!("Expected Note inside Weighted"),
            }
        }
        _ => panic!("Expected Weighted step"),
    }
}

#[test]
fn test_weighted_zero_error() {
    // @0 should be an error
    let result = Pattern::parse("C@0 D");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("@0"));
}

// ============================================================================
// Euclidean Rhythm Tests
// ============================================================================

#[test]
fn test_bjorklund_classic_patterns() {
    // E(3,8) - Cuban tresillo: 3 pulses evenly distributed across 8 slots
    let e38 = bjorklund(3, 8);
    assert_eq!(e38.len(), 8);
    assert_eq!(e38.iter().filter(|&&x| x).count(), 3);

    // E(5,8) - Cuban cinquillo: 5 pulses across 8 slots
    let e58 = bjorklund(5, 8);
    assert_eq!(e58.len(), 8);
    assert_eq!(e58.iter().filter(|&&x| x).count(), 5);

    // E(3,4) - 3 pulses across 4 slots
    let e34 = bjorklund(3, 4);
    assert_eq!(e34.len(), 4);
    assert_eq!(e34.iter().filter(|&&x| x).count(), 3);

    // E(1,4) - 1 pulse across 4 slots
    let e14 = bjorklund(1, 4);
    assert_eq!(e14.len(), 4);
    assert_eq!(e14.iter().filter(|&&x| x).count(), 1);
}

#[test]
fn test_bjorklund_edge_cases() {
    // E(0,4) - all rests
    assert_eq!(bjorklund(0, 4), vec![false, false, false, false]);

    // E(4,4) - all pulses
    assert_eq!(bjorklund(4, 4), vec![true, true, true, true]);

    // E(5,4) - more pulses than steps, cap to all true
    assert_eq!(bjorklund(5, 4), vec![true, true, true, true]);

    // E(0,0) - empty
    assert_eq!(bjorklund(0, 0), Vec::<bool>::new());
}

#[test]
fn test_euclidean_parse_simple() {
    let p = Pattern::parse("C(3,8)").unwrap();
    assert_eq!(p.steps.len(), 1);
    match &p.steps[0] {
        PatternStep::Euclidean(inner, pulses, steps) => {
            assert_eq!(*pulses, 3);
            assert_eq!(*steps, 8);
            assert!(matches!(inner.as_ref(), PatternStep::Note(_)));
        }
        _ => panic!("Expected Euclidean step"),
    }
}

#[test]
fn test_euclidean_parse_with_weight() {
    let p = Pattern::parse("C(3,8)@2").unwrap();
    assert_eq!(p.steps.len(), 1);
    match &p.steps[0] {
        PatternStep::Weighted(inner_weighted, weight) => {
            assert_eq!(*weight, 2);
            match inner_weighted.as_ref() {
                PatternStep::Euclidean(_, pulses, steps) => {
                    assert_eq!(*pulses, 3);
                    assert_eq!(*steps, 8);
                }
                _ => panic!("Expected Euclidean inside Weighted"),
            }
        }
        _ => panic!("Expected Weighted step"),
    }
}

#[test]
fn test_euclidean_parse_with_repeat() {
    let p = Pattern::parse("C(3,8)*2").unwrap();
    assert_eq!(p.steps.len(), 1);
    match &p.steps[0] {
        PatternStep::Repeat(inner, count) => {
            assert_eq!(*count, 2);
            assert!(matches!(inner.as_ref(), PatternStep::Euclidean(_, 3, 8)));
        }
        _ => panic!("Expected Repeat step"),
    }
}

#[test]
fn test_euclidean_display() {
    let p = Pattern::parse("C(3,8) D").unwrap();
    let display = format!("{}", p);
    // Display format shows base note with Euclidean suffix
    assert!(
        display.contains("(3,8)"),
        "Display should show (3,8): got {}",
        display
    );
}

#[test]
fn test_euclidean_transpose() {
    let p = Pattern::parse("C(3,8)").unwrap();
    let transposed = p.transpose(2);

    match &transposed.steps[0] {
        PatternStep::Euclidean(inner, pulses, steps) => {
            assert_eq!(*pulses, 3);
            assert_eq!(*steps, 8);
            match inner.as_ref() {
                PatternStep::Note(n) => assert_eq!(n.pitch_class(), 2), // D
                _ => panic!("Expected Note inside Euclidean"),
            }
        }
        _ => panic!("Expected Euclidean step"),
    }
}

#[test]
fn test_euclidean_to_step_info_expansion() {
    let p = Pattern::parse("C(3,8)").unwrap();

    // Get the step info for the Euclidean pattern
    let step = &p.steps[0];
    let events = step.to_step_info_for_cycle(0);

    // Should expand to 8 events: 3 notes and 5 rests
    assert_eq!(events.len(), 8, "Euclidean(3,8) should expand to 8 steps");

    let pulses: usize = events
        .iter()
        .filter(|(notes, _, is_rest)| !is_rest && !notes.is_empty())
        .count();
    let rests: usize = events
        .iter()
        .filter(|(notes, _, is_rest)| *is_rest || notes.is_empty())
        .count();

    assert_eq!(pulses, 3, "Should have 3 pulses");
    assert_eq!(rests, 5, "Should have 5 rests");
}

#[test]
fn test_euclidean_invalid_syntax() {
    // Missing closing paren
    assert!(Pattern::parse("C(3,8").is_err());

    // Missing comma
    assert!(Pattern::parse("C(38)").is_err());

    // Missing numbers
    assert!(Pattern::parse("C(,8)").is_err());
    assert!(Pattern::parse("C(3,)").is_err());

    // Zero steps
    assert!(Pattern::parse("C(3,0)").is_err());
}

#[test]
fn test_euclidean_drum() {
    let p = Pattern::parse("kick(3,8)").unwrap();
    assert_eq!(p.steps.len(), 1);
    match &p.steps[0] {
        PatternStep::Euclidean(inner, pulses, steps) => {
            assert_eq!(*pulses, 3);
            assert_eq!(*steps, 8);
            assert!(matches!(inner.as_ref(), PatternStep::Drum(_)));
        }
        _ => panic!("Expected Euclidean step with drum"),
    }
}
