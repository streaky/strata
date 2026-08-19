// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: exact-float-narrowing
fn main() {
    let source: f64 = 1.5;
    let target: f32 = { let source_value = source; let converted = source_value as f32; if (converted as f64) == source_value { converted } else { terrane_int_support::unwrap_or_fail(Err(terrane_int_support::ArithmeticError::conversion_overflow(&source_value, "float64", "float32", "the floating value is not exactly representable"))) } };
    println!("{}", terrane_scalar_support::scalar_text(&(target)));
}
