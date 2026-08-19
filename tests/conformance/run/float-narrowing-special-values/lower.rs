// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: float-narrowing-special-values
fn main() {
    let zero: f64 = 0.0;
    let one: f64 = 1.0;
    let negative_one: f64 = -1.0_f64;
    let negative_zero: f64 = -0.0_f64;
    let maximum: f64 = 340282346638528859811704183484516925440.0;
    let positive_infinity: f64 = one / zero;
    let negative_infinity: f64 = negative_one / zero;
    let narrowed_zero: f32 = { let source_value = negative_zero; let converted = source_value as f32; if (converted as f64) == source_value { converted } else { terrane_int_support::unwrap_or_fail(Err(terrane_int_support::ArithmeticError::conversion_overflow(&source_value, "float64", "float32", "the floating value is not exactly representable"))) } };
    let narrowed_positive: f32 = { let source_value = positive_infinity; let converted = source_value as f32; if (converted as f64) == source_value { converted } else { terrane_int_support::unwrap_or_fail(Err(terrane_int_support::ArithmeticError::conversion_overflow(&source_value, "float64", "float32", "the floating value is not exactly representable"))) } };
    let narrowed_negative: f32 = { let source_value = negative_infinity; let converted = source_value as f32; if (converted as f64) == source_value { converted } else { terrane_int_support::unwrap_or_fail(Err(terrane_int_support::ArithmeticError::conversion_overflow(&source_value, "float64", "float32", "the floating value is not exactly representable"))) } };
    let narrowed_maximum: f32 = { let source_value = maximum; let converted = source_value as f32; if (converted as f64) == source_value { converted } else { terrane_int_support::unwrap_or_fail(Err(terrane_int_support::ArithmeticError::conversion_overflow(&source_value, "float64", "float32", "the floating value is not exactly representable"))) } };
    println!("{}", terrane_scalar_support::scalar_text(&(narrowed_zero)));
    println!("{}", terrane_scalar_support::scalar_text(&(narrowed_positive)));
    println!("{}", terrane_scalar_support::scalar_text(&(narrowed_negative)));
    println!("{}", terrane_scalar_support::scalar_text(&(narrowed_maximum)));
}
