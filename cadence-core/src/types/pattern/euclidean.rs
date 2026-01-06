//! Bjorklund algorithm for Euclidean rhythm generation.

/// Generate a Euclidean rhythm pattern using Bjorklund's algorithm.
/// Distributes `pulses` evenly across `steps` slots.
/// Returns a Vec<bool> where `true` = pulse, `false` = rest.
pub fn bjorklund(pulses: usize, steps: usize) -> Vec<bool> {
    if steps == 0 {
        return vec![];
    }
    if pulses >= steps {
        return vec![true; steps];
    }
    if pulses == 0 {
        return vec![false; steps];
    }

    // Standard Bjorklund algorithm using Euclidean division
    let mut pattern: Vec<Vec<bool>> = Vec::new();
    let mut remainder: Vec<Vec<bool>> = Vec::new();

    for _ in 0..pulses {
        pattern.push(vec![true]);
    }
    for _ in 0..(steps - pulses) {
        remainder.push(vec![false]);
    }

    while remainder.len() > 1 {
        let mut new_pattern = Vec::new();
        let min_len = pattern.len().min(remainder.len());

        for i in 0..min_len {
            let mut combined = pattern[i].clone();
            combined.extend(remainder[i].clone());
            new_pattern.push(combined);
        }

        let leftover_pattern: Vec<_> = pattern.into_iter().skip(min_len).collect();
        let leftover_remainder: Vec<_> = remainder.into_iter().skip(min_len).collect();

        pattern = new_pattern;
        remainder = if leftover_pattern.is_empty() {
            leftover_remainder
        } else {
            leftover_pattern
        };
    }

    // Combine remaining pattern and remainder
    let mut result: Vec<bool> = Vec::new();
    for seq in pattern {
        result.extend(seq);
    }
    for seq in remainder {
        result.extend(seq);
    }
    result
}
