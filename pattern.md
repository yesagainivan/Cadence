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