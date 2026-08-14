// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: integer-coercions
fn main() {
    let value: terrane_int_support::Int = terrane_int_support::Int::from(300_i128);
    let exact: i8 = terrane_int_support::unwrap_or_fail(terrane_int_support::coerce::<i8>(&(120)));
    let wrapped: u8 = terrane_int_support::wrapping_coerce::<u8>(&((value.clone() + terrane_int_support::Int::from(0_i128))));
    let saturated: u8 = terrane_int_support::saturating_coerce::<u8>(&(value));
    println!("{}", terrane_scalar_support::scalar_text(&(exact)));
    println!("{}", terrane_scalar_support::scalar_text(&(wrapped)));
    println!("{}", terrane_scalar_support::scalar_text(&(saturated)));
}
