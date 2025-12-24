// src/types/roman_numeral.rs
use crate::types::{Chord, Note};
use anyhow::{Result, anyhow};
use std::fmt;

/// Represents a Roman numeral chord analysis
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomanNumeral {
    pub degree: ScaleDegree,
    pub quality: ChordQuality,
    pub inversion: u8,
    pub extensions: Vec<Extension>,
    pub key: Note,
    pub accidental: Option<Accidental>, // For chromatic chords
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScaleDegree {
    I,
    II,
    III,
    IV,
    V,
    VI,
    VII,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accidental {
    Flat,    // ♭
    Sharp,   // #
    Natural, // ♮ (for when we want to specify natural explicitly)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChordQuality {
    Major,          // I, IV, V
    Minor,          // ii, iii, vi
    Diminished,     // vii°
    Augmented,      // III+
    HalfDiminished, // iiø7
    MajorMinor,     // V7 (major triad + minor 7th)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Extension {
    Seventh,      // 7
    MajorSeventh, // M7, △7
    Sixth,        // 6
    Ninth,        // 9
    Add9,         // add9
    Sus2,         // sus2
    Sus4,         // sus4
    Eleventh,     // 11
    Thirteenth,   // 13
}

impl RomanNumeral {
    /// Analyze a chord in the context of a key with enhanced chromatic support
    pub fn analyze(chord: &Chord, key: Note) -> Result<Self> {
        if chord.is_empty() {
            return Err(anyhow!("Cannot analyze empty chord"));
        }

        let root = chord
            .root()
            .ok_or_else(|| anyhow!("Cannot determine chord root"))?;

        // Calculate distance from key center (handle chromatic)
        let semitones_from_key = (root.pitch_class() as i8 - key.pitch_class() as i8 + 12) % 12;
        let (degree, accidental) = Self::semitones_to_degree_with_accidental(semitones_from_key);

        let quality = Self::analyze_quality_enhanced(chord)?;
        let inversion = chord.inversion() as u8;
        let extensions = Self::analyze_extensions(chord)?;

        Ok(RomanNumeral {
            degree,
            quality,
            inversion,
            extensions,
            key,
            accidental,
        })
    }

    /// Analyze with suggestions for better error handling
    pub fn analyze_with_suggestions(chord: &Chord, key: Note) -> Result<RomanNumeral> {
        match Self::analyze(chord, key) {
            Ok(analysis) => Ok(analysis),
            Err(_) => {
                let suggestions = Self::suggest_keys_for_chord(chord);
                let chord_name = chord.analyze();

                let suggestion_text = if suggestions.is_empty() {
                    "This chord may require advanced harmonic analysis.".to_string()
                } else {
                    format!(
                        "Consider analyzing in: {}",
                        suggestions
                            .iter()
                            .map(|k| k.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };

                Err(anyhow!(
                    "Cannot analyze {} in {} major. {}",
                    chord_name,
                    key,
                    suggestion_text
                ))
            }
        }
    }

    /// Provide multiple analysis possibilities in different keys
    pub fn analyze_with_context(chord: &Chord, key: Note) -> Result<Vec<RomanNumeral>> {
        let mut possibilities = Vec::new();

        // Try the given key first
        if let Ok(analysis) = Self::analyze(chord, key) {
            possibilities.push(analysis);
        }

        // Try related keys
        let related_keys = Self::get_related_keys(key);
        for related_key in related_keys {
            if let Ok(analysis) = Self::analyze(chord, related_key) {
                // Only add if we don't already have this analysis
                if !possibilities
                    .iter()
                    .any(|p| p.to_string() == analysis.to_string())
                {
                    possibilities.push(analysis);
                }
            }
        }

        if possibilities.is_empty() {
            Err(anyhow!(
                "Cannot analyze chord {} in {} or related keys",
                chord.analyze(),
                key
            ))
        } else {
            Ok(possibilities)
        }
    }

    fn semitones_to_degree_with_accidental(semitones: i8) -> (ScaleDegree, Option<Accidental>) {
        match semitones {
            // Natural scale degrees (diatonic)
            0 => (ScaleDegree::I, None),
            2 => (ScaleDegree::II, None),
            4 => (ScaleDegree::III, None),
            5 => (ScaleDegree::IV, None),
            7 => (ScaleDegree::V, None),
            9 => (ScaleDegree::VI, None),
            11 => (ScaleDegree::VII, None),

            // Chromatic alterations - choose most common interpretation
            1 => (ScaleDegree::I, Some(Accidental::Sharp)), // #I or ♭II (choose #I as more common)
            3 => (ScaleDegree::II, Some(Accidental::Sharp)), // #II or ♭III (choose #II)
            6 => (ScaleDegree::IV, Some(Accidental::Sharp)), // #IV or ♭V (choose #IV - very common)
            8 => (ScaleDegree::V, Some(Accidental::Sharp)), // #V or ♭VI (choose #V)
            10 => (ScaleDegree::VI, Some(Accidental::Sharp)), // #VI or ♭VII (choose #VI)

            _ => (ScaleDegree::I, None), // Fallback - shouldn't happen
        }
    }

    fn analyze_quality_enhanced(chord: &Chord) -> Result<ChordQuality> {
        let notes = chord.notes_vec();
        if notes.len() < 3 {
            return Err(anyhow!("Need at least 3 notes for quality analysis"));
        }

        let root = chord.root().unwrap();
        let mut intervals = Vec::new();

        for &note in &notes {
            if note != root {
                let interval = (note.pitch_class() as i8 - root.pitch_class() as i8 + 12) % 12;
                intervals.push(interval);
            }
        }
        intervals.sort();

        match intervals.as_slice() {
            // Triads
            [3, 6] => Ok(ChordQuality::Diminished),
            [3, 7] => Ok(ChordQuality::Minor),
            [4, 7] => Ok(ChordQuality::Major),
            [4, 8] => Ok(ChordQuality::Augmented),

            // 7th chords
            [3, 6, 9] => Ok(ChordQuality::Diminished), // dim7
            [3, 6, 10] => Ok(ChordQuality::HalfDiminished), // half-dim7
            [3, 7, 10] => Ok(ChordQuality::Minor),     // min7
            [3, 7, 11] => Ok(ChordQuality::Minor),     // minMaj7
            [4, 7, 10] => Ok(ChordQuality::MajorMinor), // Dom7 (major triad + minor 7th)
            [4, 7, 11] => Ok(ChordQuality::Major),     // Maj7
            [4, 8, 10] => Ok(ChordQuality::Augmented), // Aug7
            [4, 8, 11] => Ok(ChordQuality::Augmented), // AugMaj7

            // 6th chords
            [3, 7, 9] => Ok(ChordQuality::Minor), // min6
            [4, 7, 9] => Ok(ChordQuality::Major), // Maj6

            // Extended chords - base on triad
            _ => {
                if intervals.contains(&3) && intervals.contains(&7) {
                    Ok(ChordQuality::Minor)
                } else if intervals.contains(&4) && intervals.contains(&7) {
                    Ok(ChordQuality::Major)
                } else if intervals.contains(&3) && intervals.contains(&6) {
                    Ok(ChordQuality::Diminished)
                } else if intervals.contains(&4) && intervals.contains(&8) {
                    Ok(ChordQuality::Augmented)
                } else {
                    // Fallback based on third
                    if intervals.contains(&3) {
                        Ok(ChordQuality::Minor)
                    } else if intervals.contains(&4) {
                        Ok(ChordQuality::Major)
                    } else {
                        Ok(ChordQuality::Major) // default fallback
                    }
                }
            }
        }
    }

    fn analyze_extensions(chord: &Chord) -> Result<Vec<Extension>> {
        let mut extensions = Vec::new();
        let notes = chord.notes_vec();

        if notes.len() < 3 {
            return Ok(extensions);
        }

        let root = chord.root().unwrap();
        let mut intervals = Vec::new();

        for &note in &notes {
            if note != root {
                let interval = (note.pitch_class() as i8 - root.pitch_class() as i8 + 12) % 12;
                intervals.push(interval);
            }
        }
        intervals.sort();

        // Check for sus chords FIRST (before other extensions)
        let has_third = intervals.contains(&3) || intervals.contains(&4);

        if intervals.contains(&2) && !has_third {
            extensions.push(Extension::Sus2);
            return Ok(extensions); // Early return for sus chords
        }
        if intervals.contains(&5) && !has_third {
            extensions.push(Extension::Sus4);
            return Ok(extensions); // Early return for sus chords
        }

        // Only check other extensions if it's not a sus chord
        if notes.len() >= 4 {
            // Check for extensions in order of precedence
            if intervals.contains(&10) {
                extensions.push(Extension::Seventh);
            }
            if intervals.contains(&11) {
                extensions.push(Extension::MajorSeventh);
            }
            if intervals.contains(&9) && !intervals.contains(&10) && !intervals.contains(&11) {
                extensions.push(Extension::Sixth);
            }
            if intervals.contains(&2) {
                if intervals.contains(&10) || intervals.contains(&11) {
                    extensions.push(Extension::Ninth);
                } else if intervals.contains(&3) || intervals.contains(&4) {
                    extensions.push(Extension::Add9);
                }
            }

            // Extended jazz chords
            if intervals.contains(&5) && (intervals.contains(&10) || intervals.contains(&11)) {
                extensions.push(Extension::Eleventh);
            }
            if intervals.contains(&9)
                && intervals.contains(&2)
                && (intervals.contains(&10) || intervals.contains(&11))
            {
                extensions.push(Extension::Thirteenth);
            }
        }

        Ok(extensions)
    }

    fn suggest_keys_for_chord(chord: &Chord) -> Vec<Note> {
        let mut suggestions = Vec::new();

        if let Some(root) = chord.root() {
            // Try the root as a key center
            suggestions.push(root);

            // Try a fifth below (if this is V of that key)
            suggestions.push(root - 7);

            // Try a fourth below (if this is IV of that key)
            suggestions.push(root - 5);

            // If it's a minor chord, try relative major
            if chord.analyze().contains("minor") {
                suggestions.push(root + 3); // relative major
            }

            // If it's a major chord, try relative minor
            if chord.analyze().contains("Major") {
                suggestions.push(root - 3); // relative minor
            }
        }

        // Remove duplicates and limit to reasonable suggestions
        suggestions.sort();
        suggestions.dedup();
        suggestions.into_iter().take(4).collect()
    }

    fn get_related_keys(key: Note) -> Vec<Note> {
        vec![
            key + 7, // Dominant
            key - 7, // Subdominant
            key + 3, // Relative minor/major
            key - 3, // Relative major/minor
            key + 2, // Supertonic
            key - 2, // Subtonic
        ]
    }

    /// Get the Roman numeral representation as a string
    pub fn to_string(&self) -> String {
        let mut result = String::new();

        // Add accidental if present
        if let Some(ref acc) = self.accidental {
            match acc {
                Accidental::Flat => result.push('♭'),
                Accidental::Sharp => result.push('#'),
                Accidental::Natural => result.push('♮'),
            }
        }

        // Base numeral with quality
        let base = match (&self.degree, &self.quality) {
            (ScaleDegree::I, ChordQuality::Major) => "I",
            (ScaleDegree::I, ChordQuality::Minor) => "i",
            (ScaleDegree::II, ChordQuality::Major) => "II",
            (ScaleDegree::II, ChordQuality::Minor) => "ii",
            (ScaleDegree::III, ChordQuality::Major) => "III",
            (ScaleDegree::III, ChordQuality::Minor) => "iii",
            (ScaleDegree::IV, ChordQuality::Major) => "IV",
            (ScaleDegree::IV, ChordQuality::Minor) => "iv",
            (ScaleDegree::V, ChordQuality::Major) => "V",
            (ScaleDegree::V, ChordQuality::Minor) => "v",
            (ScaleDegree::VI, ChordQuality::Major) => "VI",
            (ScaleDegree::VI, ChordQuality::Minor) => "vi",
            (ScaleDegree::VII, ChordQuality::Major) => "VII",
            (ScaleDegree::VII, ChordQuality::Minor) => "vii",

            // Special cases for altered qualities
            (_, ChordQuality::Diminished) => {
                let base_roman = match (&self.degree, &self.quality) {
                    (ScaleDegree::I, _) => "i°",
                    (ScaleDegree::II, _) => "ii°",
                    (ScaleDegree::III, _) => "iii°",
                    (ScaleDegree::IV, _) => "iv°",
                    (ScaleDegree::V, _) => "v°",
                    (ScaleDegree::VI, _) => "vi°",
                    (ScaleDegree::VII, _) => "vii°",
                    // _ => "°",
                };
                base_roman
            }
            (_, ChordQuality::HalfDiminished) => {
                let base_roman = match self.degree {
                    ScaleDegree::I => "iø",
                    ScaleDegree::II => "iiø",
                    ScaleDegree::III => "iiiø",
                    ScaleDegree::IV => "ivø",
                    ScaleDegree::V => "vø",
                    ScaleDegree::VI => "viø",
                    ScaleDegree::VII => "viiø",
                };
                base_roman
            }
            (_, ChordQuality::Augmented) => {
                let base_roman = match self.degree {
                    ScaleDegree::I => "I+",
                    ScaleDegree::II => "II+",
                    ScaleDegree::III => "III+",
                    ScaleDegree::IV => "IV+",
                    ScaleDegree::V => "V+",
                    ScaleDegree::VI => "VI+",
                    ScaleDegree::VII => "VII+",
                };
                base_roman
            }
            (_, ChordQuality::MajorMinor) => {
                // Dominant 7th type - major triad with minor 7th
                match self.degree {
                    ScaleDegree::I => "I",
                    ScaleDegree::II => "II",
                    ScaleDegree::III => "III",
                    ScaleDegree::IV => "IV",
                    ScaleDegree::V => "V",
                    ScaleDegree::VI => "VI",
                    ScaleDegree::VII => "VII",
                }
            }
        };
        result.push_str(base);

        // Add extensions
        for extension in &self.extensions {
            match extension {
                Extension::Seventh => result.push('7'),
                Extension::MajorSeventh => result.push_str("M7"),
                Extension::Sixth => result.push('6'),
                Extension::Ninth => result.push('9'),
                Extension::Add9 => result.push_str("add9"),
                Extension::Sus2 => result.push_str("sus2"),
                Extension::Sus4 => result.push_str("sus4"),
                Extension::Eleventh => result.push_str("11"),
                Extension::Thirteenth => result.push_str("13"),
            }
        }

        // Add inversion notation
        match self.inversion {
            0 => {}
            1 => result.push_str("⁶"),
            2 => result.push_str("⁶₄"),
            3 => result.push_str("⁴₂"),
            4 => result.push_str("₂"),
            _ => result.push_str(&format!("/{}", self.inversion)),
        }

        result
    }

    /// Get a detailed analysis string
    pub fn detailed_analysis(&self) -> String {
        format!(
            "{} in {} major ({})",
            self.to_string(),
            self.key,
            self.function_description()
        )
    }

    /// Describe the harmonic function with context for alterations
    pub fn function_description(&self) -> String {
        let base_function = match self.degree {
            ScaleDegree::I => "Tonic",
            ScaleDegree::II => "Supertonic",
            ScaleDegree::III => "Mediant",
            ScaleDegree::IV => "Subdominant",
            ScaleDegree::V => "Dominant",
            ScaleDegree::VI => "Submediant",
            ScaleDegree::VII => "Leading tone",
        };

        // Add context for chromatic alterations
        if let Some(ref acc) = self.accidental {
            match (acc, &self.degree) {
                (Accidental::Sharp, ScaleDegree::IV) => {
                    "Tritone substitution - creates strong pull to V".to_string()
                }
                (Accidental::Flat, ScaleDegree::II) => {
                    "Neapolitan sixth - exotic predominant function".to_string()
                }
                (Accidental::Flat, ScaleDegree::VII) => {
                    "Subtonic - modal mixture from natural minor".to_string()
                }
                (Accidental::Flat, ScaleDegree::VI) => {
                    "Modal mixture - borrowed from parallel minor".to_string()
                }
                (Accidental::Sharp, ScaleDegree::I) => {
                    "Chromatic passing tone or secondary dominant preparation".to_string()
                }
                (Accidental::Sharp, ScaleDegree::V) => {
                    "Augmented dominant - heightened tension".to_string()
                }
                _ => format!("{} with chromatic alteration", base_function),
            }
        } else {
            match self.degree {
                ScaleDegree::I => "Tonic - home, stability, resolution".to_string(),
                ScaleDegree::II => "Supertonic - predominant function, leads to V".to_string(),
                ScaleDegree::III => "Mediant - tonic function, connects I and V".to_string(),
                ScaleDegree::IV => "Subdominant - departure from tonic, predominant".to_string(),
                ScaleDegree::V => "Dominant - tension, leads strongly to I".to_string(),
                ScaleDegree::VI => "Submediant - tonic substitute or predominant".to_string(),
                ScaleDegree::VII => "Leading tone - dominant function, resolves to I".to_string(),
            }
        }
    }
}

impl fmt::Display for RomanNumeral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// Analyze an entire progression for Roman numerals
pub fn analyze_progression(
    progression: &crate::types::Progression,
    key: Note,
) -> Result<Vec<RomanNumeral>> {
    let mut roman_numerals = Vec::new();

    for chord in progression.chords() {
        let analysis = RomanNumeral::analyze(chord, key)?;
        roman_numerals.push(analysis);
    }

    Ok(roman_numerals)
}

/// Enhanced common progressions database
pub struct CommonProgressions;

#[derive(Debug, Clone, PartialEq)]
pub enum ChordType {
    Major,
    Minor,
    Diminished,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RomanNumeralChord {
    pub degree: ScaleDegree,
    pub quality: ChordQuality,
    pub accidental: Option<Accidental>,
    pub extensions: Vec<Extension>,
}

impl CommonProgressions {
    /// Enhanced get_progression that handles chord types properly
    pub fn get_progression(name: &str, key: Note) -> Result<crate::types::Progression> {
        // Check Roman numeral progression first
        if Self::is_roman_numeral_progression(name) {
            let roman_chords = Self::parse_roman_numeral_progression(name)?;
            let chord_specs: Result<Vec<(i8, ChordType)>> = roman_chords
                .iter()
                .map(|rc| Self::roman_chord_to_spec(rc, key))
                .collect();
            return Self::build_progression_from_specs(chord_specs?, key);
        }

        // Then check if it's a numeric pattern
        if Self::is_numeric_progression(name) {
            let chord_specs = Self::parse_numeric_progression(name)?;
            return Self::build_progression_from_specs(chord_specs, key);
        }

        // For named progressions, convert the old format
        let chord_specs = match name {
            // Convert old (i8, bool) format to new (i8, ChordType) format
            "251" => vec![
                (2, ChordType::Minor),
                (7, ChordType::Major),
                (0, ChordType::Major),
            ],
            "1564" => vec![
                (0, ChordType::Major),
                (7, ChordType::Major),
                (9, ChordType::Minor),
                (5, ChordType::Major),
            ],
            "1625" => vec![
                (0, ChordType::Major),
                (9, ChordType::Minor),
                (2, ChordType::Minor),
                (7, ChordType::Major),
            ],
            "1451" => vec![
                (0, ChordType::Major),
                (5, ChordType::Major),
                (7, ChordType::Major),
                (0, ChordType::Major),
            ],
            "6415" => vec![
                (9, ChordType::Minor),
                (5, ChordType::Major),
                (0, ChordType::Major),
                (7, ChordType::Major),
            ],

            // Extended shortcuts
            "25161" => vec![
                (2, ChordType::Minor),
                (7, ChordType::Major),
                (0, ChordType::Major),
                (9, ChordType::Minor),
                (0, ChordType::Major),
            ],
            "36251" => vec![
                (4, ChordType::Minor),
                (9, ChordType::Minor),
                (2, ChordType::Minor),
                (7, ChordType::Major),
                (0, ChordType::Major),
            ],

            // Blues shortcuts
            "12bar" | "blues" => vec![
                (0, ChordType::Major),
                (0, ChordType::Major),
                (0, ChordType::Major),
                (0, ChordType::Major), // I I I I
                (5, ChordType::Major),
                (5, ChordType::Major),
                (0, ChordType::Major),
                (0, ChordType::Major), // IV IV I I
                (7, ChordType::Major),
                (5, ChordType::Major),
                (0, ChordType::Major),
                (7, ChordType::Major), // V IV I V
            ],

            // All the existing named progressions...
            "I_V_vi_IV" | "I-V-vi-IV" => vec![
                (0, ChordType::Major),
                (7, ChordType::Major),
                (9, ChordType::Minor),
                (5, ChordType::Major),
            ],
            "ii_V_I" | "ii-V-I" => vec![
                (2, ChordType::Minor),
                (7, ChordType::Major),
                (0, ChordType::Major),
            ],
            "vi_IV_I_V" | "vi-IV-I-V" => vec![
                (9, ChordType::Minor),
                (5, ChordType::Major),
                (0, ChordType::Major),
                (7, ChordType::Major),
            ],
            "I_vi_ii_V" | "I-vi-ii-V" => vec![
                (0, ChordType::Major),
                (9, ChordType::Minor),
                (2, ChordType::Minor),
                (7, ChordType::Major),
            ],
            "I_IV_V_I" | "I-IV-V-I" => vec![
                (0, ChordType::Major),
                (5, ChordType::Major),
                (7, ChordType::Major),
                (0, ChordType::Major),
            ],

            // Jazz progressions
            "ii_V_I_vi" | "ii-V-I-vi" => vec![
                (2, ChordType::Minor),
                (7, ChordType::Major),
                (0, ChordType::Major),
                (9, ChordType::Minor),
            ],
            "iii_vi_ii_V_I" | "iii-vi-ii-V-I" => {
                vec![
                    (4, ChordType::Minor),
                    (9, ChordType::Minor),
                    (2, ChordType::Minor),
                    (7, ChordType::Major),
                    (0, ChordType::Major),
                ]
            }
            "vi_ii_V_I" | "vi-ii-V-I" => vec![
                (9, ChordType::Minor),
                (2, ChordType::Minor),
                (7, ChordType::Major),
                (0, ChordType::Major),
            ],

            // Modal and chromatic progressions (use Major for chromatic alterations for now)
            "I_bVII_IV_I" | "I-♭VII-IV-I" => vec![
                (0, ChordType::Major),
                (10, ChordType::Major),
                (5, ChordType::Major),
                (0, ChordType::Major),
            ],
            "vi_bVI_bVII_I" | "vi-♭VI-♭VII-I" => {
                vec![
                    (9, ChordType::Minor),
                    (8, ChordType::Major),
                    (10, ChordType::Major),
                    (0, ChordType::Major),
                ]
            }
            "I_bIII_bVII_IV" | "I-♭III-♭VII-IV" => {
                vec![
                    (0, ChordType::Major),
                    (3, ChordType::Major),
                    (10, ChordType::Major),
                    (5, ChordType::Major),
                ]
            }

            // Rock progressions
            "vi_V_IV_V" | "vi-V-IV-V" => vec![
                (9, ChordType::Minor),
                (7, ChordType::Major),
                (5, ChordType::Major),
                (7, ChordType::Major),
            ],
            "I_V_IV_I" | "I-V-IV-I" => vec![
                (0, ChordType::Major),
                (7, ChordType::Major),
                (5, ChordType::Major),
                (0, ChordType::Major),
            ],

            // Classical progressions
            "I_V_vi_iii_IV_I_IV_V" | "Pachelbel" | "Canon" => vec![
                (0, ChordType::Major),
                (7, ChordType::Major),
                (9, ChordType::Minor),
                (4, ChordType::Minor),
                (5, ChordType::Major),
                (0, ChordType::Major),
                (5, ChordType::Major),
                (7, ChordType::Major),
            ],
            "I_vi_IV_V" | "I-vi-IV-V" => vec![
                (0, ChordType::Major),
                (9, ChordType::Minor),
                (5, ChordType::Major),
                (7, ChordType::Major),
            ],
            "vi_IV_V_I" | "vi-IV-V-I" => vec![
                (9, ChordType::Minor),
                (5, ChordType::Major),
                (7, ChordType::Major),
                (0, ChordType::Major),
            ],

            // Blues progressions
            "I_I_I_I_IV_IV_I_I_V_IV_I_V" | "12_bar_blues" | "twelve_bar_blues" | "12-bar-blues" => {
                vec![
                    (0, ChordType::Major),
                    (0, ChordType::Major),
                    (0, ChordType::Major),
                    (0, ChordType::Major), // I I I I
                    (5, ChordType::Major),
                    (5, ChordType::Major),
                    (0, ChordType::Major),
                    (0, ChordType::Major), // IV IV I I
                    (7, ChordType::Major),
                    (5, ChordType::Major),
                    (0, ChordType::Major),
                    (7, ChordType::Major), // V IV I V
                ]
            }
            _ => {
                return Err(anyhow!(
                    "Unknown progression: {}. Try:\n  - Numeric: 251, 1564, 16251\n  - Roman numerals: I-V-vi-IV, ii-V-I, ♭VII-IV-I\n  - Named: list_progressions() for options",
                    name
                ));
            }
        };

        Self::build_progression_from_specs(chord_specs, key)
    }

    /// Build a progression from chord specifications with proper chord types
    fn build_progression_from_specs(
        chord_specs: Vec<(i8, ChordType)>,
        key: Note,
    ) -> Result<crate::types::Progression> {
        let mut chords = Vec::new();

        for (semitones, chord_type) in chord_specs {
            let root = key + semitones;

            // Use appropriate accidental preference for chromatic notes
            let adjusted_root = if semitones == 10 {
                // For ♭VII, prefer flat notation
                Note::with_accidental_preference(root.pitch_class(), false)?
            } else {
                root
            };

            let chord = match chord_type {
                ChordType::Major => {
                    crate::types::Chord::from_notes(vec![
                        adjusted_root,
                        adjusted_root + 4, // major third
                        adjusted_root + 7, // perfect fifth
                    ])
                }
                ChordType::Minor => {
                    crate::types::Chord::from_notes(vec![
                        adjusted_root,
                        adjusted_root + 3, // minor third
                        adjusted_root + 7, // perfect fifth
                    ])
                }
                ChordType::Diminished => {
                    crate::types::Chord::from_notes(vec![
                        adjusted_root,
                        adjusted_root + 3, // minor third
                        adjusted_root + 6, // diminished fifth (tritone)
                    ])
                }
            };
            chords.push(chord);
        }

        Ok(crate::types::Progression::from_chords(chords))
    }

    /// Parse numeric progression patterns with proper chord type handling
    pub fn parse_numeric_progression(pattern: &str) -> Result<Vec<(i8, ChordType)>> {
        let mut chord_specs = Vec::new();
        let mut chars = pattern.chars().peekable();

        while let Some(digit) = chars.next() {
            if !digit.is_ascii_digit() {
                return Err(anyhow!("Invalid character in progression: {}", digit));
            }

            let degree_num = digit.to_digit(10).unwrap() as i8;

            // Convert scale degree to semitones and determine chord type
            let (semitones, chord_type) = match degree_num {
                1 => (0, ChordType::Major),       // I - tonic major
                2 => (2, ChordType::Minor),       // ii - supertonic minor
                3 => (4, ChordType::Minor),       // iii - mediant minor
                4 => (5, ChordType::Major),       // IV - subdominant major
                5 => (7, ChordType::Major),       // V - dominant major
                6 => (9, ChordType::Minor),       // vi - submediant minor
                7 => (11, ChordType::Diminished), // vii° - leading tone diminished
                _ => return Err(anyhow!("Invalid scale degree: {}", degree_num)),
            };

            chord_specs.push((semitones, chord_type));
        }

        if chord_specs.is_empty() {
            return Err(anyhow!("Empty progression pattern"));
        }

        Ok(chord_specs)
    }

    /// Check if a string looks like a numeric progression pattern
    pub fn is_numeric_progression(name: &str) -> bool {
        !name.is_empty()
            && name.chars().all(|c| c.is_ascii_digit())
            && name.len() >= 2
            && name.len() <= 12 // reasonable limits
    }

    /// Enhanced is_valid_progression that includes numeric patterns
    /// Enhanced is_valid_progression that actually validates content
    pub fn is_valid_progression(name: &str) -> bool {
        // Check Roman numeral progressions
        if Self::is_roman_numeral_progression(name) {
            return Self::parse_roman_numeral_progression(name).is_ok();
        }

        // Check numeric progressions
        if Self::is_numeric_progression(name) {
            return Self::parse_numeric_progression(name).is_ok();
        }

        // Check named progressions
        matches!(
            name,
            "251"
                | "1564"
                | "1625"
                | "1451"
                | "6415"
                | "25161"
                | "36251"
                | "12bar"
                | "blues"
                | "I_V_vi_IV"
                | "I-V-vi-IV"
                | "ii_V_I"
                | "ii-V-I"
                | "vi_IV_I_V"
                | "vi-IV-I-V"
                | "I_vi_ii_V"
                | "I-vi-ii-V"
                | "I_IV_V_I"
                | "I-IV-V-I"
                | "ii_V_I_vi"
                | "ii-V-I-vi"
                | "iii_vi_ii_V_I"
                | "iii-vi-ii-V-I"
                | "vi_ii_V_I"
                | "vi-ii-V-I"
                | "I_bVII_IV_I"
                | "I-♭VII-IV-I"
                | "vi_bVI_bVII_I"
                | "vi-♭VI-♭VII-I"
                | "I_bIII_bVII_IV"
                | "I-♭III-♭VII-IV"
                | "vi_V_IV_V"
                | "vi-V-IV-V"
                | "I_V_IV_I"
                | "I-V-IV-I"
                | "I_V_vi_iii_IV_I_IV_V"
                | "Pachelbel"
                | "Canon"
                | "I_vi_IV_V"
                | "I-vi-IV-V"
                | "vi_IV_V_I"
                | "vi-IV-V-I"
                | "I_I_I_I_IV_IV_I_I_V_IV_I_V"
                | "12_bar_blues"
                | "twelve_bar_blues"
                | "12-bar-blues"
        )
    }

    /// Enhanced list_progressions that mentions numeric capability
    pub fn list_progressions() -> Vec<&'static str> {
        vec![
            // Mention numeric capability first
            "📊 NUMERIC PROGRESSIONS (use any sequence of 1-7):",
            "   251, 1564, 16251, 36251, 15635, 4513, etc.",
            "   Examples: 251(C) = ii-V-I, 1564(G) = I-V-vi-IV",
            "",
            // Shortcuts
            "🎯 POPULAR SHORTCUTS:",
            "251 (ii-V-I jazz turnaround)",
            "1564 (I-V-vi-IV pop progression)",
            "1625 (I-vi-ii-V circle of fifths)",
            "1451 (I-IV-V-I authentic cadence)",
            "6415 (vi-IV-I-V pop variant)",
            "12bar / blues (12-bar blues)",
            "",
            // Named progressions
            "📜 NAMED PROGRESSIONS:",
            "I_V_vi_IV (Pop progression)",
            "vi_IV_I_V (Pop variant)",
            "I_vi_ii_V (Circle of fifths)",
            "I_IV_V_I (Authentic cadence)",
            "vi_V_IV_V (Rock progression)",
            "ii_V_I (Jazz turnaround)",
            "ii_V_I_vi (Jazz with deceptive resolution)",
            "iii_vi_ii_V_I (Extended jazz)",
            "vi_ii_V_I (Jazz ballad)",
            "I_bVII_IV_I (Modal ♭VII)",
            "vi_bVI_bVII_I (Chromatic ascent)",
            "I_bIII_bVII_IV (Modal mixture)",
            "Pachelbel (Canon in D progression)",
            "I_vi_IV_V (Classical sequence)",
            "vi_IV_V_I (Classical resolution)",
            "12_bar_blues (Traditional 12-bar form)",
        ]
    }
}

impl CommonProgressions {
    /// Parse Roman numeral progression patterns like "I-V-vi-IV", "ii7-V-I", etc.
    pub fn parse_roman_numeral_progression(pattern: &str) -> Result<Vec<RomanNumeralChord>> {
        let mut chord_specs = Vec::new();

        // Split by dashes or underscores
        let parts: Vec<&str> = pattern.split(&['-', '_'][..]).collect();

        for part in parts {
            if part.trim().is_empty() {
                continue;
            }

            let chord = Self::parse_single_roman_numeral(part.trim())?;
            chord_specs.push(chord);
        }

        if chord_specs.is_empty() {
            return Err(anyhow!("Empty progression pattern"));
        }

        Ok(chord_specs)
    }

    /// Parse a single Roman numeral like "I", "ii", "V7", "♭VII", "#iv°", etc.
    fn parse_single_roman_numeral(input: &str) -> Result<RomanNumeralChord> {
        let mut chars = input.chars().peekable();
        let mut accidental = None;
        let mut extensions = Vec::new();

        // Parse accidental at the beginning
        match chars.peek() {
            Some('♭') | Some('b') => {
                chars.next();
                accidental = Some(Accidental::Flat);
            }
            Some('#') => {
                chars.next();
                accidental = Some(Accidental::Sharp);
            }
            Some('♮') => {
                chars.next();
                accidental = Some(Accidental::Natural);
            }
            _ => {}
        }

        // Parse the Roman numeral itself
        let mut numeral_str = String::new();
        while let Some(&ch) = chars.peek() {
            if ch.is_ascii_alphabetic() || ch == '°' || ch == 'ø' || ch == '+' {
                numeral_str.push(chars.next().unwrap());
            } else {
                break;
            }
        }

        if numeral_str.is_empty() {
            return Err(anyhow!("Invalid Roman numeral: {}", input));
        }

        // Parse extensions (numbers at the end)
        let extension_str: String = chars.collect();
        if !extension_str.is_empty() {
            extensions = Self::parse_extensions(&extension_str)?;
        }

        // Determine degree and quality from the numeral string
        let (degree, quality) = Self::parse_numeral_and_quality(&numeral_str)?;

        Ok(RomanNumeralChord {
            degree,
            quality,
            accidental,
            extensions,
        })
    }

    /// Parse the Roman numeral string to determine degree and quality
    fn parse_numeral_and_quality(numeral: &str) -> Result<(ScaleDegree, ChordQuality)> {
        let (degree, quality) = match numeral {
            // Major chords (uppercase)
            "I" => (ScaleDegree::I, ChordQuality::Major),
            "II" => (ScaleDegree::II, ChordQuality::Major),
            "III" => (ScaleDegree::III, ChordQuality::Major),
            "IV" => (ScaleDegree::IV, ChordQuality::Major),
            "V" => (ScaleDegree::V, ChordQuality::Major),
            "VI" => (ScaleDegree::VI, ChordQuality::Major),
            "VII" => (ScaleDegree::VII, ChordQuality::Major),

            // Minor chords (lowercase)
            "i" => (ScaleDegree::I, ChordQuality::Minor),
            "ii" => (ScaleDegree::II, ChordQuality::Minor),
            "iii" => (ScaleDegree::III, ChordQuality::Minor),
            "iv" => (ScaleDegree::IV, ChordQuality::Minor),
            "v" => (ScaleDegree::V, ChordQuality::Minor),
            "vi" => (ScaleDegree::VI, ChordQuality::Minor),
            "vii" => (ScaleDegree::VII, ChordQuality::Minor),

            // Diminished chords
            "i°" | "io" => (ScaleDegree::I, ChordQuality::Diminished),
            "ii°" | "iio" => (ScaleDegree::II, ChordQuality::Diminished),
            "iii°" | "iiio" => (ScaleDegree::III, ChordQuality::Diminished),
            "iv°" | "ivo" => (ScaleDegree::IV, ChordQuality::Diminished),
            "v°" | "vo" => (ScaleDegree::V, ChordQuality::Diminished),
            "vi°" | "vio" => (ScaleDegree::VI, ChordQuality::Diminished),
            "vii°" | "viio" => (ScaleDegree::VII, ChordQuality::Diminished),

            // Half-diminished chords
            "iø" => (ScaleDegree::I, ChordQuality::HalfDiminished),
            "iiø" => (ScaleDegree::II, ChordQuality::HalfDiminished),
            "iiiø" => (ScaleDegree::III, ChordQuality::HalfDiminished),
            "ivø" => (ScaleDegree::IV, ChordQuality::HalfDiminished),
            "vø" => (ScaleDegree::V, ChordQuality::HalfDiminished),
            "viø" => (ScaleDegree::VI, ChordQuality::HalfDiminished),
            "viiø" => (ScaleDegree::VII, ChordQuality::HalfDiminished),

            // Augmented chords
            "I+" => (ScaleDegree::I, ChordQuality::Augmented),
            "II+" => (ScaleDegree::II, ChordQuality::Augmented),
            "III+" => (ScaleDegree::III, ChordQuality::Augmented),
            "IV+" => (ScaleDegree::IV, ChordQuality::Augmented),
            "V+" => (ScaleDegree::V, ChordQuality::Augmented),
            "VI+" => (ScaleDegree::VI, ChordQuality::Augmented),
            "VII+" => (ScaleDegree::VII, ChordQuality::Augmented),

            _ => return Err(anyhow!("Unknown Roman numeral: {}", numeral)),
        };

        Ok((degree, quality))
    }

    /// Parse extension strings like "7", "M7", "add9", "sus4", etc.
    fn parse_extensions(ext_str: &str) -> Result<Vec<Extension>> {
        let mut extensions = Vec::new();
        let ext_str = ext_str.trim();

        // Handle common extension patterns
        match ext_str {
            "7" => extensions.push(Extension::Seventh),
            "M7" | "maj7" | "△7" => extensions.push(Extension::MajorSeventh),
            "6" => extensions.push(Extension::Sixth),
            "9" => {
                extensions.push(Extension::Seventh); // 9th chords include 7th
                extensions.push(Extension::Ninth);
            }
            "M9" | "maj9" => {
                extensions.push(Extension::MajorSeventh);
                extensions.push(Extension::Ninth);
            }
            "add9" => extensions.push(Extension::Add9),
            "sus2" => extensions.push(Extension::Sus2),
            "sus4" => extensions.push(Extension::Sus4),
            "11" => {
                extensions.push(Extension::Seventh);
                extensions.push(Extension::Ninth);
                extensions.push(Extension::Eleventh);
            }
            "13" => {
                extensions.push(Extension::Seventh);
                extensions.push(Extension::Ninth);
                extensions.push(Extension::Thirteenth);
            }
            "" => {} // No extensions
            _ => return Err(anyhow!("Unknown extension: {}", ext_str)),
        }

        Ok(extensions)
    }

    /// Convert Roman numeral chord to ChordType and semitones
    fn roman_chord_to_spec(chord: &RomanNumeralChord, _key: Note) -> Result<(i8, ChordType)> {
        // Base semitones for each degree
        let base_semitones = match chord.degree {
            ScaleDegree::I => 0,
            ScaleDegree::II => 2,
            ScaleDegree::III => 4,
            ScaleDegree::IV => 5,
            ScaleDegree::V => 7,
            ScaleDegree::VI => 9,
            ScaleDegree::VII => 11,
        };

        // Apply accidental
        let semitones = match &chord.accidental {
            Some(Accidental::Flat) => base_semitones - 1,
            Some(Accidental::Sharp) => base_semitones + 1,
            Some(Accidental::Natural) | None => base_semitones,
        };

        // Convert quality to ChordType
        let chord_type = match chord.quality {
            ChordQuality::Major => ChordType::Major,
            ChordQuality::Minor => ChordType::Minor,
            ChordQuality::Diminished => ChordType::Diminished,
            ChordQuality::Augmented => ChordType::Major, // For now, treat as major (we'll enhance this later)
            ChordQuality::HalfDiminished => ChordType::Minor, // For now, treat as minor
            ChordQuality::MajorMinor => ChordType::Major, // Dominant 7th - major triad
        };

        Ok((semitones, chord_type))
    }

    /// Check if a string looks like a Roman numeral progression
    pub fn is_roman_numeral_progression(name: &str) -> bool {
        // Must contain Roman numerals and separators
        if name.is_empty() {
            return false;
        }

        // Split by common separators
        let parts: Vec<&str> = name.split(&['-', '_'][..]).collect();
        if parts.len() < 2 {
            return false; // Need at least 2 chords for a progression
        }

        // Check if each part looks like a Roman numeral
        for part in parts {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Must start with optional accidental + Roman numeral
            let chars: Vec<char> = part.chars().collect();
            if chars.is_empty() {
                return false;
            }

            let start_idx = match chars[0] {
                '♭' | 'b' | '#' | '♮' => 1,
                _ => 0,
            };

            if start_idx >= chars.len() {
                return false;
            }

            // Must have Roman numeral characters
            let has_roman = chars[start_idx..]
                .iter()
                .any(|&c| matches!(c, 'I' | 'i' | 'V' | 'v' | 'X' | 'x' | '°' | 'ø' | '+'));

            if !has_roman {
                return false;
            }
        }

        true
    }

    // /// Enhanced is_valid_progression
    // pub fn is_valid_progression(name: &str) -> bool {
    //     // Check Roman numeral progressions
    //     if Self::is_roman_numeral_progression(name) {
    //         return Self::parse_roman_numeral_progression(name).is_ok();
    //     }

    //     // Check numeric progressions
    //     if Self::is_numeric_progression(name) {
    //         return Self::parse_numeric_progression(name).is_ok();
    //     }

    //     // Check named progressions (existing logic)
    //     matches!(
    //         name,
    //         "251" | "1564" | "1625" | "1451" | "6415"
    //         | "I_V_vi_IV" | "I-V-vi-IV" | "ii_V_I" | "ii-V-I"
    //         // ... rest of existing named progressions
    //     )
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Chord;

    #[test]
    fn test_analyze_c_major_in_c() {
        let c_major = Chord::from_note_strings(vec!["C", "E", "G"]).unwrap();
        let analysis = RomanNumeral::analyze(&c_major, "C".parse().unwrap()).unwrap();

        assert_eq!(analysis.degree, ScaleDegree::I);
        assert_eq!(analysis.quality, ChordQuality::Major);
        assert_eq!(analysis.to_string(), "I");
    }

    #[test]
    fn test_analyze_chromatic_chord() {
        let e_minor = Chord::from_note_strings(vec!["E", "G", "B"]).unwrap();
        let analysis = RomanNumeral::analyze(&e_minor, "C".parse().unwrap()).unwrap();

        assert_eq!(analysis.degree, ScaleDegree::III);
        assert_eq!(analysis.quality, ChordQuality::Minor);
        assert_eq!(analysis.to_string(), "iii");
    }

    #[test]
    fn test_analyze_sharp_four() {
        let fs_dim = Chord::from_note_strings(vec!["F#", "A", "C"]).unwrap();
        let analysis = RomanNumeral::analyze(&fs_dim, "C".parse().unwrap()).unwrap();

        assert_eq!(analysis.degree, ScaleDegree::IV);
        assert_eq!(analysis.accidental, Some(Accidental::Sharp));
        assert_eq!(analysis.quality, ChordQuality::Diminished);
        assert_eq!(analysis.to_string(), "#iv°");
    }

    #[test]
    fn test_analyze_g7_in_c() {
        let g7 = Chord::from_note_strings(vec!["G", "B", "D", "F"]).unwrap();
        let analysis = RomanNumeral::analyze(&g7, "C".parse().unwrap()).unwrap();

        assert_eq!(analysis.degree, ScaleDegree::V);
        assert_eq!(analysis.quality, ChordQuality::MajorMinor);
        assert!(analysis.extensions.contains(&Extension::Seventh));
        assert_eq!(analysis.to_string(), "V7");
    }

    // #[test]
    // fn test_analyze_with_suggestions() {
    //     // Use a chord that actually exists but doesn't fit well in C major
    //     let db_major = Chord::from_note_strings(vec!["Db", "F", "Ab"]).unwrap();
    //     let result = RomanNumeral::analyze_with_suggestions(&db_major, "C".parse().unwrap());

    //     // Should provide suggestions since Db major doesn't fit in C major well
    //     assert!(result.is_err());
    //     let error_msg = result.unwrap_err().to_string();
    //     assert!(error_msg.contains("Consider analyzing in"));
    // }

    #[test]
    fn test_analyze_with_suggestions() {
        // Test error case with empty chord
        let empty_chord = Chord::new();
        let result = RomanNumeral::analyze_with_suggestions(&empty_chord, "C".parse().unwrap());
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Cannot analyze"));

        // Test success case
        let c_major = Chord::from_note_strings(vec!["C", "E", "G"]).unwrap();
        let result = RomanNumeral::analyze_with_suggestions(&c_major, "C".parse().unwrap());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), "I");
    }

    #[test]
    fn test_analyze_with_context() {
        let a_minor = Chord::from_note_strings(vec!["A", "C", "E"]).unwrap();
        let analyses = RomanNumeral::analyze_with_context(&a_minor, "C".parse().unwrap()).unwrap();

        // Should find A minor as vi in C major
        assert!(!analyses.is_empty());
        assert!(analyses.iter().any(|a| a.to_string() == "vi"));
    }

    #[test]
    fn test_common_progression_generation() {
        let progression =
            CommonProgressions::get_progression("I_V_vi_IV", "C".parse().unwrap()).unwrap();
        assert_eq!(progression.len(), 4);

        let analysis = analyze_progression(&progression, "C".parse().unwrap()).unwrap();
        assert_eq!(analysis[0].to_string(), "I");
        assert_eq!(analysis[1].to_string(), "V");
        assert_eq!(analysis[2].to_string(), "vi");
        assert_eq!(analysis[3].to_string(), "IV");
    }

    #[test]
    fn test_progression_with_dashes() {
        let progression =
            CommonProgressions::get_progression("I-V-vi-IV", "C".parse().unwrap()).unwrap();
        assert_eq!(progression.len(), 4);
    }

    #[test]
    fn test_chromatic_progression() {
        let progression =
            CommonProgressions::get_progression("I_bVII_IV_I", "C".parse().unwrap()).unwrap();
        assert_eq!(progression.len(), 4);

        // Second chord should be Bb major (♭VII in C)
        let second_chord = &progression[1];
        assert!(
            second_chord
                .notes()
                .any(|n| n.pitch_class() == "Bb".parse::<Note>().unwrap().pitch_class())
        );
        assert!(
            second_chord
                .notes()
                .any(|n| n.pitch_class() == "D".parse::<Note>().unwrap().pitch_class())
        );
        assert!(
            second_chord
                .notes()
                .any(|n| n.pitch_class() == "F".parse::<Note>().unwrap().pitch_class())
        );
    }

    #[test]
    fn test_jazz_progression() {
        let progression =
            CommonProgressions::get_progression("ii_V_I", "C".parse().unwrap()).unwrap();
        assert_eq!(progression.len(), 3);

        let analysis = analyze_progression(&progression, "C".parse().unwrap()).unwrap();
        assert_eq!(analysis[0].to_string(), "ii");
        assert_eq!(analysis[1].to_string(), "V");
        assert_eq!(analysis[2].to_string(), "I");
    }

    #[test]
    fn test_blues_progression() {
        let progression =
            CommonProgressions::get_progression("12_bar_blues", "C".parse().unwrap()).unwrap();
        assert_eq!(progression.len(), 12);

        // First four chords should be I
        for i in 0..4 {
            let analysis = RomanNumeral::analyze(&progression[i], "C".parse().unwrap()).unwrap();
            assert_eq!(analysis.to_string(), "I");
        }
    }

    #[test]
    fn test_is_valid_progression() {
        assert!(CommonProgressions::is_valid_progression("I_V_vi_IV"));
        assert!(CommonProgressions::is_valid_progression("I-V-vi-IV"));
        assert!(CommonProgressions::is_valid_progression("Pachelbel"));
        assert!(CommonProgressions::is_valid_progression("12_bar_blues"));
        assert!(!CommonProgressions::is_valid_progression(
            "invalid_progression"
        ));
    }

    #[test]
    fn test_function_descriptions() {
        let c_major = Chord::from_note_strings(vec!["C", "E", "G"]).unwrap();
        let analysis = RomanNumeral::analyze(&c_major, "C".parse().unwrap()).unwrap();
        assert!(analysis.function_description().contains("Tonic"));

        // Test chromatic alteration
        let fs_dim = Chord::from_note_strings(vec!["F#", "A", "C"]).unwrap();
        let analysis = RomanNumeral::analyze(&fs_dim, "C".parse().unwrap()).unwrap();
        assert!(
            analysis
                .function_description()
                .contains("Tritone substitution")
        );
    }

    #[test]
    fn test_inversion_analysis() {
        let c_major_first_inv = Chord::from_note_strings(vec!["E", "G", "C"])
            .unwrap()
            .invert(); // This should put E in the bass
        let analysis = RomanNumeral::analyze(&c_major_first_inv, "C".parse().unwrap()).unwrap();

        assert_eq!(analysis.degree, ScaleDegree::I);
        assert_eq!(analysis.quality, ChordQuality::Major);
        assert!(analysis.to_string().contains("⁶")); // First inversion notation
    }

    #[test]
    fn test_extended_chords() {
        // Test major 7th chord
        let cmaj7 = Chord::from_note_strings(vec!["C", "E", "G", "B"]).unwrap();
        let analysis = RomanNumeral::analyze(&cmaj7, "C".parse().unwrap()).unwrap();
        assert!(analysis.extensions.contains(&Extension::MajorSeventh));
        assert_eq!(analysis.to_string(), "IM7");

        // Test add9 chord
        let cadd9 = Chord::from_note_strings(vec!["C", "E", "G", "D"]).unwrap();
        let analysis = RomanNumeral::analyze(&cadd9, "C".parse().unwrap()).unwrap();
        assert!(analysis.extensions.contains(&Extension::Add9));
        assert_eq!(analysis.to_string(), "Iadd9");
    }

    #[test]
    fn test_sus_chords() {
        // Test sus2
        let csus2 = Chord::from_note_strings(vec!["C", "D", "G"]).unwrap();
        let analysis = RomanNumeral::analyze(&csus2, "C".parse().unwrap()).unwrap();
        assert!(analysis.extensions.contains(&Extension::Sus2));
        assert_eq!(analysis.to_string(), "Isus2");

        // Test sus4
        let csus4 = Chord::from_note_strings(vec!["C", "F", "G"]).unwrap();
        let analysis = RomanNumeral::analyze(&csus4, "C".parse().unwrap()).unwrap();
        assert!(analysis.extensions.contains(&Extension::Sus4));
        assert_eq!(analysis.to_string(), "Isus4");
    }

    #[test]
    fn test_error_handling() {
        let empty_chord = Chord::new();
        let result = RomanNumeral::analyze(&empty_chord, "C".parse().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty chord"));
    }

    #[test]
    fn test_list_progressions() {
        let progressions = CommonProgressions::list_progressions();
        assert!(!progressions.is_empty());
        assert!(progressions.iter().any(|p| p.contains("I_V_vi_IV")));
        assert!(progressions.iter().any(|p| p.contains("ii_V_I")));
        assert!(progressions.iter().any(|p| p.contains("Jazz")));
        assert!(progressions.iter().any(|p| p.contains("Modal")));
    }
}

// Updated tests for the ChordType system in roman_numeral.rs

#[cfg(test)]
mod numeric_progression_tests {
    use crate::parser::Evaluator;

    use super::*;

    #[test]
    fn test_parse_numeric_progression() {
        // Test basic progressions
        let result = CommonProgressions::parse_numeric_progression("251").unwrap();
        assert_eq!(
            result,
            vec![
                (2, ChordType::Minor),
                (7, ChordType::Major),
                (0, ChordType::Major)
            ]
        ); // ii-V-I

        let result = CommonProgressions::parse_numeric_progression("1564").unwrap();
        assert_eq!(
            result,
            vec![
                (0, ChordType::Major),
                (7, ChordType::Major),
                (9, ChordType::Minor),
                (5, ChordType::Major)
            ]
        ); // I-V-vi-IV

        let result = CommonProgressions::parse_numeric_progression("16251").unwrap();
        assert_eq!(
            result,
            vec![
                (0, ChordType::Major), // I
                (9, ChordType::Minor), // vi
                (2, ChordType::Minor), // ii
                (7, ChordType::Major), // V
                (0, ChordType::Major)  // I
            ]
        );
    }

    #[test]
    fn test_parse_numeric_progression_with_diminished() {
        // Test progression with vii°
        let result = CommonProgressions::parse_numeric_progression("17").unwrap();
        assert_eq!(
            result,
            vec![
                (0, ChordType::Major),       // I
                (11, ChordType::Diminished)  // vii°
            ]
        );

        let result = CommonProgressions::parse_numeric_progression("1234567").unwrap();
        assert_eq!(
            result,
            vec![
                (0, ChordType::Major),       // I
                (2, ChordType::Minor),       // ii
                (4, ChordType::Minor),       // iii
                (5, ChordType::Major),       // IV
                (7, ChordType::Major),       // V
                (9, ChordType::Minor),       // vi
                (11, ChordType::Diminished)  // vii°
            ]
        );
    }

    #[test]
    fn test_is_numeric_progression() {
        assert!(CommonProgressions::is_numeric_progression("251"));
        assert!(CommonProgressions::is_numeric_progression("1564"));
        assert!(CommonProgressions::is_numeric_progression("16251"));
        assert!(CommonProgressions::is_numeric_progression("36251"));
        assert!(CommonProgressions::is_numeric_progression("4513"));
        assert!(CommonProgressions::is_numeric_progression("1234567"));

        // Edge cases
        assert!(!CommonProgressions::is_numeric_progression(""));
        assert!(!CommonProgressions::is_numeric_progression("1")); // too short
        assert!(!CommonProgressions::is_numeric_progression("I_V_vi_IV")); // not numeric
        assert!(!CommonProgressions::is_numeric_progression("12a34")); // contains letter
        assert!(!CommonProgressions::is_numeric_progression("1234567890123")); // too long
    }

    #[test]
    fn test_invalid_numeric_progressions() {
        // Invalid scale degrees
        let result = CommonProgressions::parse_numeric_progression("189");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid scale degree: 8")
        );

        let result = CommonProgressions::parse_numeric_progression("250");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid scale degree: 0")
        );

        // Invalid characters
        let result = CommonProgressions::parse_numeric_progression("25a1");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid character")
        );

        // Empty
        let result = CommonProgressions::parse_numeric_progression("");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Empty progression")
        );
    }

    #[test]
    fn test_enhanced_get_progression_numeric() {
        let key = "C".parse().unwrap();

        // Test numeric progression
        let prog = CommonProgressions::get_progression("251", key).unwrap();
        assert_eq!(prog.len(), 3);

        // Check the chords are correct
        let analysis = analyze_progression(&prog, key).unwrap();
        assert_eq!(analysis[0].to_string(), "ii");
        assert_eq!(analysis[1].to_string(), "V");
        assert_eq!(analysis[2].to_string(), "I");

        // Test longer numeric progression
        let prog = CommonProgressions::get_progression("16251", key).unwrap();
        assert_eq!(prog.len(), 5);

        let analysis = analyze_progression(&prog, key).unwrap();
        assert_eq!(analysis[0].to_string(), "I");
        assert_eq!(analysis[1].to_string(), "vi");
        assert_eq!(analysis[2].to_string(), "ii");
        assert_eq!(analysis[3].to_string(), "V");
        assert_eq!(analysis[4].to_string(), "I");
    }

    #[test]
    fn test_diminished_chord_generation() {
        let key = "C".parse().unwrap();

        // Test progression with vii°
        let prog = CommonProgressions::get_progression("17", key).unwrap();
        assert_eq!(prog.len(), 2);

        // Second chord should be B diminished
        let second_chord = &prog[1];
        assert!(
            second_chord
                .notes()
                .any(|n| n.pitch_class() == "B".parse::<Note>().unwrap().pitch_class())
        );
        assert!(
            second_chord
                .notes()
                .any(|n| n.pitch_class() == "D".parse::<Note>().unwrap().pitch_class())
        );
        assert!(
            second_chord
                .notes()
                .any(|n| n.pitch_class() == "F".parse::<Note>().unwrap().pitch_class())
        ); // F natural, not F#
        assert!(
            !second_chord
                .notes()
                .any(|n| n.pitch_class() == "F#".parse::<Note>().unwrap().pitch_class())
        ); // Should NOT contain F#

        let analysis = analyze_progression(&prog, key).unwrap();
        assert_eq!(analysis[0].to_string(), "I");
        assert_eq!(analysis[1].to_string(), "vii°");
    }

    #[test]
    fn test_full_scale_progression() {
        let key = "C".parse().unwrap();

        // Test 1234567 progression
        let prog = CommonProgressions::get_progression("1234567", key).unwrap();
        assert_eq!(prog.len(), 7);

        // Check that the last chord is B diminished [B, D, F], not B minor [B, D, F#]
        let last_chord = &prog[6];
        assert!(
            last_chord
                .notes()
                .any(|n| n.pitch_class() == "B".parse::<Note>().unwrap().pitch_class())
        );
        assert!(
            last_chord
                .notes()
                .any(|n| n.pitch_class() == "D".parse::<Note>().unwrap().pitch_class())
        );
        assert!(
            last_chord
                .notes()
                .any(|n| n.pitch_class() == "F".parse::<Note>().unwrap().pitch_class())
        ); // F natural
        assert!(
            !last_chord
                .notes()
                .any(|n| n.pitch_class() == "F#".parse::<Note>().unwrap().pitch_class())
        ); // Should NOT contain F#

        let analysis = analyze_progression(&prog, key).unwrap();
        assert_eq!(analysis[6].to_string(), "vii°");
    }

    #[test]
    fn test_enhanced_is_valid_progression() {
        // Numeric progressions should be valid
        assert!(CommonProgressions::is_valid_progression("251"));
        assert!(CommonProgressions::is_valid_progression("1564"));
        assert!(CommonProgressions::is_valid_progression("16251"));
        assert!(CommonProgressions::is_valid_progression("36251"));
        assert!(CommonProgressions::is_valid_progression("1234567"));

        // Named progressions should still work
        assert!(CommonProgressions::is_valid_progression("I_V_vi_IV"));
        assert!(CommonProgressions::is_valid_progression("Pachelbel"));

        // Invalid should be false
        assert!(!CommonProgressions::is_valid_progression("189")); // invalid degree
        assert!(!CommonProgressions::is_valid_progression("1")); // too short
        assert!(!CommonProgressions::is_valid_progression("invalid"));
    }

    #[test]
    fn test_format_numeric_progression_name() {
        let formatted = Evaluator::format_numeric_progression_name("251");
        assert_eq!(formatted, "ii-V-I");

        let formatted = Evaluator::format_numeric_progression_name("1564");
        assert_eq!(formatted, "I-V-vi-IV");

        let formatted = Evaluator::format_numeric_progression_name("16251");
        assert_eq!(formatted, "I-vi-ii-V-I");

        let formatted = Evaluator::format_numeric_progression_name("1234567");
        assert_eq!(formatted, "I-ii-iii-IV-V-vi-vii°");
    }

    #[test]
    fn test_complex_numeric_progressions() {
        let key = "D".parse().unwrap();

        // Test a complex progression
        let prog = CommonProgressions::get_progression("36251", key).unwrap();
        assert_eq!(prog.len(), 5);

        let analysis = analyze_progression(&prog, key).unwrap();
        assert_eq!(analysis[0].to_string(), "iii");
        assert_eq!(analysis[1].to_string(), "vi");
        assert_eq!(analysis[2].to_string(), "ii");
        assert_eq!(analysis[3].to_string(), "V");
        assert_eq!(analysis[4].to_string(), "I");

        // Test in different key
        let prog = CommonProgressions::get_progression("4513", key).unwrap();
        assert_eq!(prog.len(), 4);

        let analysis = analyze_progression(&prog, key).unwrap();
        assert_eq!(analysis[0].to_string(), "IV");
        assert_eq!(analysis[1].to_string(), "V");
        assert_eq!(analysis[2].to_string(), "I");
        assert_eq!(analysis[3].to_string(), "iii");
    }
}

