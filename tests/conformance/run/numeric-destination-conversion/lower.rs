// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: numeric-destination-conversion
fn main() {
    let small: i8 = 12;
    let adaptive: terrane_int_support::Int = terrane_int_support::Int::from((small) as i128);
    let wide: i32 = (small) as i32;
    let selected: terrane_int_support::Int = terrane_int_support::Int::from((small) as i128);
    let count: i32 = 16777216;
    let total: f64 = { let source = count; let converted = source as f64; if (converted as i32) == source { converted } else { terrane_int_support::unwrap_or_fail(Err(terrane_int_support::ArithmeticError::IntegerConversionOverflow)) } };
    let exact: terrane_int_support::Int = terrane_int_support::Int::from(18014398509481984_i128);
    let exact_float: f64 = terrane_int_support::unwrap_or_fail(terrane_int_support::exact_f64(&(exact)));
    let whole: f64 = 4.0;
    let converted: terrane_int_support::Int = terrane_int_support::unwrap_or_fail(terrane_int_support::exact_int_f64(whole));
    println!("{}", terrane_scalar_support::scalar_text(&(adaptive)));
    println!("{}", terrane_scalar_support::scalar_text(&(wide)));
    println!("{}", terrane_scalar_support::scalar_text(&(selected)));
    println!("{}", terrane_scalar_support::scalar_text(&(total)));
    println!("{}", terrane_scalar_support::scalar_text(&(exact_float)));
    println!("{}", terrane_scalar_support::scalar_text(&(converted)));
}
