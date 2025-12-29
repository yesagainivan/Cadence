# Queue Cycle Mode Implementation Handoff

## Goal
Implement true `QueueMode::Cycle` support that activates a queued pattern when the **currently playing pattern** on the same track completes its cycle.

---

## Current State

### What's Implemented

1. **Queue infrastructure is complete:**
   - `PendingLoop` struct for tracking queued patterns
   - `QueueLoop` command and `queue_loop()` method  
   - Queue mode checking in `process_tick()`
   - Working: `Beat`, `Bar`, `Beats(n)` modes

2. **Cycle tracking exists in `LoopingPattern`:**
   - `current_cycle: usize` - tracks which cycle we're on
   - `start_beat: f64` - when the pattern started
   - `beats_per_cycle` - available from pattern evaluation
   - Cycle detection at lines 218-225:
     ```rust
     let new_cycle = (beats_elapsed / beats_per_cycle).floor() as usize;
     if new_cycle > self.current_cycle {
         self.current_cycle = new_cycle;
         // ...cycle boundary detected!
     }
     ```

### What's Missing

The current `Cycle` mode in `process_tick()` is just a placeholder that behaves like `Beat`:

```rust
QueueMode::Cycle => {
    // TODO: Track cycle completions per pattern for true Cycle mode
    is_beat_boundary && tick.beat.floor() > pending.queued_at_beat.floor()
}
```

**The Gap:** When checking if a pending pattern should activate, we need to query whether the *currently active pattern on that track* has completed a cycle. But:
1. `pending_loops` and `active_loops` are separate HashMaps
2. We don't expose cycle boundary events from `active_loops`

---

## Proposed Implementation

### Option A: Query Active Pattern State (Recommended)

Add a method to check if a track's active pattern just completed a cycle:

```rust
impl EventDispatcher {
    /// Check if the active loop on a track just completed a cycle
    fn track_completed_cycle(&self, track_id: usize, tick: &ClockTick) -> bool {
        for pattern in self.active_loops.values() {
            if pattern.track_id == track_id {
                // Get beats_per_cycle from the pattern
                // This requires evaluating the pattern or caching this info
                if let Some((_, beats_per_cycle, _, _)) = pattern.cached_pattern_info {
                    let beats_elapsed = (tick.beat - pattern.start_beat) as f32;
                    let new_cycle = (beats_elapsed / beats_per_cycle).floor() as usize;
                    
                    // Check if we're at a cycle boundary
                    let cycle_position = beats_elapsed % beats_per_cycle;
                    return cycle_position < 0.05; // Within first 5% of cycle
                }
            }
        }
        false
    }
}
```

**Problem:** `cached_pattern_info` is rarely populated. Need to always cache it.

### Option B: Track Cycle Boundaries Explicitly

During `get_step_at_beat()`, emit a signal when cycle changes:

```rust
pub struct LoopingPattern {
    // ... existing fields ...
    pub cycle_just_completed: bool,  // Set to true when cycle changes
}
```

Then in `process_tick()`:
1. First, process all active patterns (sets `cycle_just_completed`)
2. Then, check pending patterns:
   ```rust
   QueueMode::Cycle => {
       // Check if active pattern on this track just completed
       self.active_loops.values().any(|p| 
           p.track_id == *track_id && p.cycle_just_completed
       )
   }
   ```
3. Reset `cycle_just_completed` after checking

### Option C: Calculate Cycle from Stored Metadata

Store `beats_per_cycle` when the pattern starts:

```rust
pub struct LoopingPattern {
    // ... existing fields ...
    pub beats_per_cycle: f32,  // Cache when pattern starts
}
```

Populate during `StartLoop` by evaluating the pattern once.

---

## Recommended Approach

**Option B + Option C hybrid:**

1. Add `beats_per_cycle: f32` to `LoopingPattern`
2. Populate it during initial evaluation in `get_step_at_beat()`
3. Add a helper `is_at_cycle_start()` method
4. In queue checking, use this helper

### Implementation Steps

1. **Modify `LoopingPattern`:**
   ```rust
   pub struct LoopingPattern {
       // ... existing ...
       pub last_known_beats_per_cycle: f32,
   }
   ```

2. **Update `get_step_at_beat()` to cache `beats_per_cycle`:**
   ```rust
   // After evaluating pattern:
   self.last_known_beats_per_cycle = pattern.beats_per_cycle;
   ```

3. **Add helper in EventDispatcher:**
   ```rust
   fn active_pattern_at_cycle_start(&self, track_id: usize, current_beat: f64) -> bool {
       for pattern in self.active_loops.values() {
           if pattern.track_id == track_id && pattern.last_known_beats_per_cycle > 0.0 {
               let beats_elapsed = (current_beat - pattern.start_beat) as f32;
               let cycle_position = beats_elapsed % pattern.last_known_beats_per_cycle;
               // At cycle start if position is very small (within tolerance)
               if cycle_position < 0.05 {
                   return true;
               }
           }
       }
       false  // No active pattern or not at cycle start
   }
   ```

4. **Update queue mode check:**
   ```rust
   QueueMode::Cycle => {
       self.active_pattern_at_cycle_start(*track_id, tick.beat)
   }
   ```

---

## Edge Cases to Handle

1. **No active pattern on track:**
   - If there's nothing playing, should we activate immediately or wait for first beat?
   - Recommendation: Activate immediately (treat like `Beat`)

2. **Pattern with non-standard duration (e.g., `.fast(2)`):**
   - `beats_per_cycle` changes with `.fast()`/`.slow()`
   - This is already handled because we evaluate the pattern each time

3. **EveryPattern:**
   - Base and transformed patterns have same `beats_per_cycle`
   - Just use base pattern's value

4. **Dynamic pattern changes (reactive):**
   - Pattern might change length mid-cycle
   - We use `last_known_beats_per_cycle` which updates on re-evaluation

---

## Test Cases Needed

```rust
#[test]
fn test_queue_mode_cycle_activation() {
    // Scenario: Pattern "C D E F" (4 beats) playing on track 1
    // Queue "G A B C" on track 1 with Cycle mode
    // Should activate when first pattern completes (at beat 4, 8, 12...)
    
    let beats_per_cycle = 4.0;
    let pattern_start_beat = 0.0;
    
    // At beat 3.9 - NOT a cycle boundary
    let current_beat = 3.9;
    let beats_elapsed = current_beat - pattern_start_beat;
    let cycle_position = beats_elapsed % beats_per_cycle;
    assert!(cycle_position > 0.1, "Should not be at cycle start");
    
    // At beat 4.0 - IS a cycle boundary
    let current_beat = 4.0;
    let beats_elapsed = current_beat - pattern_start_beat;
    let cycle_position = beats_elapsed % beats_per_cycle;
    assert!(cycle_position < 0.1, "Should be at cycle start");
}

#[test]
fn test_queue_cycle_with_fast_pattern() {
    // Pattern "C D".fast(2) has beats_per_cycle = 1.0 (not 2.0)
    let beats_per_cycle = 1.0;
    
    // Cycles complete at beats 1, 2, 3...
}
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/audio/event_dispatcher.rs` | Add `last_known_beats_per_cycle` field, helper method, update Cycle logic |

---

## Estimated Effort

- ~30-50 lines of code
- 1-2 hours including tests
- Low risk (isolated to EventDispatcher)

---

## Success Criteria

```
cadence> play "C D E F" loop       # 4-beat pattern on track 1
cadence> play "kick snare" queue cycle loop   # Queue for cycle boundary
# Should hear: C D E F kick snare kick snare...
# Pattern switches exactly at beat 4
```
