// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: lexical-replacement
fn main() {
    let value: terrane_int_support::Int = terrane_int_support::Int::from(1_i128);
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
    let _ = &value;
    let value: String = String::from("two");
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
}