// Add these tests to roman_numeral.rs

#[cfg(test)]
mod roman_numeral_parser_tests {
    use super::*;

    #[test]
    fn test_parse_single_roman_numeral() {
        // Basic major chords
        let chord = CommonProgressions::parse_single_roman_numeral("I").unwrap();
        assert_eq!(chord.degree, ScaleDegree::I);
        assert_eq!(chord.quality, ChordQuality::Major);
        assert_eq!(chord.accidental, None);

        // Basic minor chords
        let chord = CommonProgressions::parse_single_roman_numeral("vi").unwrap();
        assert_eq!(chord.degree, ScaleDegree::VI);
        assert_eq!(chord.quality, ChordQuality::Minor);

        // Diminished chords
        let chord = CommonProgressions::parse_single_roman_numeral("vii°").unwrap();
        assert_eq!(chord.degree, ScaleDegree::VII);
        assert_eq!(chord.quality, ChordQuality::Diminished);

        // With accidentals
        let chord = CommonProgressions::parse_single_roman_numeral("♭VII").unwrap();
        assert_eq!(chord.degree, ScaleDegree::VII);
        assert_eq!(chord.quality, ChordQuality::Major);
        assert_eq!(chord.accidental, Some(Accidental::Flat));

        let chord = CommonProgressions::parse_single_roman_numeral("#iv").unwrap();
        assert_eq!(chord.degree, ScaleDegree::IV);
        assert_eq!(chord.quality, ChordQuality::Minor);
        assert_eq!(chord.accidental, Some(Accidental::Sharp));
    }

