// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: mixed-width-arithmetic
fn main() {
    let left: i8 = 100;
    let right: i32 = 2;
    let unsigned: u8 = 120;
    let total: i32 = terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_addition((left) as i32, right));
    let combined: i16 = terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_addition((left) as i16, (unsigned) as i16));
    println!("{}", terrane_scalar_support::scalar_text(&(total)));
    println!("{}", terrane_scalar_support::scalar_text(&(combined)));
}
