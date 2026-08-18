// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: fractional-numeric-destination
fn main() {
    let ratio: f64 = 4.2;
    let count: terrane_int_support::Int = terrane_int_support::unwrap_or_fail(terrane_int_support::exact_int_f64(ratio));
    println!("{}", terrane_scalar_support::scalar_text(&(count)));
}
