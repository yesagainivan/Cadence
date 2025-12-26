cadence> audio play progression V_V_V_V_V_ii_bVII_IV_IV_I_I_I_I_I_I_bIII_bVII(C) loop queue
Generated V_V_V_V_V_ii_bVII_IV_IV_I_I_I_I_I_I_bIII_bVII progression in C
🔁 Queued looping progression for next beat... (use 'audio stop' to stop)
cadence> audio stop
🔇 Audio playback stopped.
cadence> audio play progression V_V_V_V_V_ii_bVII_IV_IV_I_I_I_I_I_I_bIII_bVII(C) loop queue
Generated V_V_V_V_V_ii_bVII_IV_IV_I_I_I_I_I_I_bIII_bVII progression in C
🔁 Queued looping progression for next beat... (use 'audio stop' to stop)
cadence> audio stop
🔇 Audio playback stopped.
cadence> 



let big = V_V_V_V_V_ii_bVII_IV_IV_I_I_I_I_I_I_bIII_bVII(C);
let small = V_ii_bVII_IV_I_bIII_bVII(C)
let tip = [[C6], [C2], [F7], [C6]];
let bottom = [[C2], [F7], [C6], [C2]];
let middle = [[F7], [C6], [C2], [F7]];

play smooth_voice_leading(big)

let down = "C E G".fast(2).rev();



// Define a function
fn jazz_comp(key) {
    return ii_V_I(key).wave("saw").env("pluck")
}
// Use it
play jazz_comp(C) loop
play jazz_comp(G) loop


play "C E G".wave("saw") loop
track 1 { play "C E G".wave("square") loop }


//


cadence> bass = "D D D F"
cadence> play bass
Playing "D D D F" (Track 1)
🔊 Playing "D D D F" (Track 1) - live reactive!
cadence> play bass queue cycle loop
Playing "D D D F" (looping, Track 1)
🔁 Queued "D D D F" for next cycle... (Track 1)
cadence> bass = "D A3 D F"
cadence> bass = "D A2 D F"
cadence> bass = "D3 A2 D F"
cadence> bass = "D3 A2 D3 F"
cadence> melody = "A4";
cadence> on 2 play melody queue cycle loop
Playing "A" (looping, Track 2)
cadence> 🔁 Queued "A" for next cycle... (Track 2)
cadence> melody = "A4 G4 D4 E4";
cadence> on 2 play melody queue cycle loop
Playing "A G D E" (looping, Track 2)
cadence> 🔁 Queued "A G D E" for next cycle... (Track 2)
cadence> melody = "A3 G4 D4 E4";
cadence> melody = "A3 D4 D4 E4";
cadence> melody = "A3 D4 G4 E4";
cadence> melody = "A3 D4 G3 E4";
cadence> melody = "A3 D4 G3 F4";
cadence> melody = "A3 D4 G3 F3";
cadence> let con = "F3 _".fast(2);
cadence> on 3 play con queue bar loop
cadence> Playing "F3 _" (looping, Track 3)
🔁 Queued "F3 _" for next bar... (Track 3)
cadence> on 4 play con.fast(2) queue bar loop
Playing "F3 _" (looping, Track 4)
cadence> 🔁 Queued "F3 _" for next bar... (Track 4)
cadence> let coni = con + 12
cadence> coni
"F _"
cadence> tracks
🎛️  Active Tracks (5/16):
  Track 1: ▶ playing
  Track 2: ▶ playing
  Track 3: ▶ playing
  Track 4: ▶ playing
  Track 5: ⏹ stopped

cadence> let arp = "D5 E5 F5";
cadence> on 5 play every(2, "rev", arp) queue bar loop
cadence> Playing "F5 E5 D5" (looping, Track 5)
🔁 Queued "F5 E5 D5" for next bar... (Track 5)
cadence> on 5 play every(2, "rev", arp).fast(2) queue bar loop
Playing "F5 E5 D5" (looping, Track 5)
cadence> 🔁 Queued "F5 E5 D5" for next bar... (Track 5)
cadence> on 5 play every(2, "rev", arp).fast(3) queue bar loop
Playing "D5 E5 F5" (looping, Track 5)
🔁 Queued "D5 E5 F5" for next bar... (Track 5)
cadence> on 5 play every(2, "rev", arp).fast(4) queue bar loop
Playing "D5 E5 F5" (looping, Track 5)
🔁 Queued "D5 E5 F5" for next bar... (Track 5)
cadence> on 5 play every(2, "rev", arp).fast(2) queue bar loop
Playing "D5 E5 F5" (looping, Track 5)
🔁 Queued "D5 E5 F5" for next bar... (Track 5)
cadence> let arp = "D5 E5 F5 A5";
cadence> on 5 play every(2, "rev", arp).fast(2) queue bar loop
cadence> Playing "A5 F5 E5 D5" (looping, Track 5)
🔁 Queued "A5 F5 E5 D5" for next bar... (Track 5)
track 5 volume 20
cadence> Volume set to 20% (Track 5)
cadence> let chords = [[D,F,A] [A,C#,E] _ _ ]
Parse error: at line 1, column 22: Expected RightDoubleBracket, found LeftBracket
cadence> let chords = [[D,F,A] [A,C#,E] _ _]
Parse error: at line 1, column 22: Expected RightDoubleBracket, found LeftBracket
cadence> let chords = "[D,F,A] [A,C#,E] _ _"
cadence> let chords = "[D,F,A] [A,C#,E] [D,F,A] _"
cadence> tracks
🎛️  Active Tracks (5/16):
  Track 1: ▶ playing
  Track 2: ▶ playing
  Track 3: ▶ playing
  Track 4: ▶ playing
  Track 5: ▶ playing

cadence> on 6 play chords queue bar loop
cadence> Playing "D minor: [D, F, A] A Major: [A, C#5, E5] D minor: [D, F, A] _" (looping, Track 6)
🔁 Queued "D minor: [D, F, A] A Major: [A, C#5, E5] D minor: [D, F, A] _" for next bar... (Track 6)
cadence> let chords = "[D,F,A] [A,C#,E] [D,F,A,D5] _"
cadence> let chords = "[D,F,A] [A,C#,E] [D,F,A,D5]*2"
cadence> let chords = "[D,F,A] [A,C#,E] [D,F,A,D5]*2 _"
cadence> on 6 play chords queue bar loop
cadence> Playing "D minor: [D, F, A] A Major: [A, C#5, E5] [D, F, A, D5]*2 _" (looping, Track 6)
🔁 Queued "D minor: [D, F, A] A Major: [A, C#5, E5] [D, F, A, D5]*2 _" for next bar... (Track 6)
cadence> on 6 play chords queue bar loop
cadence> Playing "D minor: [D, F, A] A Major: [A, C#5, E5] [D, F, A, D5]*2 _" (looping, Track 6)
🔁 Queued "D minor: [D, F, A] A Major: [A, C#5, E5] [D, F, A, D5]*2 _" for next bar... (Track 6)
cadence> let chords = "[D,F,A] [A,C#,E] [D,F,A,D5] _"
cadence> let chords = "[D,F,A] [A,C#,E] [D,F,A,D5] _"
cadence> bass = "D3*2 A2 D3 F"
cadence> bass = "D3*2 A2 D3 F"
cadence> bass = "D3*2 A2 D3 F"
cadence> bass = "D3*3 A2 D3 F"
cadence> 