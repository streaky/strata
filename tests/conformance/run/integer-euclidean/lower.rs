// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: integer-euclidean
fn main() {
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail((-terrane_int_support::Int::from(7_i128)).euclidean_div(&(terrane_int_support::Int::from(3_i128)))))));
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail((terrane_int_support::Int::from(7_i128)).euclidean_div(&(-terrane_int_support::Int::from(3_i128)))))));
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail((-terrane_int_support::Int::from(7_i128)).modulo(&(terrane_int_support::Int::from(3_i128)))))));
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail((terrane_int_support::Int::from(7_i128)).modulo(&(-terrane_int_support::Int::from(3_i128)))))));
}
