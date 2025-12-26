cadence> let chords = "[G5,C5,E5] [Bb4,D5,F5]".slow(2);
cadence> let chords = "[G5,C5,E5] [[Bb4,D5,F5] _ _ [F4,A4,C5]]".slow(2);
cadence> Error: slow() expects (pattern, factor_note)
cadence> let chords = "[G5,C5,E5] [[Bb4,D5,F5] [F4,A4,C5]]".slow(2);
Error: slow() expects (pattern, factor_note)
cadence> let x = "[G5,C5,E5] [[Bb4,D5,F5] [F4,A4,C5]]"
cadence> x
"[G5,C5,E5] [[Bb4,D5,F5] [F4,A4,C5]]"
cadence> on 6 play x
cadence> Playing "[G5,C5,E5] [[Bb4,D5,F5] [F4,A4,C5]]" (Track 6)
🔊 Playing "[G5,C5,E5] [[Bb4,D5,F5] [F4,A4,C5]]" (Track 6) - live reactive!
Failed to evaluate playback source: Cannot play a string "[G5,C5,E5] [[Bb4,D5,F5] [F4,A4,C5]]"
cadence> let chords = "[G5,C5,E5] [Bb4,D5,F5] [F4,A4,C5]".slow(2);