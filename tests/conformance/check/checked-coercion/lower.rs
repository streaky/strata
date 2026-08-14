// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: checked-coercion
fn main() {
    let value: terrane_int_support::Int = terrane_int_support::Int::from(300_i128);
    let checked: Option<i8> = terrane_int_support::checked_coerce::<i8>(&(value));
    let absent: bool = (checked).is_none();
    println!("{}", terrane_scalar_support::scalar_text(&(absent)));
}
