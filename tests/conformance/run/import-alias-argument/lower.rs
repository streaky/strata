// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: later-coercion-alias-argument
fn main() {
    let value: i64 = 100;
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(terrane_int_support::coerce::<i8>(&(value))))));
}
