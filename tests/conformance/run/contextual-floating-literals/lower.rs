// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: contextual-floating-literals
fn main() {
    let hexadecimal: f32 = 255.0_f32;
    let hexadecimal64: f64 = 5.0_f64;
    let whole: f64 = 9007199254740992.0_f64;
    println!("{}", terrane_scalar_support::scalar_text(&(hexadecimal)));
    println!("{}", terrane_scalar_support::scalar_text(&(hexadecimal64)));
    println!("{}", terrane_scalar_support::scalar_text(&(whole)));
}
