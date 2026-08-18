// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: lexical-replacement
fn main() {
    let value: i8 = 1;
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
    let _ = &value;
    let value: terrane_int_support::Int = terrane_int_support::Int::from(2_i128);
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
}
