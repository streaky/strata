// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: contextual-constant-arithmetic
fn third() -> f32 {
    return 0.33333334_f32;
}
fn bounded() -> i32 {
    return 1;
}
fn main() {
    let reduced: i8 = 100;
    let integral: i64 = 4;
    let quotient: i64 = 0;
    let ratio: f32 = 0.33333334_f32;
    let rounded: terrane_int_support::Int = terrane_int_support::unwrap_or_fail(terrane_int_support::rounded_f32(ratio, terrane_int_support::FloatRounding::TiesEven));
    println!("{}", terrane_scalar_support::scalar_text(&(reduced)));
    println!("{}", terrane_scalar_support::scalar_text(&(integral)));
    println!("{}", terrane_scalar_support::scalar_text(&(quotient)));
    println!("{}", terrane_scalar_support::scalar_text(&(ratio)));
    println!("{}", terrane_scalar_support::scalar_text(&(rounded)));
    println!("{}", terrane_scalar_support::scalar_text(&(true)));
    println!("{}", terrane_scalar_support::scalar_text(&(false)));
    println!("{}", terrane_scalar_support::scalar_text(&(bounded())));
    println!("{}", terrane_scalar_support::scalar_text(&(third())));
}