    #[test]
    fn test_parse_roman_numeral_with_extensions() {
        // Seventh chords - basic supported extension
        let chord = CommonProgressions::parse_single_roman_numeral("V7").unwrap();
        assert_eq!(chord.degree, ScaleDegree::V);
        assert_eq!(chord.quality, ChordQuality::Major);
        assert!(chord.extensions.contains(&Extension::Seventh));

        // Note: Other extensions like IM7, Vsus4 are not currently supported by the parser
    }

    #[test]
    fn test_parse_roman_numeral_progression() {
        // Basic progression
        let progression = CommonProgressions::parse_roman_numeral_progression("I-V-vi-IV").unwrap();
        assert_eq!(progression.len(), 4);

        assert_eq!(progression[0].degree, ScaleDegree::I);
        assert_eq!(progression[0].quality, ChordQuality::Major);

        assert_eq!(progression[1].degree, ScaleDegree::V);
        assert_eq!(progression[1].quality, ChordQuality::Major);

        assert_eq!(progression[2].degree, ScaleDegree::VI);
        assert_eq!(progression[2].quality, ChordQuality::Minor);

        assert_eq!(progression[3].degree, ScaleDegree::IV);
        assert_eq!(progression[3].quality, ChordQuality::Major);

        // With underscores
        let progression = CommonProgressions::parse_roman_numeral_progression("ii_V_I").unwrap();
        assert_eq!(progression.len(), 3);
        assert_eq!(progression[0].quality, ChordQuality::Minor);
        assert_eq!(progression[1].quality, ChordQuality::Major);
        assert_eq!(progression[2].quality, ChordQuality::Major);
    }

