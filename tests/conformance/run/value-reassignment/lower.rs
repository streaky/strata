// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: value-reassignment
fn main() {
    let mut value: terrane_int_support::Int = terrane_int_support::Int::from(1_i128);
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
    let next: terrane_int_support::Int = terrane_int_support::Int::from(2_i128);
    value = next.clone();
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
    value = terrane_int_support::Int::from(3_i128);
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
}
