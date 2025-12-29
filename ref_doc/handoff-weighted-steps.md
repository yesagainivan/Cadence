# Handoff: Weighted Steps (`@N` Syntax)

## Overview

Implement weighted step notation allowing unequal step durations within patterns. For example:
- `"C@2 D"` - C is twice as long as D
- `"C@3 D@1 E@2"` - C takes 3/6, D takes 1/6, E takes 2/6 of the cycle

## Current State

The `feature/rational-timing` branch has:
- ✅ `Time` type (`Ratio<i64>`) for exact timing
- ✅ WASM serialization as `{ "n": N, "d": D }`
- ✅ All 306 tests passing
- ✅ WASM built and deployed to editor

## Proposed Design

### Syntax

```
"C@2 D"       → C gets 2 parts, D gets 1 part (default)
"C@2 D@3 E"   → C=2/6, D=3/6, E=1/6
"[C,E]@3 G"   → Chord [C,E] gets 3 parts, G gets 1 part
```

### Implementation Areas

#### 1. Lexer (`cadence-core/src/parser/lexer.rs`)

Add `@` token recognition after note/chord tokens.

```rust
// New token type
Token::Weight(usize)  // e.g., @2, @3
```

#### 2. Pattern Parser (`cadence-core/src/types/pattern.rs`)

Modify `PatternStep` to include weight:

```rust
pub enum PatternStep {
    Note(Note),
    WeightedNote { note: Note, weight: usize },
    Chord(Chord),
    WeightedChord { chord: Chord, weight: usize },
    Rest,
    WeightedRest(usize),
    SubPattern(Vec<PatternStep>),
    // ...
}
```

Or add a weight field to each variant.

#### 3. Event Generation (`to_rich_events`)

Update `to_rich_events()` to calculate durations based on weights:

```rust
fn to_rich_events(&self) -> Vec<PlaybackEvent> {
    // Calculate total weight
    let total_weight: i64 = self.steps.iter()
        .map(|s| s.weight().unwrap_or(1) as i64)
        .sum();
    
    // Duration per weight unit
    let unit_duration = self.beats_per_cycle / time(total_weight, 1);
    
    // Generate events with weighted durations
    for step in &self.steps {
        let weight = step.weight().unwrap_or(1) as i64;
        let duration = unit_duration * time(weight, 1);
        // ...
    }
}
```

### Test Cases

```rust
#[test]
fn test_weighted_steps() {
    let p = Pattern::from_str("C@2 D").unwrap();
    let events = p.to_rich_events();
    
    // C gets 2/3 of 4 beats = 8/3 beats
    assert_eq!(events[0].duration, time(8, 3));
    // D gets 1/3 of 4 beats = 4/3 beats  
    assert_eq!(events[1].duration, time(4, 3));
}

#[test]
fn test_weighted_chord() {
    let p = Pattern::from_str("[C,E]@3 G").unwrap();
    let events = p.to_rich_events();
    
    // Chord gets 3/4 of 4 beats = 3 beats
    assert_eq!(events[0].duration, beats(3));
    // G gets 1/4 of 4 beats = 1 beat
    assert_eq!(events[1].duration, beats(1));
}
```

### Questions to Consider

1. **Euclidean defaults?** Should unweighted steps default to 1, or should there be special handling?

2. **Nested patterns?** How do weights interact with sub-patterns `[C@2 D]`?

3. **Weight 0?** Should `@0` be allowed (skip entirely)?

4. **Syntax alternatives?** Consider `C*2` or `C:2` if `@` conflicts.

## Files to Modify

1. `cadence-core/src/parser/lexer.rs` - Add weight token
2. `cadence-core/src/types/pattern.rs` - Add weight to PatternStep, update parsing
3. `cadence-core/src/types/pattern.rs::to_rich_events()` - Calculate weighted durations
4. Tests in respective files

## Branch

Continue on `feature/rational-timing` or create `feature/weighted-steps`.