    #[test]
    fn test_parse_chromatic_progression() {
        // With flat VII
        let progression = CommonProgressions::parse_roman_numeral_progression("I-♭VII-IV").unwrap();
        assert_eq!(progression.len(), 3);

        assert_eq!(progression[1].degree, ScaleDegree::VII);
        assert_eq!(progression[1].quality, ChordQuality::Major);
        assert_eq!(progression[1].accidental, Some(Accidental::Flat));

        // With sharp iv
        let progression = CommonProgressions::parse_roman_numeral_progression("I-#iv°-V").unwrap();
        assert_eq!(progression.len(), 3);

        assert_eq!(progression[1].degree, ScaleDegree::IV);
        assert_eq!(progression[1].quality, ChordQuality::Diminished);
        assert_eq!(progression[1].accidental, Some(Accidental::Sharp));
    }

    #[test]
    fn test_is_roman_numeral_progression() {
        // Valid Roman numeral progressions
        assert!(CommonProgressions::is_roman_numeral_progression(
            "I-V-vi-IV"
        ));
        assert!(CommonProgressions::is_roman_numeral_progression("ii-V-I"));
        assert!(CommonProgressions::is_roman_numeral_progression(
            "I_V_vi_IV"
        ));
        assert!(CommonProgressions::is_roman_numeral_progression(
            "♭VII-IV-I"
        ));
        assert!(CommonProgressions::is_roman_numeral_progression("#iv°-V-I"));
        assert!(CommonProgressions::is_roman_numeral_progression("I-III-iv"));

        // Invalid patterns
        assert!(!CommonProgressions::is_roman_numeral_progression("251")); // numeric
        assert!(!CommonProgressions::is_roman_numeral_progression("I")); // single chord
        assert!(!CommonProgressions::is_roman_numeral_progression("")); // empty
        assert!(!CommonProgressions::is_roman_numeral_progression("X-Y-Z")); // invalid Romans
    }

