// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: descriptor-constructs
fn main() {
    let value: terrane_int_support::Int = terrane_int_support::Int::from(1_i128);
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = value.clone(); true })));
    println!("{}", terrane_scalar_support::scalar_text(&({  true })));
    println!("{}", terrane_scalar_support::scalar_text(&({  true })));
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = value; true })));
}
