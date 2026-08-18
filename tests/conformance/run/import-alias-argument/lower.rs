// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: later-coercion-alias-argument
fn main() {
    let value: terrane_int_support::Int = terrane_int_support::Int::from(100_i128);
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(terrane_int_support::coerce::<i8>(&(value))))));
}