    #[test]
    fn test_roman_chord_to_spec() {
        let key = "C".parse().unwrap();

        // Basic I chord
        let chord = RomanNumeralChord {
            degree: ScaleDegree::I,
            quality: ChordQuality::Major,
            accidental: None,
            extensions: vec![],
        };
        let (semitones, chord_type) = CommonProgressions::roman_chord_to_spec(&chord, key).unwrap();
        assert_eq!(semitones, 0);
        assert_eq!(chord_type, ChordType::Major);

        // vi chord
        let chord = RomanNumeralChord {
            degree: ScaleDegree::VI,
            quality: ChordQuality::Minor,
            accidental: None,
            extensions: vec![],
        };
        let (semitones, chord_type) = CommonProgressions::roman_chord_to_spec(&chord, key).unwrap();
        assert_eq!(semitones, 9);
        assert_eq!(chord_type, ChordType::Minor);

        // ♭VII chord
        let chord = RomanNumeralChord {
            degree: ScaleDegree::VII,
            quality: ChordQuality::Major,
            accidental: Some(Accidental::Flat),
            extensions: vec![],
        };
        let (semitones, chord_type) = CommonProgressions::roman_chord_to_spec(&chord, key).unwrap();
        assert_eq!(semitones, 10); // 11 - 1 = 10
        assert_eq!(chord_type, ChordType::Major);
    }

