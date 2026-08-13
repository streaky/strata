use terrane_compiler::display::ScalarDisplay;

#[test]
fn canonical_scalar_text_is_locale_independent() {
    assert_eq!(true.scalar_text(), "true");
    assert_eq!(false.scalar_text(), "false");
    assert_eq!((-128_i8).scalar_text(), "-128");
    assert_eq!(u128::MAX.scalar_text(), u128::MAX.to_string());
    assert_eq!("Terrane".scalar_text(), "Terrane");
}

#[test]
fn float_text_normalizes_non_finite_values_and_preserves_negative_zero() {
    assert_eq!(f64::INFINITY.scalar_text(), "inf");
    assert_eq!(f64::NEG_INFINITY.scalar_text(), "-inf");
    assert_eq!(f64::NAN.scalar_text(), "nan");
    assert_eq!((-0.0_f64).scalar_text(), "-0");
    assert_eq!(1.25_f32.scalar_text(), "1.25");
}
