use terrane_int_support::Int;
use terrane_scalar_support::scalar_text;

#[test]
fn displays_core_scalars_canonically() {
    assert_eq!(scalar_text(&true), "true");
    assert_eq!(scalar_text(&false), "false");
    assert_eq!(scalar_text(&Int::from(i128::MAX)), i128::MAX.to_string());
    assert_eq!(scalar_text(&"Terrane".to_owned()), "Terrane");
    assert_eq!(scalar_text(&()), "none");
    let text = "borrowed".to_owned();
    assert_eq!(scalar_text(&&text), "borrowed");
}

#[test]
fn normalizes_float_text_and_preserves_negative_zero() {
    assert_eq!(scalar_text(&f64::INFINITY), "inf");
    assert_eq!(scalar_text(&f64::NEG_INFINITY), "-inf");
    assert_eq!(scalar_text(&f64::NAN), "nan");
    assert_eq!(scalar_text(&-0.0_f64), "-0");
    assert_eq!(scalar_text(&1.25_f32), "1.25");
}