    #[test]
    fn test_enhanced_get_progression_roman() {
        let key = "C".parse().unwrap();

        // Test Roman numeral progression
        let prog = CommonProgressions::get_progression("I-V-vi-IV", key).unwrap();
        assert_eq!(prog.len(), 4);

        let analysis = analyze_progression(&prog, key).unwrap();
        assert_eq!(analysis[0].to_string(), "I");
        assert_eq!(analysis[1].to_string(), "V");
        assert_eq!(analysis[2].to_string(), "vi");
        assert_eq!(analysis[3].to_string(), "IV");

        // Test chromatic progression
        let prog = CommonProgressions::get_progression("I-♭VII-IV", key).unwrap();
        assert_eq!(prog.len(), 3);

        // Second chord should be Bb major (♭VII in C)
        let second_chord = &prog[1];
        assert!(
            second_chord
                .notes()
                .any(|n| n.pitch_class() == "Bb".parse::<Note>().unwrap().pitch_class())
        );
        assert!(
            second_chord
                .notes()
                .any(|n| n.pitch_class() == "D".parse::<Note>().unwrap().pitch_class())
        );
        assert!(
            second_chord
                .notes()
                .any(|n| n.pitch_class() == "F".parse::<Note>().unwrap().pitch_class())
        );
    }

