//! Rational timing types for exact musical timing
//!
//! TidalCycles-style rational time representation ensures drift-free timing
//! for polyrhythms, nested fast/slow, and long performances.

use num_rational::Ratio;

/// Exact time point using rationals (beats from origin)
/// Uses i64 for large numerator/denominator support
pub type Time = Ratio<i64>;

/// A time arc [start, end) representing a span of time
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Arc {
    pub start: Time,
    pub end: Time,
}

impl Arc {
    /// Create a new arc from start to end
    pub fn new(start: Time, end: Time) -> Self {
        Self { start, end }
    }

    /// Create an arc from integers: [n1/d1, n2/d2)
    pub fn from_parts(start_n: i64, start_d: i64, end_n: i64, end_d: i64) -> Self {
        Self {
            start: Ratio::new(start_n, start_d),
            end: Ratio::new(end_n, end_d),
        }
    }

    /// Duration of this arc
    pub fn duration(&self) -> Time {
        self.end - self.start
    }

    /// Check if a time point falls within this arc [start, end)
    pub fn contains(&self, t: Time) -> bool {
        t >= self.start && t < self.end
    }

    /// Check if this arc overlaps with another
    pub fn overlaps(&self, other: &Arc) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// Helper to create Time from a ratio n/d
#[inline]
pub fn time(n: i64, d: i64) -> Time {
    Ratio::new(n, d)
}

/// Create Time from an integer (whole beats)
#[inline]
pub fn beats(n: i64) -> Time {
    Ratio::from_integer(n)
}

/// Convert rational to f64 for audio output
#[inline]
pub fn to_f64(t: Time) -> f64 {
    *t.numer() as f64 / *t.denom() as f64
}

/// Convert rational to f32 for audio output
#[inline]
pub fn to_f32(t: Time) -> f32 {
    *t.numer() as f32 / *t.denom() as f32
}

/// Convert f64 to approximate Time (for clock tick conversion)
/// Uses a fixed denominator for reasonable precision
pub fn from_f64(f: f64) -> Time {
    // Use denominator of 9600 (LCM of common musical divisions: 24, 32, 48, etc.)
    let denom = 9600i64;
    let numer = (f * denom as f64).round() as i64;
    Ratio::new(numer, denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_creation() {
        let t = time(1, 3);
        assert_eq!(*t.numer(), 1);
        assert_eq!(*t.denom(), 3);
    }

    #[test]
    fn test_time_arithmetic() {
        let a = time(1, 3);
        let b = time(1, 6);
        let sum = a + b;
        assert_eq!(sum, time(1, 2)); // 1/3 + 1/6 = 1/2
    }

    #[test]
    fn test_arc_contains() {
        let arc = Arc::new(time(0, 1), time(1, 3));
        assert!(arc.contains(time(0, 1)));
        assert!(arc.contains(time(1, 6)));
        assert!(!arc.contains(time(1, 3))); // End is exclusive
        assert!(!arc.contains(time(1, 2)));
    }

    #[test]
    fn test_arc_duration() {
        let arc = Arc::new(time(1, 4), time(3, 4));
        assert_eq!(arc.duration(), time(1, 2));
    }

    #[test]
    fn test_conversion_roundtrip() {
        let t = time(1, 3);
        let f = to_f64(t);
        assert!((f - 0.333333333).abs() < 0.0001);
    }

    #[test]
    fn test_from_f64() {
        let t = from_f64(0.5);
        assert_eq!(to_f64(t), 0.5);
    }

    #[test]
    fn test_beats_helper() {
        assert_eq!(beats(4), time(4, 1));
    }
}
