//! EveryPattern - TidalCycles-style cycle-based pattern alternation.

use super::core::Pattern;
use std::fmt;

/// A pattern combinator that applies a transformation every N cycles.
/// This is the TidalCycles-style approach where the pattern itself
/// tracks which variant to use based on cycle position.
///
/// Unlike lazy evaluation, both patterns are pre-computed at creation time,
/// making the runtime selection fast and predictable.
#[derive(Clone, Debug, PartialEq)]
pub struct EveryPattern {
    /// How often to apply the transformation (every N cycles)
    pub interval: usize,
    /// The base (untransformed) pattern
    pub base: Pattern,
    /// The transformed pattern (pre-computed at creation time)
    pub transformed: Pattern,
}

impl EveryPattern {
    /// Create a new EveryPattern combinator
    ///
    /// # Arguments
    /// * `interval` - Apply the transformation every N cycles (1 = every cycle, 2 = every other cycle)
    /// * `base` - The original, untransformed pattern
    /// * `transformed` - The pattern with the transformation applied
    pub fn new(interval: usize, base: Pattern, transformed: Pattern) -> Self {
        Self {
            interval: interval.max(1), // Ensure interval is at least 1
            base,
            transformed,
        }
    }

    /// Get the appropriate pattern for the given absolute cycle number.
    ///
    /// For `every(N, transform, pattern)`:
    /// - Transform is applied every Nth cycle, starting from cycle N-1
    /// - `every(2, rev, p)`: base on 0, transformed on 1, base on 2, transformed on 3...
    /// - `every(3, rev, p)`: base on 0, 1, transformed on 2, base on 3, 4, transformed on 5...
    ///
    /// # Arguments
    /// * `cycle` - The current cycle number (0-indexed)
    ///
    /// # Returns
    /// A reference to either the transformed or base pattern
    pub fn get_pattern_for_cycle(&self, cycle: usize) -> &Pattern {
        // Transform on cycles where (cycle + 1) is divisible by interval
        // This gives: for interval 2, transform on cycles 1, 3, 5, 7...
        // For interval 3, transform on cycles 2, 5, 8...
        if (cycle + 1).is_multiple_of(self.interval) {
            &self.transformed
        } else {
            &self.base
        }
    }

    /// Get a clone of the pattern for the given cycle
    pub fn pattern_for_cycle(&self, cycle: usize) -> Pattern {
        self.get_pattern_for_cycle(cycle).clone()
    }

    /// Get the interval (how often the transformation is applied)
    pub fn interval(&self) -> usize {
        self.interval
    }
}

impl fmt::Display for EveryPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "every({}, transform, {})", self.interval, self.base)
    }
}
