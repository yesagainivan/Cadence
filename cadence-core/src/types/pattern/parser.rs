//! Mini-notation parser for patterns.

use super::step::PatternStep;
use crate::types::{Chord, DrumSound, Note};
use anyhow::{anyhow, Result};

/// Check if a pattern step contains actual pattern content (not just variable references)
pub fn has_non_variable_content(step: &PatternStep) -> bool {
    match step {
        PatternStep::Note(_) | PatternStep::Chord(_) | PatternStep::Rest | PatternStep::Drum(_) => {
            true
        }
        PatternStep::Group(steps) => steps.iter().any(has_non_variable_content),
        PatternStep::Repeat(inner, _) => has_non_variable_content(inner),
        PatternStep::Weighted(inner, _) => has_non_variable_content(inner),
        PatternStep::Alternation(steps) => steps.iter().any(has_non_variable_content),
        PatternStep::Euclidean(inner, _, _) => has_non_variable_content(inner),
        PatternStep::Polyrhythm(sub_patterns) => sub_patterns
            .iter()
            .any(|sub| sub.iter().any(has_non_variable_content)),
        PatternStep::Variable(_) => false,
    }
}

pub fn parse_steps(notation: &str) -> Result<Vec<PatternStep>> {
    let mut steps = Vec::new();
    let mut chars = notation.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            // Whitespace - skip
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            // Rest
            '_' => {
                chars.next();
                let step = maybe_parse_weight_and_repeat(&mut chars, PatternStep::Rest)?;
                steps.push(step);
            }
            // Alternation (slow): <C D E> plays one element per cycle
            '<' => {
                chars.next(); // consume '<'
                let alt_content = take_until_angle_bracket(&mut chars)?;
                let inner_steps = parse_steps(&alt_content)?;
                if inner_steps.is_empty() {
                    return Err(anyhow!("Alternation <> cannot be empty"));
                }
                let step = maybe_parse_weight_and_repeat(
                    &mut chars,
                    PatternStep::Alternation(inner_steps),
                )?;
                steps.push(step);
            }
            // Polyrhythm: {C D E, F G} plays multiple patterns simultaneously at their own tempos
            '{' => {
                chars.next(); // consume '{'
                let poly_content = take_until_brace(&mut chars)?;
                // Split by comma to get sub-patterns
                let sub_pattern_strs: Vec<&str> = poly_content.split(',').collect();
                if sub_pattern_strs.is_empty() {
                    return Err(anyhow!("Polyrhythm {{}} cannot be empty"));
                }
                let mut sub_patterns: Vec<Vec<PatternStep>> = Vec::new();
                for sub_str in sub_pattern_strs {
                    let sub_steps = parse_steps(sub_str.trim())?;
                    if sub_steps.is_empty() {
                        return Err(anyhow!("Polyrhythm sub-pattern cannot be empty"));
                    }
                    sub_patterns.push(sub_steps);
                }
                let step = maybe_parse_weight_and_repeat(
                    &mut chars,
                    PatternStep::Polyrhythm(sub_patterns),
                )?;
                steps.push(step);
            }
            // Group start
            '[' => {
                chars.next(); // consume '['
                let group_content = take_until_bracket(&mut chars)?;

                // Check if it's a nested group first (starts with '[' after whitespace)
                // This handles [[Bb4,D5,F5] [F4,A4,C5]] as a group containing chords
                let trimmed = group_content.trim_start();
                if trimmed.starts_with('[') {
                    // It's a nested group - parse recursively
                    let inner_steps = parse_steps(&group_content)?;
                    let step =
                        maybe_parse_weight_and_repeat(&mut chars, PatternStep::Group(inner_steps))?;
                    steps.push(step);
                } else if group_content.contains(',') {
                    // It's a chord - parse comma-separated notes
                    let note_strs: Vec<&str> = group_content.split(',').map(|s| s.trim()).collect();
                    let chord = Chord::from_note_strings(note_strs)?;
                    let step =
                        maybe_parse_weight_and_repeat(&mut chars, PatternStep::Chord(chord))?;
                    steps.push(step);
                } else {
                    // It's a group
                    let inner_steps = parse_steps(&group_content)?;
                    let step =
                        maybe_parse_weight_and_repeat(&mut chars, PatternStep::Group(inner_steps))?;
                    steps.push(step);
                }
            }
            // Note (uppercase A-G) or identifier/variable (starts with letter)
            'A'..='G' => {
                let token = take_note_or_identifier(&mut chars);
                // Uppercase start means it's likely a note - try to parse
                let step = match token.parse::<Note>() {
                    Ok(note) => PatternStep::Note(note),
                    Err(_) => {
                        // Not a valid note, treat as variable
                        PatternStep::Variable(token)
                    }
                };
                let step = maybe_parse_weight_and_repeat(&mut chars, step)?;
                steps.push(step);
            }
            // Lowercase letter - could be a flat note (a-g), a drum, or a variable
            'a'..='g' => {
                let token = take_note_or_identifier(&mut chars);
                // Check if it looks like a note (single letter + optional accidental + optional octave)
                // or a drum name, or an identifier
                let step = if let Ok(note) = token.parse::<Note>() {
                    PatternStep::Note(note)
                } else if let Some(drum) = DrumSound::from_str(&token) {
                    PatternStep::Drum(drum)
                } else {
                    // Not a valid note or drum, treat as variable
                    PatternStep::Variable(token)
                };
                let step = maybe_parse_weight_and_repeat(&mut chars, step)?;
                steps.push(step);
            }
            // Identifier starting with h-z (could be drum like 'kick', 'hh', or variable)
            'h'..='z' | 'H'..='Z' => {
                let ident = take_identifier(&mut chars);
                // Check if it's a drum name first
                let step = if let Some(drum) = DrumSound::from_str(&ident) {
                    PatternStep::Drum(drum)
                } else {
                    PatternStep::Variable(ident)
                };
                let step = maybe_parse_weight_and_repeat(&mut chars, step)?;
                steps.push(step);
            }
            // Unknown
            _ => {
                return Err(anyhow!("Unexpected character in pattern: '{}'", c));
            }
        }
    }

    Ok(steps)
}

