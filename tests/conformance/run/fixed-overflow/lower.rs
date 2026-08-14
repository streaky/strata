// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: fixed-overflow
fn main() {
    let left: i8 = 120;
    let right: i8 = 10;
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_addition(left, right)))));
}
