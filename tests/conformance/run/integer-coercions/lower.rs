// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: integer-coercions
fn main() {
    let value: i128 = 300;
    let exact: i8 = terrane_int_support::coerce::<i8>(&(120)).unwrap_or_else(|failure| panic!("{}", failure.render()));
    let wrapped: u8 = terrane_int_support::wrapping_coerce::<u8>(&(value));
    let saturated: u8 = terrane_int_support::saturating_coerce::<u8>(&(value));
    println!("{}", terrane_scalar_support::scalar_text(&(exact)));
    println!("{}", terrane_scalar_support::scalar_text(&(wrapped)));
    println!("{}", terrane_scalar_support::scalar_text(&(saturated)));
}