/// Take content until matching '>', handling nested angle brackets
fn take_until_angle_bracket(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String> {
    let mut content = String::new();
    let mut depth = 1;

    while let Some(c) = chars.next() {
        match c {
            '<' => {
                depth += 1;
                content.push(c);
            }
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(content);
                }
                content.push(c);
            }
            _ => content.push(c),
        }
    }

    Err(anyhow!("Unclosed angle bracket in pattern"))
}

/// Take content until matching ']', handling nested brackets
fn take_until_bracket(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String> {
    let mut content = String::new();
    let mut depth = 1;

    while let Some(c) = chars.next() {
        match c {
            '[' => {
                depth += 1;
                content.push(c);
            }
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(content);
                }
                content.push(c);
            }
            _ => content.push(c),
        }
    }

    Err(anyhow!("Unclosed bracket in pattern"))
}

/// Take content until matching '}', handling nested braces
fn take_until_brace(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String> {
    let mut content = String::new();
    let mut depth = 1;

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                depth += 1;
                content.push(c);
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(content);
                }
                content.push(c);
            }
            _ => content.push(c),
        }
    }

    Err(anyhow!("Unclosed brace in pattern"))
}

/// Take a note token OR a longer identifier (for variable names)
/// Keeps case as-is for variable names, but uppercases for notes
fn take_note_or_identifier(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut token = String::new();

    // First char is the letter
    if let Some(c) = chars.next() {
        token.push(c);
    }

    // Check for accidental (only if the first char is A-G)
    if token.len() == 1 {
        let first_upper = token.chars().next().unwrap().to_ascii_uppercase();
        if ('A'..='G').contains(&first_upper) {
            if let Some(&c) = chars.peek() {
                if c == '#' {
                    // Sharp - consume and treat as note
                    token.push(chars.next().unwrap());
                    // Rest must be octave digits
                    while let Some(&c) = chars.peek() {
                        if c.is_ascii_digit() || c == '-' {
                            token.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    // Uppercase for note parsing
                    return token
                        .chars()
                        .next()
                        .unwrap()
                        .to_ascii_uppercase()
                        .to_string()
                        + &token[1..];
                }
            }
        }
    }

    // Continue taking alphanumeric chars (for identifiers like "cmaj", "bass")
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphanumeric() || c == '_' {
            token.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    token
}

/// Take an identifier (for variable names starting with h-z)
fn take_identifier(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut ident = String::new();

    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphanumeric() || c == '_' {
            ident.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    ident
}

/// Parse optional (n,k) Euclidean, @N weight, and *N repetition suffixes
/// Order: Euclidean first, then weight, then repeat (e.g., C(3,8)@2*3)
fn maybe_parse_weight_and_repeat(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    step: PatternStep,
) -> Result<PatternStep> {
    // Check for (n,k) Euclidean pattern first
    let step = if chars.peek() == Some(&'(') {
        chars.next(); // consume '('
        let (pulses, steps) = parse_euclidean_params(chars)?;
        PatternStep::Euclidean(Box::new(step), pulses, steps)
    } else {
        step
    };

    // Check for @N weight
    let step = if chars.peek() == Some(&'@') {
        chars.next(); // consume '@'
        let mut weight_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                weight_str.push(chars.next().unwrap());
            } else {
                break;
            }
        }
        if weight_str.is_empty() {
            return Err(anyhow!("Expected number after '@'"));
        }
        let weight: usize = weight_str.parse()?;
        if weight == 0 {
            return Err(anyhow!("Weight @0 is not allowed (use _ for rest)"));
        }
        PatternStep::Weighted(Box::new(step), weight)
    } else {
        step
    };

    // Then check for *N repeat
    if chars.peek() == Some(&'*') {
        chars.next(); // consume '*'
        let mut count_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                count_str.push(chars.next().unwrap());
            } else {
                break;
            }
        }
        if count_str.is_empty() {
            return Err(anyhow!("Expected number after '*'"));
        }
        let count: usize = count_str.parse()?;
        Ok(PatternStep::Repeat(Box::new(step), count))
    } else {
        Ok(step)
    }
}

/// Parse the (pulses,steps) parameters for Euclidean rhythms
fn parse_euclidean_params(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<(usize, usize)> {
    // Parse first number (pulses)
    let mut pulses_str = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            pulses_str.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    if pulses_str.is_empty() {
        return Err(anyhow!("Expected number for Euclidean pulses"));
    }

    // Expect comma
    if chars.next() != Some(',') {
        return Err(anyhow!("Expected ',' in Euclidean pattern (n,k)"));
    }

    // Parse second number (steps)
    let mut steps_str = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            steps_str.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    if steps_str.is_empty() {
        return Err(anyhow!("Expected number for Euclidean steps"));
    }

    // Expect closing paren
    if chars.next() != Some(')') {
        return Err(anyhow!("Expected ')' to close Euclidean pattern"));
    }

    let pulses: usize = pulses_str.parse()?;
    let steps: usize = steps_str.parse()?;

    if steps == 0 {
        return Err(anyhow!("Euclidean steps must be > 0"));
    }

    Ok((pulses, steps))
}
