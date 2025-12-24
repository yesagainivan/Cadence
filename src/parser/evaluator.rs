use crate::{
    parser::ast::{Expression, Value},
    types::{Chord, CommonProgressions, Note, RomanNumeral, VoiceLeading, analyze_progression},
};
// use crate::types::{chord::Chord, note::Note};
use anyhow::{Result, anyhow};

/// Evaluates parsed expressions into values
pub struct Evaluator;

impl Evaluator {
    /// Create a new evaluator
    pub fn new() -> Self {
        Evaluator
    }

    /// Evaluate an expression and return the result (no environment)
    pub fn eval(&self, expr: Expression) -> Result<Value> {
        self.eval_with_env(expr, None)
    }

    /// Evaluate an expression with optional environment for variable resolution
    pub fn eval_with_env(
        &self,
        expr: Expression,
        env: Option<&crate::parser::environment::Environment>,
    ) -> Result<Value> {
        match expr {
            Expression::Note(note) => Ok(Value::Note(note)),
            Expression::Chord(chord) => Ok(Value::Chord(chord)),
            Expression::Pattern(pattern) => Ok(Value::Pattern(pattern)),
            Expression::String(s) => Ok(Value::String(s)),
            Expression::Transpose { target, semitones } => {
                let target_value = self.eval_with_env(*target, env)?;
                match target_value {
                    Value::Note(note) => {
                        let transposed = note + semitones;
                        Ok(Value::Note(transposed))
                    }
                    Value::Chord(chord) => {
                        let transposed = chord + semitones;
                        Ok(Value::Chord(transposed))
                    }
                    Value::Progression(progression) => {
                        let transposed = progression + semitones;
                        Ok(Value::Progression(transposed))
                    }
                    Value::Boolean(_) => Err(anyhow!("Cannot transpose a boolean value")),
                    Value::Pattern(_) => Err(anyhow!("Cannot transpose a pattern directly")),
                    Value::Number(_) => Err(anyhow!("Cannot transpose a number")),
                    Value::String(_) => Err(anyhow!("Cannot transpose a string")),
                }
            }
            Expression::Intersection { left, right } => {
                let left_value = self.eval_with_env(*left, env)?;
                let right_value = self.eval_with_env(*right, env)?;

                match (left_value, right_value) {
                    (Value::Chord(left_chord), Value::Chord(right_chord)) => {
                        let intersection = left_chord & right_chord;
                        Ok(Value::Chord(intersection))
                    }
                    _ => Err(anyhow!("Intersection only supported between chords")),
                }
            }
            Expression::Union { left, right } => {
                let left_value = self.eval_with_env(*left, env)?;
                let right_value = self.eval_with_env(*right, env)?;

                match (left_value, right_value) {
                    (Value::Chord(left_chord), Value::Chord(right_chord)) => {
                        let union = left_chord | right_chord;
                        Ok(Value::Chord(union))
                    }
                    _ => Err(anyhow!("Union only supported between chords")),
                }
            }
            Expression::Difference { left, right } => {
                let left_value = self.eval_with_env(*left, env)?;
                let right_value = self.eval_with_env(*right, env)?;

                match (left_value, right_value) {
                    (Value::Chord(left_chord), Value::Chord(right_chord)) => {
                        let difference = left_chord ^ right_chord;
                        Ok(Value::Chord(difference))
                    }
                    _ => Err(anyhow!("Difference only supported between chords")),
                }
            }
            Expression::FunctionCall { name, args } => {
                self.eval_function_with_env(&name, args, env)
            }
            Expression::Progression(progression) => Ok(Value::Progression(progression)),
            Expression::Variable(name) => match env {
                Some(e) => e
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| anyhow!("Variable '{}' is not defined", name)),
                None => Err(anyhow!(
                    "Variable '{}' cannot be resolved (no environment)",
                    name
                )),
            },
            Expression::Boolean(b) => Ok(Value::Boolean(b)),
            Expression::Comparison {
                left,
                right,
                operator,
            } => {
                let left_val = self.eval_with_env(*left, env)?;
                let right_val = self.eval_with_env(*right, env)?;
                let result = match operator {
                    crate::parser::ast::ComparisonOp::Equal => left_val == right_val,
                    crate::parser::ast::ComparisonOp::NotEqual => left_val != right_val,
                };
                Ok(Value::Boolean(result))
            }
        }
    }

    /// Evaluate a function call with optional environment for variable resolution
    fn eval_function_with_env(
        &self,
        name: &str,
        args: Vec<Expression>,
        env: Option<&crate::parser::environment::Environment>,
    ) -> Result<Value> {
        match name {
            // Enhanced progression handling with smart pattern detection
            name if CommonProgressions::is_valid_progression(name)
                || CommonProgressions::is_numeric_progression(name)
                || CommonProgressions::is_roman_numeral_progression(name) =>
            {
                if args.len() != 1 {
                    return Err(anyhow!("Progression {} expects 1 key argument", name));
                }

                let key_value = self.eval_with_env(args[0].clone(), env)?;
                if let Value::Note(key) = key_value {
                    let prog = CommonProgressions::get_progression(name, key)?;

                    // Enhanced display message with smart formatting
                    let display_name = if CommonProgressions::is_numeric_progression(name) {
                        Self::format_numeric_progression_name(name)
                    } else if CommonProgressions::is_roman_numeral_progression(name) {
                        name.to_string() // Roman numerals display as-is
                    } else {
                        name.replace("_", "-")
                    };

                    println!("Generated {} progression in {}", display_name, key);
                    Ok(Value::Progression(prog))
                } else {
                    Err(anyhow!("Progression {} expects a key (note)", name))
                }
            }

            // Keep all existing functions...
            "invert" => {
                if args.len() != 1 {
                    return Err(anyhow!("invert() expects 1 argument, got {}", args.len()));
                }

                let arg_value = self.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Chord(chord) => {
                        let inverted = chord.invert();
                        Ok(Value::Chord(inverted))
                    }
                    Value::Progression(progression) => {
                        // Apply invert to each chord in the progression
                        let inverted = progression.map(|chord| chord.invert());
                        Ok(Value::Progression(inverted))
                    }
                    _ => Err(anyhow!("invert() only works on chords or progressions")),
                }
            }

            "invert_n" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "invert_n() expects 2 arguments, got {}",
                        args.len()
                    ));
                }

                let mut arg_iter = args.into_iter();
                let chord_expr = arg_iter.next().unwrap();
                let n_expr = arg_iter.next().unwrap();

                let chord_value = self.eval_with_env(chord_expr, env)?;
                let n_value = self.eval_with_env(n_expr, env)?;

                match (chord_value, n_value) {
                    (Value::Chord(chord), Value::Note(note)) => {
                        let n = note.pitch_class() as usize;
                        let inverted = chord.invert_n(n);
                        Ok(Value::Chord(inverted))
                    }
                    _ => Err(anyhow!("invert_n() expects (chord, note) arguments")),
                }
            }

            "root" => {
                if args.len() != 1 {
                    return Err(anyhow!("root() expects 1 argument, got {}", args.len()));
                }

                let arg_value = self.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Chord(chord) => {
                        if let Some(root_note) = chord.root() {
                            Ok(Value::Note(root_note))
                        } else {
                            Err(anyhow!("Cannot determine root of empty chord"))
                        }
                    }
                    _ => Err(anyhow!("root() only works on chords")),
                }
            }

            "bass" => {
                if args.len() != 1 {
                    return Err(anyhow!("bass() expects 1 argument, got {}", args.len()));
                }

                let arg_value = self.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Chord(chord) => {
                        if let Some(bass_note) = chord.bass() {
                            Ok(Value::Note(bass_note))
                        } else {
                            Err(anyhow!("Cannot determine bass of empty chord"))
                        }
                    }
                    _ => Err(anyhow!("bass() only works on chords")),
                }
            }

            "retrograde" => {
                if args.len() != 1 {
                    return Err(anyhow!(
                        "retrograde() expects 1 argument, got {}",
                        args.len()
                    ));
                }

                let arg_value = self.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Progression(progression) => {
                        let retrograded = progression.retrograde();
                        Ok(Value::Progression(retrograded))
                    }
                    _ => Err(anyhow!("retrograde() only works on progressions")),
                }
            }

            "map" => {
                if args.len() != 2 {
                    return Err(anyhow!("map() expects 2 arguments, got {}", args.len()));
                }

                let mut arg_iter = args.into_iter();
                let function_expr = arg_iter.next().unwrap();
                let progression_expr = arg_iter.next().unwrap();

                // Extract function name from either Variable or FunctionCall with no args
                let func_name = match &function_expr {
                    Expression::Variable(name) => name.clone(),
                    Expression::FunctionCall {
                        name,
                        args: func_args,
                    } if func_args.is_empty() => name.clone(),
                    _ => {
                        return Err(anyhow!("map() first argument must be a function name"));
                    }
                };

                let progression_value = self.eval_with_env(progression_expr, env)?;
                if let Value::Progression(progression) = progression_value {
                    let mapped = match func_name.as_str() {
                        "invert" => progression.map(|chord| chord.invert()),
                        _ => return Err(anyhow!("Unknown function for map: {}", func_name)),
                    };
                    Ok(Value::Progression(mapped))
                } else {
                    Err(anyhow!("map() second argument must be a progression"))
                }
            }

            // Voice leading functions
            "voice_leading" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "voice_leading() expects 2 arguments, got {}",
                        args.len()
                    ));
                }

                let mut arg_iter = args.into_iter();
                let chord1_expr = arg_iter.next().unwrap();
                let chord2_expr = arg_iter.next().unwrap();

                let chord1_value = self.eval_with_env(chord1_expr, env)?;
                let chord2_value = self.eval_with_env(chord2_expr, env)?;

                match (chord1_value, chord2_value) {
                    (Value::Chord(chord1), Value::Chord(chord2)) => {
                        let voice_leading = VoiceLeading::analyze(&chord1, &chord2);

                        let movement_info = format!(
                            "Voice leading: {} common tones, {} total movement, {}",
                            voice_leading.common_tones.len(),
                            voice_leading.total_movement,
                            voice_leading.voice_leading_type()
                        );

                        println!("{}", movement_info);

                        if !voice_leading.common_tones.is_empty() {
                            Ok(Value::Chord(Chord::from_notes(voice_leading.common_tones)))
                        } else {
                            Ok(Value::Chord(Chord::new()))
                        }
                    }
                    _ => Err(anyhow!("voice_leading() expects two chords")),
                }
            }

            "smooth_voice_leading" | "smooth" => {
                if args.len() != 1 {
                    return Err(anyhow!(
                        "smooth_voice_leading() expects 1 argument, got {}",
                        args.len()
                    ));
                }

                let arg_value = self.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Progression(progression) => {
                        println!("Optimizing voice leading...");

                        let original_quality = progression.average_voice_leading_quality();
                        println!("Original voice leading quality: {:.1}", original_quality);

                        let optimized = progression.optimize_voice_leading();

                        let new_quality = optimized.average_voice_leading_quality();
                        println!("Optimized voice leading quality: {:.1}", new_quality);

                        Ok(Value::Progression(optimized))
                    }
                    _ => Err(anyhow!("smooth_voice_leading() only works on progressions")),
                }
            }

            "analyze_voice_leading" => {
                if args.len() != 1 {
                    return Err(anyhow!(
                        "analyze_voice_leading() expects 1 argument, got {}",
                        args.len()
                    ));
                }

                let arg_value = self.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Progression(progression) => {
                        let analysis = progression.detailed_voice_leading_analysis();

                        println!("Voice Leading Analysis:");
                        println!("======================");
                        for analysis_item in &analysis {
                            println!("{}", analysis_item);
                            println!("  {}", analysis_item.voice_leading);
                        }

                        let avg_quality = progression.average_voice_leading_quality();
                        let has_good_vl = progression.has_good_voice_leading();

                        println!("\nOverall Analysis:");
                        println!("  Average quality score: {:.1}", avg_quality);
                        println!(
                            "  Good voice leading: {}",
                            if has_good_vl {
                                "✓ Yes"
                            } else {
                                "⚠ Needs work"
                            }
                        );

                        Ok(Value::Progression(progression))
                    }
                    _ => Err(anyhow!(
                        "analyze_voice_leading() only works on progressions"
                    )),
                }
            }

            "common_tones" | "ct" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "common_tones() expects 2 arguments, got {}",
                        args.len()
                    ));
                }

                let mut arg_iter = args.into_iter();
                let chord1_expr = arg_iter.next().unwrap();
                let chord2_expr = arg_iter.next().unwrap();

                let chord1_value = self.eval_with_env(chord1_expr, env)?;
                let chord2_value = self.eval_with_env(chord2_expr, env)?;

                match (chord1_value, chord2_value) {
                    (Value::Chord(chord1), Value::Chord(chord2)) => {
                        let voice_leading = VoiceLeading::analyze(&chord1, &chord2);
                        Ok(Value::Chord(Chord::from_notes(voice_leading.common_tones)))
                    }
                    _ => Err(anyhow!("common_tones() expects two chords")),
                }
            }

            "voice_leading_quality" => {
                if args.len() != 1 {
                    return Err(anyhow!(
                        "voice_leading_quality() expects 1 argument, got {}",
                        args.len()
                    ));
                }

                let arg_value = self.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Progression(progression) => {
                        let quality = progression.average_voice_leading_quality();
                        println!("Voice leading quality score: {:.1}", quality);

                        let quality_note = Note::new((quality.abs() as u8) % 12)?;
                        Ok(Value::Note(quality_note))
                    }
                    _ => Err(anyhow!(
                        "voice_leading_quality() only works on progressions"
                    )),
                }
            }

            // Roman numeral analysis
            "roman_numeral" | "rn" => {
                if args.len() != 2 {
                    return Err(anyhow!("roman_numeral() expects 2 arguments: chord, key"));
                }

                let chord_value = self.eval_with_env(args[0].clone(), env)?;
                let key_value = self.eval_with_env(args[1].clone(), env)?;

                match (chord_value, key_value) {
                    (Value::Chord(chord), Value::Note(key)) => {
                        match RomanNumeral::analyze_with_suggestions(&chord, key) {
                            Ok(analysis) => {
                                println!("{}", analysis.detailed_analysis());
                                Ok(Value::Chord(chord))
                            }
                            Err(e) => {
                                println!("Analysis failed: {}", e);
                                match RomanNumeral::analyze_with_context(&chord, key) {
                                    Ok(analyses) => {
                                        println!("Multiple interpretations found:");
                                        for (i, analysis) in analyses.iter().enumerate() {
                                            println!(
                                                "  {}: {}",
                                                i + 1,
                                                analysis.detailed_analysis()
                                            );
                                        }
                                        Ok(Value::Chord(chord))
                                    }
                                    Err(_) => Err(e),
                                }
                            }
                        }
                    }
                    _ => Err(anyhow!("roman_numeral() expects (chord, key)")),
                }
            }

            // Enhanced progression function with flexible syntax for Roman numerals too
            "progression" => {
                if args.len() == 2 {
                    // Extract progression name from various formats
                    let prog_name = match &args[0] {
                        Expression::FunctionCall {
                            name,
                            args: inner_args,
                        } if inner_args.is_empty() => {
                            // Could be Roman numeral or named progression
                            if CommonProgressions::is_roman_numeral_progression(name) {
                                name.clone() // Keep Roman numerals as-is
                            } else {
                                name.replace("_", "-") // Convert underscores to dashes for display
                            }
                        }
                        _ => return Err(anyhow!("progression() expects (progression_name, key)")),
                    };

                    let key_value = self.eval_with_env(args[1].clone(), env)?;
                    if let Value::Note(key) = key_value {
                        // Try both underscore and dash versions, and Roman numeral versions
                        let underscore_name = prog_name.replace("-", "_");
                        let prog =
                            CommonProgressions::get_progression(&prog_name, key).or_else(|_| {
                                CommonProgressions::get_progression(&underscore_name, key)
                            })?;

                        println!("Generated {} progression in {}", prog_name, key);
                        Ok(Value::Progression(prog))
                    } else {
                        Err(anyhow!("progression() expects (name, key)"))
                    }
                } else {
                    Err(anyhow!("progression() expects 2 arguments: name, key"))
                }
            }

            // List available progressions
            "list_progressions" => {
                if !args.is_empty() {
                    return Err(anyhow!("list_progressions() takes no arguments"));
                }

                println!("Available progressions:");
                for prog in CommonProgressions::list_progressions() {
                    println!("  {}", prog);
                }
                println!("\nUsage examples:");
                println!("  I_V_vi_IV(C)              # Named progression");
                println!("  I-V-vi-IV(C)              # Roman numeral progression");
                println!("  1564(C)                   # Numeric progression");
                println!("  progression(I-V-vi-IV, C) # Function call");

                Ok(Value::Progression(crate::types::Progression::new()))
            }

            // Enhanced progression analysis
            "analyze_progression" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "analyze_progression() expects 2 arguments: progression, key"
                    ));
                }

                let prog_value = self.eval_with_env(args[0].clone(), env)?;
                let key_value = self.eval_with_env(args[1].clone(), env)?;

                match (prog_value, key_value) {
                    (Value::Progression(progression), Value::Note(key)) => {
                        match analyze_progression(&progression, key) {
                            Ok(analysis) => {
                                println!("Roman Numeral Analysis in {} major:", key);
                                for (i, rn) in analysis.iter().enumerate() {
                                    println!("  {}: {} ({})", i + 1, rn, rn.function_description());
                                }

                                if progression.len() > 1 {
                                    let vl_quality = progression.average_voice_leading_quality();
                                    println!("\nVoice leading quality: {:.1}", vl_quality);
                                }
                            }
                            Err(e) => {
                                println!("Analysis failed: {}", e);
                                println!(
                                    "Try analyzing in a different key or check chord spellings."
                                );
                            }
                        }

                        Ok(Value::Progression(progression))
                    }
                    _ => Err(anyhow!("analyze_progression() expects (progression, key)")),
                }
            }

            // Pattern operators
            "fast" => {
                if args.len() != 2 {
                    return Err(anyhow!("fast() expects 2 arguments: pattern, factor"));
                }

                let pattern_value = self.eval_with_env(args[0].clone(), env)?;
                let factor_value = self.eval_with_env(args[1].clone(), env)?;

                match (pattern_value, factor_value) {
                    (Value::Pattern(pattern), Value::Note(note)) => {
                        // Use pitch class as factor (e.g., D = 2)
                        let factor = (note.pitch_class() as usize).max(1);
                        Ok(Value::Pattern(pattern.fast(factor)))
                    }
                    _ => Err(anyhow!("fast() expects (pattern, factor_note)")),
                }
            }

            "slow" => {
                if args.len() != 2 {
                    return Err(anyhow!("slow() expects 2 arguments: pattern, factor"));
                }

                let pattern_value = self.eval_with_env(args[0].clone(), env)?;
                let factor_value = self.eval_with_env(args[1].clone(), env)?;

                match (pattern_value, factor_value) {
                    (Value::Pattern(pattern), Value::Note(note)) => {
                        // Use pitch class as factor (e.g., D = 2)
                        let factor = (note.pitch_class() as usize).max(1);
                        Ok(Value::Pattern(pattern.slow(factor)))
                    }
                    _ => Err(anyhow!("slow() expects (pattern, factor_note)")),
                }
            }

            "rev" => {
                if args.len() != 1 {
                    return Err(anyhow!("rev() expects 1 argument: pattern"));
                }

                let arg_value = self.eval_with_env(args.into_iter().next().unwrap(), env)?;
                match arg_value {
                    Value::Pattern(pattern) => Ok(Value::Pattern(pattern.rev())),
                    _ => Err(anyhow!("rev() only works on patterns")),
                }
            }

            "every" => {
                if args.len() != 3 {
                    return Err(anyhow!(
                        "every() expects 3 arguments: intervals, function_name, pattern"
                    ));
                }

                // Parse n (intervals)
                let n_val = self.eval_with_env(args[0].clone(), env)?;
                let n = match n_val {
                    Value::Note(note) => note.pitch_class() as i32,
                    Value::Number(num) => num,
                    _ => return Err(anyhow!("every() expects a number as first argument")),
                };

                // Parse function name (transform)
                let transform_name = match &args[1] {
                    Expression::Variable(name) => name.clone(),
                    Expression::String(s) => s.clone(),
                    Expression::Pattern(p) => p.to_string(), // Fallback
                    Expression::FunctionCall {
                        name,
                        args: internal_args,
                    } if internal_args.is_empty() => name.clone(),
                    _ => {
                        return Err(anyhow!(
                            "every() expects a function name as second argument"
                        ));
                    }
                };

                // Parse pattern
                let pattern_val = self.eval_with_env(args[2].clone(), env)?;
                let pattern = match pattern_val {
                    Value::Pattern(p) => p,
                    _ => return Err(anyhow!("every() expects a pattern as third argument")),
                };

                // Get current cycle from environment
                let cycle = if let Some(e) = env {
                    match e.get("_cycle") {
                        Some(Value::Number(c)) => *c,
                        Some(Value::Note(n)) => n.pitch_class() as i32,
                        _ => 0,
                    }
                } else {
                    0
                };

                // Apply logic: if cycle % n == 0, apply transform
                if n > 0 && cycle % n == 0 {
                    // Apply the transform
                    // We need to call the function recursively with the pattern
                    // Construct args for the function call
                    // This is tricky because eval_function_with_env expects Expressions, but we have a Value::Pattern.
                    // We can wrap the pattern back into Expression::Pattern?
                    // No, Expression::Pattern holds a Pattern struct. Yes, we can.

                    // Are there any other args required by the transform?
                    // `rev` takes 1 arg (pattern).
                    // `fast` takes 2 args. `every(3, fast(2), p)`?
                    // If the user passed `fast(2)` as the transform arg, it would be a FunctionCall expression.
                    // BUT my parser parses `fast(2)` as a function call immediately.
                    // If I pass `fast` (variable), I only have the name. I don't have the factor 2.

                    // IF the user syntax is `every(3, "rev", p)`, then `rev` is just a name.
                    // If the user wants `fast 2`, they might need partial application which I don't have.
                    // Or syntax: `every(3, fast, 2, p)`? No.

                    // Let's assume for now, `every` supports only specific unary transformations referencable by name: `rev`.
                    // Or we support `every(n, function_call_expr, pattern)` where `function_call_expr` is evaluated?
                    // Tidal's `every 3 (fast 2) "..."` works because Haskell functions are curried.

                    // For now, let's support `rev` specially.
                    // If `transform_name` == "rev", apply rev.

                    match transform_name.as_str() {
                        "rev" => Ok(Value::Pattern(pattern.rev())),
                        _ => {
                            // Try calling it as a function if we can reconstruct expression?
                            // Maybe later. For now, just rev.
                            // Also support fast/slow if I can figure out how to pass args.
                            // Wait, if I use `Expression::FunctionCall` as the argument, `eval_with_env` will evaluate it!
                            // That returns a Value. I can't pass a function.

                            // LIMITATION: `every` currently only supports "rev".
                            Err(anyhow!(
                                "every() currently only supports 'rev' transformation"
                            ))
                        }
                    }
                } else {
                    Ok(Value::Pattern(pattern))
                }
            }

            _ => Err(anyhow!("Unknown function: {}", name)),
        }
    }

    /// Format numeric progression names for display with proper chord types
    pub fn format_numeric_progression_name(numeric: &str) -> String {
        let roman_numerals = ["I", "ii", "iii", "IV", "V", "vi", "vii°"];

        let formatted: Vec<String> = numeric
            .chars()
            .filter_map(|c| {
                if let Some(digit) = c.to_digit(10) {
                    if digit >= 1 && digit <= 7 {
                        Some(roman_numerals[(digit - 1) as usize].to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        if formatted.is_empty() {
            numeric.to_string()
        } else {
            formatted.join("-")
        }
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to parse and evaluate a string expression
pub fn eval(input: &str) -> Result<Value> {
    let expr = crate::parser::parse(input)?;
    let evaluator = Evaluator::new();
    evaluator.eval(expr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn test_eval_note() {
        let expr = parse("C").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Note(note) => assert_eq!(note.pitch_class(), 0),
            _ => panic!("Expected note value"),
        }
    }

    #[test]
    fn test_eval_chord() {
        let expr = parse("[C, E, G]").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                assert_eq!(chord.len(), 3);
                assert!(chord.contains(&"C".parse().unwrap()));
                assert!(chord.contains(&"E".parse().unwrap()));
                assert!(chord.contains(&"G".parse().unwrap()));
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_transpose_note() {
        let expr = parse("C + 2").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Note(note) => assert_eq!(note.pitch_class(), 2), // D
            _ => panic!("Expected note value"),
        }
    }

    #[test]
    fn test_eval_transpose_chord() {
        let expr = parse("[C, E, G] + 2").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                let pitch_classes: Vec<u8> = chord.notes().map(|n| n.pitch_class()).collect();
                assert!(pitch_classes.contains(&2)); // D
                assert!(pitch_classes.contains(&6)); // F#
                assert!(pitch_classes.contains(&9)); // A
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_intersection() {
        let expr = parse("[C, E, G] & [A, C, E]").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                assert_eq!(chord.len(), 2); // C and E
                assert!(chord.contains(&"C".parse().unwrap()));
                assert!(chord.contains(&"E".parse().unwrap()));
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_progression() {
        let expr = parse("[[C, E, G], [F, A, C]]").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Progression(progression) => {
                assert_eq!(progression.len(), 2);
                assert!(progression[0].contains(&"C".parse().unwrap()));
                assert!(progression[1].contains(&"F".parse().unwrap()));
            }
            _ => panic!("Expected progression value"),
        }
    }

    #[test]
    fn test_eval_progression_transpose() {
        let expr = parse("[[C, E, G], [F, A, C]] + 2").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Progression(progression) => {
                // First chord should be D major (C major + 2)
                let first_chord = &progression[0];
                let pitch_classes: Vec<u8> = first_chord.notes().map(|n| n.pitch_class()).collect();
                assert!(pitch_classes.contains(&2)); // D
                assert!(pitch_classes.contains(&6)); // F#
                assert!(pitch_classes.contains(&9)); // A
            }
            _ => panic!("Expected progression value"),
        }
    }

    #[test]
    fn test_eval_retrograde_function() {
        let expr = parse("retrograde([[C, E, G], [F, A, C], [G, B, D]])").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Progression(progression) => {
                assert_eq!(progression.len(), 3);
                // First chord should now be G major (was last)
                assert!(progression[0].contains(&"G".parse().unwrap()));
                assert!(progression[0].contains(&"B".parse().unwrap()));
                assert!(progression[0].contains(&"D".parse().unwrap()));
            }
            _ => panic!("Expected progression value"),
        }
    }

    #[test]
    fn test_eval_map_function() {
        let expr = parse("map(invert, [[C, E, G], [F, A, C]])").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Progression(progression) => {
                assert_eq!(progression.len(), 2);
                // First chord should be C major first inversion (E in bass, C root)
                // Compare pitch_class because octave changes during inversion
                assert_eq!(progression[0].bass().unwrap().pitch_class(), 4); // E
                assert_eq!(progression[0].root().unwrap().pitch_class(), 0); // C
            }
            _ => panic!("Expected progression value"),
        }
    }

    #[test]
    fn test_eval_union() {
        let expr = parse("[C, E, G] | [A, C, E]").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                assert_eq!(chord.len(), 4); // A, C, E, G
                assert!(chord.contains(&"A".parse().unwrap()));
                assert!(chord.contains(&"C".parse().unwrap()));
                assert!(chord.contains(&"E".parse().unwrap()));
                assert!(chord.contains(&"G".parse().unwrap()));
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_difference() {
        let expr = parse("[C, E, G] ^ [A, C, E]").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                assert_eq!(chord.len(), 2); // A and G
                assert!(chord.contains(&"A".parse().unwrap()));
                assert!(chord.contains(&"G".parse().unwrap()));
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_invert_function() {
        let expr = parse("invert([C, E, G])").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Chord(chord) => {
                // After inversion, bass becomes E (pitch class 4), root is still C (pitch class 0)
                // We compare pitch_class because the octave changes during inversion
                assert_eq!(chord.bass().unwrap().pitch_class(), 4); // E in bass
                assert_eq!(chord.root().unwrap().pitch_class(), 0); // C still root
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_root_function() {
        let expr = parse("root([C, E, G])").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Note(note) => assert_eq!(note, "C".parse().unwrap()),
            _ => panic!("Expected note value"),
        }
    }

    #[test]
    fn test_eval_bass_function() {
        let expr = parse("bass(invert([C, E, G]))").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Note(note) => assert_eq!(note, "E".parse().unwrap()),
            _ => panic!("Expected note value"),
        }
    }

    #[test]
    fn test_eval_convenience_function() {
        let result = eval("[C, E, G] + 2").unwrap();

        match result {
            Value::Chord(chord) => {
                let pitch_classes: Vec<u8> = chord.notes().map(|n| n.pitch_class()).collect();
                assert!(pitch_classes.contains(&2)); // D
                assert!(pitch_classes.contains(&6)); // F#
                assert!(pitch_classes.contains(&9)); // A
            }
            _ => panic!("Expected chord value"),
        }
    }

    #[test]
    fn test_eval_error_unknown_function() {
        let expr = parse("unknown([C, E, G])").unwrap();
        let result = Evaluator::new().eval(expr);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown function"));
    }

    #[test]
    fn test_eval_error_wrong_argument_count() {
        let expr = parse("invert([C, E, G], [F, A, C])").unwrap();
        let result = Evaluator::new().eval(expr);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("expects 1 argument")
        );
    }

    #[test]
    fn test_eval_error_wrong_argument_type() {
        let expr = parse("invert(C)").unwrap();
        let result = Evaluator::new().eval(expr);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("only works on chords")
        );
    }
}

#[cfg(test)]
mod evaluator_numeric_tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn test_eval_numeric_progression() {
        // 251(C) now works again - parser treats Number+LeftParen as function call
        let expr = parse("251(C)").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Progression(progression) => {
                assert_eq!(progression.len(), 3);
                // Should be ii-V-I in C: Dm - G - C
                let key = "C".parse().unwrap();
                let analysis = analyze_progression(&progression, key).unwrap();
                assert_eq!(analysis[0].to_string(), "ii");
                assert_eq!(analysis[1].to_string(), "V");
                assert_eq!(analysis[2].to_string(), "I");
            }
            _ => panic!("Expected progression value"),
        }
    }

    #[cfg(test)]
    mod pattern_tests {
        use super::*;
        use crate::parser::parse;

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
                        _ => panic!("Expected Note E, got {:?}", steps[0]),
                    }
                    // Last step should be C (pitch class 0)
                    match &steps[2] {
                        crate::types::PatternStep::Note(n) => assert_eq!(n.pitch_class(), 0),
                        _ => panic!("Expected Note C, got {:?}", steps[2]),
                    }
                }
                _ => panic!("Expected pattern value"),
            }
        }
    }

    #[test]
    fn test_eval_long_numeric_progression() {
        let expr = parse("16251(F)").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Progression(progression) => {
                assert_eq!(progression.len(), 5);
                let key = "F".parse().unwrap();
                let analysis = analyze_progression(&progression, key).unwrap();
                assert_eq!(analysis[0].to_string(), "I");
                assert_eq!(analysis[1].to_string(), "vi");
                assert_eq!(analysis[2].to_string(), "ii");
                assert_eq!(analysis[3].to_string(), "V");
                assert_eq!(analysis[4].to_string(), "I");
            }
            _ => panic!("Expected progression value"),
        }
    }

    #[test]
    fn test_eval_invalid_numeric_progression() {
        let expr = parse("189(C)").unwrap();
        let result = Evaluator::new().eval(expr);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid scale degree")
        );
    }

    #[test]
    fn test_eval_invert_progression() {
        // Test that invert works on progressions
        let expr = parse("invert([[C, E, G], [F, A, C]])").unwrap();
        let result = Evaluator::new().eval(expr).unwrap();

        match result {
            Value::Progression(prog) => {
                assert_eq!(prog.len(), 2);
            }
            _ => panic!("Expected Progression value"),
        }
    }
}
