// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: integer-coercion-failure
fn main() {
    let value: terrane_int_support::Int = terrane_int_support::Int::from(128_i128);
    terrane_int_support::unwrap_or_fail(terrane_int_support::coerce::<i8>(&(value)));
}
