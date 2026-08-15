// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: checked-coercion
fn main() {
    let value: terrane_int_support::Int = terrane_int_support::Int::from(300_i128);
    let within: terrane_int_support::Int = terrane_int_support::Int::from(100_i128);
    let coerced: i8 = terrane_int_support::unwrap_or_fail(terrane_int_support::coerce::<i8>(&(within)));
    let renamed_checked: Option<i8> = terrane_int_support::checked_coerce::<i8>(&(within));
    let checked: Option<i8> = terrane_int_support::checked_coerce::<i8>(&(value));
    let absent: bool = (checked).is_none();
    let present: bool = (checked).is_some();
    println!("{}", terrane_scalar_support::scalar_text(&(coerced)));
    println!("{}", terrane_scalar_support::scalar_text(&((renamed_checked).is_some())));
    println!("{}", terrane_scalar_support::scalar_text(&(absent)));
    println!("{}", terrane_scalar_support::scalar_text(&(present)));
}