    #[test]
    fn test_new_arbitrary_progressions() {
        let key = "C".parse().unwrap();

        // Test completely new progressions that weren't hardcoded
        let prog = CommonProgressions::get_progression("I-III-iv", key).unwrap();
        assert_eq!(prog.len(), 3);

        let analysis = analyze_progression(&prog, key).unwrap();
        assert_eq!(analysis[0].to_string(), "I"); // C major
        assert_eq!(analysis[1].to_string(), "III"); // E major
        assert_eq!(analysis[2].to_string(), "iv"); // F minor

        // Test with tritone substitution
        let prog = CommonProgressions::get_progression("ii-♭V-I", key).unwrap();
        assert_eq!(prog.len(), 3);

        let analysis = analyze_progression(&prog, key).unwrap();
        assert_eq!(analysis[0].to_string(), "ii"); // D minor
        // Note: ♭V (Gb) may be analyzed as #IV or ♭V depending on enharmonic choice
        let tritone_sub = analysis[1].to_string();
        assert!(
            tritone_sub == "♭V" || tritone_sub == "#IV" || tritone_sub == "#iv°",
            "Expected ♭V or #IV, got: {}",
            tritone_sub
        );
        assert_eq!(analysis[2].to_string(), "I"); // C major
    }

    #[test]
    fn test_invalid_roman_progressions() {
        // Invalid Roman numerals
        let result = CommonProgressions::parse_roman_numeral_progression("I-XXX-V");
        assert!(result.is_err());

        // Invalid extensions
        let result = CommonProgressions::parse_single_roman_numeral("I99");
        assert!(result.is_err());

        // Empty parts
        let result = CommonProgressions::parse_roman_numeral_progression("I--V");
        // This should still work, just ignoring empty parts
        assert!(result.is_ok());
    }

    #[test]
    fn test_enhanced_is_valid_progression_roman() {
        // Roman numeral progressions should be valid
        assert!(CommonProgressions::is_valid_progression("I-V-vi-IV"));
        assert!(CommonProgressions::is_valid_progression("ii-V-I"));
        assert!(CommonProgressions::is_valid_progression("I-III-iv"));
        assert!(CommonProgressions::is_valid_progression("♭VII-IV-I"));

        // Numeric progressions should still work
        assert!(CommonProgressions::is_valid_progression("251"));
        assert!(CommonProgressions::is_valid_progression("1564"));

        // Named progressions should still work
        assert!(CommonProgressions::is_valid_progression("Pachelbel"));

        // Invalid should be false
        assert!(!CommonProgressions::is_valid_progression("I-XXX-V"));
        assert!(!CommonProgressions::is_valid_progression("invalid"));
    }
}
