# Handoff: TidalCycles-Style `every()` Implementation

**Date:** 2024-12-28
**Status:** Ready for future session

---

## Summary

The `every()` function currently doesn't work correctly with reactive playback. It only applies the transformation once and stays that way. This document describes how to implement a proper TidalCycles-style `every()` that alternates based on cycle position.

---

## Current Behavior (Broken)

```cadence
let p = "C E G B"
play every(2, rev, p) loop
```

**Expected:** Alternates between `"C E G B"` and `"B G E C"` every cycle
**Actual:** Just plays `"B G E C"` forever

### Root Cause

The current `every()` implementation in `cadence-core/src/parser/builtins.rs` (lines 437-518):

```rust
let cycle = env.get("_cycle").unwrap_or(0);
if cycle % n == 0 {
    apply_transformation();
}
```

Problems:
1. `_cycle` is never injected into the environment by the playback system
2. Even if it were, the pattern is evaluated **once** and returns a static result
3. TidalCycles' `every` is fundamentally different - it creates a **meta-pattern** that tracks position internally

---

## TidalCycles Approach

In TidalCycles, `every n f p` doesn't transform the pattern immediately. Instead, it creates a pattern combinator that:

1. Tracks its own internal cycle counter
2. On each query for events at a given time arc, decides whether to apply `f` based on cycle position
3. Returns different events depending on the current cycle

This is more like creating a **pattern of patterns** that alternates.

---

## Proposed Implementation

### Option A: Pattern-Level Cycle Tracking (Recommended)

Add a new pattern wrapper that tracks cycles internally:

#### 1. Add `EveryPattern` struct in `cadence-core/src/types/pattern.rs`:

```rust
/// A pattern that applies a transformation every N cycles
#[derive(Clone, Debug)]
pub struct EveryPattern {
    /// How often to apply the transformation (every N cycles)
    pub interval: usize,
    /// The base pattern
    pub base: Pattern,
    /// The transformed pattern (pre-computed)
    pub transformed: Pattern,
}

impl EveryPattern {
    pub fn new(interval: usize, base: Pattern, transformed: Pattern) -> Self {
        Self { interval, base, transformed }
    }

    /// Get events for the given cycle number
    pub fn get_events_for_cycle(&self, cycle: usize) -> &Pattern {
        if cycle % self.interval == 0 {
            &self.transformed
        } else {
            &self.base
        }
    }
}
```

#### 2. Add `Value::EveryPattern` variant in `cadence-core/src/parser/ast.rs`:

```rust
pub enum Value {
    // ... existing variants ...
    EveryPattern(Box<EveryPattern>),
}
```

#### 3. Update `LoopingPattern` in `src/audio/event_dispatcher.rs`:

Track the current cycle and handle `Value::EveryPattern`:

```rust
pub struct LoopingPattern {
    // ... existing fields ...
    /// Current cycle count (number of complete pattern cycles)
    pub current_cycle: usize,
}

pub fn get_step_at_beat(&mut self, current_beat: f64) -> Result<Option<PlaybackStep>, anyhow::Error> {
    // ... evaluate expression ...
    
    match value {
        Value::EveryPattern(every) => {
            // Use self.current_cycle to select the right pattern
            let pattern = every.get_events_for_cycle(self.current_cycle);
            // ... rest of pattern handling ...
        }
        // ... other variants ...
    }
}
```

#### 4. Update cycle tracking in `get_step_at_beat`:

Detect when a new cycle starts and increment `current_cycle`:

```rust
// Calculate if we've started a new cycle
let beats_elapsed = (current_beat - self.start_beat) as f32;
let cycle_number = (beats_elapsed / beats_per_cycle).floor() as usize;
if cycle_number > self.current_cycle {
    self.current_cycle = cycle_number;
}
```

#### 5. Update `every()` builtin to return `Value::EveryPattern`:

```rust
// In builtins.rs every() implementation:
let transformed = evaluator.eval_with_env(call_expr, env)?;
let transformed_pattern = match transformed {
    Value::Pattern(p) => p,
    _ => return Err(anyhow!("Transform must return a pattern")),
};

Ok(Value::EveryPattern(Box::new(EveryPattern::new(
    n as usize,
    pattern,
    transformed_pattern,
))))
```

---

### Option B: Environment Injection (Simpler but less elegant)

Inject `_cycle` into the environment on each re-evaluation:

1. In `LoopingPattern::get_step_at_beat()`, before evaluating:
   ```rust
   if let Some(env) = env {
       env.set("_cycle", Value::Number(self.current_cycle as i32));
   }
   ```

2. Track `current_cycle` as described above

**Downsides:**
- Pollutes the user's environment with internal state
- Still requires the pattern to be re-evaluated to see changes
- Less composable than the pattern-combinator approach

---

## Files to Modify

| File | Changes |
|------|---------|
| `cadence-core/src/types/pattern.rs` | Add `EveryPattern` struct |
| `cadence-core/src/parser/ast.rs` | Add `Value::EveryPattern` variant |
| `cadence-core/src/parser/builtins.rs` | Update `every()` to return `EveryPattern` |
| `src/audio/event_dispatcher.rs` | Handle `Value::EveryPattern` in `get_step_at_beat`, track cycles |
| `cadence-core/src/types/mod.rs` | Export `EveryPattern` |

---

## Testing

```cadence
// Test 1: Basic alternation
let p = "C D E F"
play every(2, rev, p) loop
// Should alternate: C D E F → F E D C → C D E F → ...

// Test 2: Every 3 cycles
play every(3, fast(2), "C E G B") loop
// Cycles 1,2: C E G B (normal)
// Cycle 3: C E G B C E G B (fast)
// Repeat

// Test 3: Combined with other transforms
play every(2, rev, "C E G").fast(2) loop
// Fast pattern that reverses every other cycle
```

---

## Related Work Done This Session

- ✅ Fixed `fast()` and `slow()` with TidalCycles-style cycle position tracking
- ✅ Fixed `env()` and `wave()` pattern modifiers
- ✅ Added `PlaybackStep` struct with `duration_beats` for sub-beat timing
- ✅ Rewrote `LoopingPattern` to use `start_beat` and `last_triggered_step`
- ✅ Changed `process_tick` to check on every tick (24 PPQN), not just beat boundaries

---

## References

- [TidalCycles Pattern Combinators](https://tidalcycles.org/docs/reference/mini_notation)
- `src/audio/event_dispatcher.rs` - Current cycle tracking implementation
- `cadence-core/src/parser/builtins.rs` - Current `every()` implementation (line 437)
