// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: value-reassignment
fn main() {
    let mut value: terrane_int_support::Int = terrane_int_support::Int::from(1_i128);
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
    let next: i64 = 2;
    value = terrane_int_support::Int::from((next) as i128);
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
    value = terrane_int_support::Int::from(3_i128);
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
}
