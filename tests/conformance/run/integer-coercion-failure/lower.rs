// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: integer-coercion-failure
fn main() {
    let value: i64 = 128;
    let narrow: i8 = terrane_int_support::unwrap_or_fail(terrane_int_support::coerce::<i8>(&(value)));
    println!("{}", terrane_scalar_support::scalar_text(&(narrow)));
}
