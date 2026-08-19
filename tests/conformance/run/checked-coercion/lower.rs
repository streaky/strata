// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: checked-coercion
static __TERRANE_F0_SHARED: std::sync::LazyLock<terrane_int_support::Int> = std::sync::LazyLock::new(|| terrane_int_support::Int::from(100_i128));
fn convert(item: terrane_int_support::Int) -> i8 {
    let mut result: i8 = 0;
    if item.clone() > terrane_int_support::Int::from(0_i128) {
        result = terrane_int_support::unwrap_or_fail(terrane_int_support::coerce::<i8>(&(item)));
    }
    return result;
}
fn helper() {
    println!("{}", terrane_scalar_support::scalar_text(&(String::from("helper"))));
}
fn main() {
    helper();
    let value: i64 = 300;
    let within: i64 = 100;
    let coerced: i8 = terrane_int_support::unwrap_or_fail(terrane_int_support::coerce::<i8>(&(within)));
    let renamed_checked: Option<i8> = terrane_int_support::checked_coerce::<i8>(&(within));
    let shared_coerced: i8 = terrane_int_support::unwrap_or_fail(terrane_int_support::coerce::<i8>(&*__TERRANE_F0_SHARED));
    let parameter_coerced: i8 = convert(terrane_int_support::Int::from((within) as i128));
    let checked: Option<i8> = terrane_int_support::checked_coerce::<i8>(&(value));
    let absent: bool = (checked).is_none();
    let present: bool = (checked).is_some();
    let shadow_safe: i8 = terrane_int_support::saturating_coerce::<i8>(&(value));
    println!("{}", terrane_scalar_support::scalar_text(&(coerced)));
    println!("{}", terrane_scalar_support::scalar_text(&((renamed_checked).is_some())));
    println!("{}", terrane_scalar_support::scalar_text(&(shared_coerced)));
    println!("{}", terrane_scalar_support::scalar_text(&(parameter_coerced)));
    println!("{}", terrane_scalar_support::scalar_text(&(absent)));
    println!("{}", terrane_scalar_support::scalar_text(&(present)));
    println!("{}", terrane_scalar_support::scalar_text(&(shadow_safe)));
}
