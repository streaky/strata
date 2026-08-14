use num_bigint::BigInt;
use terrane_int_support::{
    ArithmeticError, FixedWidthArithmetic, Int, Tier, checked_coerce, coerce, saturating_coerce,
    wrapping_coerce,
};

#[test]
fn arithmetic_promotes_and_normalizes_exactly() {
    let wide = Int::from(i64::MAX) + Int::from(1_i64);
    assert_eq!(wide.tier(), Tier::I128);
    assert_eq!(wide.to_string(), "9223372036854775808");

    let big = Int::from(i128::MAX) + Int::from(1_i64);
    assert_eq!(big.tier(), Tier::Arbitrary);
    assert_eq!(big, Int::from_big(BigInt::from(i128::MAX) + 1));

    let normalized = big - Int::from_big(BigInt::from(i128::MAX));
    assert_eq!(normalized, Int::from(1_i64));
    assert_eq!(normalized.tier(), Tier::I64);
}

#[test]
fn mixed_compact_tiers_compute_without_arbitrary_promotion() {
    let wide = Int::from(i128::from(i64::MAX) + 1);
    assert_eq!((wide.clone() + Int::from(-1_i64)).tier(), Tier::I64);
    assert_eq!((Int::from(-1_i64) + wide.clone()).tier(), Tier::I64);
    assert_eq!((wide.clone() * Int::from(2_i64)).tier(), Tier::I128);
    assert_eq!(wide.clone() & Int::from(i64::MAX), Int::from(0_i64));
    assert_eq!((!wide.clone()).tier(), Tier::I128);
    assert!(wide > Int::from(i64::MAX));
}

#[test]
fn signed_minimum_negation_promotes_without_overflow() {
    assert_eq!((-Int::from(i64::MIN)).tier(), Tier::I128);
    assert_eq!((-Int::from(i128::MIN)).tier(), Tier::Arbitrary);
}

#[test]
fn bitwise_operations_use_infinite_twos_complement() {
    assert_eq!((!Int::from(0_i64)).to_string(), "-1");
    assert_eq!((Int::from(-1_i64) & Int::from(255_i64)).to_string(), "255");
}

#[test]
fn division_and_modulo_follow_euclidean_semantics() {
    let cases = [
        (-7_i64, 3_i64, "-3", "2"),
        (7, -3, "-2", "1"),
        (-7, -3, "3", "2"),
        (7, 3, "2", "1"),
    ];
    for (left, right, quotient, remainder) in cases {
        assert_eq!(
            Int::from(left)
                .euclidean_div(&Int::from(right))
                .unwrap()
                .to_string(),
            quotient
        );
        assert_eq!(
            Int::from(left)
                .modulo(&Int::from(right))
                .unwrap()
                .to_string(),
            remainder
        );
    }
    assert_eq!(
        Int::from(1_i64).euclidean_div(&Int::from(0_i64)),
        Err(ArithmeticError::DivisionByZero)
    );
}

#[test]
fn shifts_are_exact_and_reject_negative_counts() {
    let shifted = Int::from(1_i64).shift_left(&Int::from(130_i64)).unwrap();
    assert_eq!(shifted.tier(), Tier::Arbitrary);
    assert_eq!(
        shifted.shift_right(&Int::from(129_i64)).unwrap(),
        Int::from(2_i64)
    );
    assert_eq!(
        Int::from(-3_i64).shift_right(&Int::from(1_i64)).unwrap(),
        Int::from(-2_i64)
    );
    assert_eq!(
        Int::from(1_i64).shift_left(&Int::from(-1_i64)),
        Err(ArithmeticError::NegativeShiftCount)
    );
}

#[test]
fn shifts_bound_materialization_and_collapse_oversized_right_counts() {
    let huge = Int::from_decimal("100000000000000000000000000000000000");
    assert_eq!(
        Int::from(1_i64).shift_left(&huge),
        Err(ArithmeticError::ShiftCountTooLarge)
    );
    assert_eq!(Int::from(0_i64).shift_left(&huge), Ok(Int::from(0_i64)));
    assert_eq!(Int::from(9_i64).shift_right(&huge), Ok(Int::from(0_i64)));
    assert_eq!(Int::from(-9_i64).shift_right(&huge), Ok(Int::from(-1_i64)));
}

#[test]
fn fixed_width_operation_families_pin_overflow_modes() {
    assert_eq!(i8::MAX.checked_addition(1), None);
    assert_eq!(i8::MAX.wrapping_addition(1), i8::MIN);
    assert_eq!(i8::MAX.saturating_addition(1), i8::MAX);
    assert_eq!(i8::MAX.overflowing_addition(1), (i8::MIN, true));

    assert_eq!(i8::MIN.checked_division(-1), Ok(None));
    assert_eq!(i8::MIN.wrapping_division(-1), Ok(i8::MIN));
    assert_eq!(i8::MIN.saturating_division(-1), Ok(i8::MAX));
    assert_eq!(i8::MIN.overflowing_division(-1), Ok((i8::MIN, true)));
    assert_eq!(
        1_u8.checked_division(0),
        Err(ArithmeticError::DivisionByZero)
    );
    assert_eq!(
        1_u8.wrapping_remainder(0),
        Err(ArithmeticError::DivisionByZero)
    );
}

#[test]
fn fixed_width_default_operations_report_failures() {
    assert_eq!(
        terrane_int_support::fixed_addition(i8::MAX, 1),
        Err(ArithmeticError::ArithmeticOverflow)
    );
    assert_eq!(
        terrane_int_support::fixed_division(1_i8, 0),
        Err(ArithmeticError::DivisionByZero)
    );
    assert_eq!(terrane_int_support::fixed_remainder(7_u8, 4), Ok(3));
    assert_eq!(terrane_int_support::fixed_shift_left(3_i8, &2_i8), Ok(12));
    assert_eq!(
        terrane_int_support::fixed_shift_left(120_i8, &1_i8),
        Err(ArithmeticError::ArithmeticOverflow)
    );
    assert_eq!(
        terrane_int_support::fixed_shift_right(1_u8, &8_u8),
        Err(ArithmeticError::ShiftCountTooLarge)
    );
    assert_eq!(
        terrane_int_support::fixed_shift_left(1_i8, &-1_i8),
        Err(ArithmeticError::NegativeShiftCount)
    );
}

#[test]
fn integer_coercion_families_cover_signed_unsigned_and_adaptive_values() {
    assert_eq!(coerce::<i8>(&127_i128), Ok(127));
    assert_eq!(
        coerce::<i8>(&128_i128),
        Err(ArithmeticError::IntegerConversionOverflow)
    );
    assert_eq!(checked_coerce::<u8>(&-1_i16), None);
    assert_eq!(wrapping_coerce::<i8>(&255_u16), -1);
    assert_eq!(wrapping_coerce::<u8>(&-1_i16), 255);
    assert_eq!(saturating_coerce::<i8>(&1000_i128), i8::MAX);
    assert_eq!(saturating_coerce::<u8>(&-1000_i128), 0);

    let arbitrary = Int::from_big(BigInt::from(u128::MAX) + 1);
    assert_eq!(wrapping_coerce::<u8>(&arbitrary), 0);
    assert_eq!(coerce::<Int>(&arbitrary), Ok(arbitrary));
}

#[test]
fn runtime_arithmetic_failures_render_in_source_terms() {
    let cases = [
        (
            ArithmeticError::DivisionByZero,
            ".division-by-zero: integer division by zero",
        ),
        (
            ArithmeticError::ArithmeticOverflow,
            ".arithmetic-overflow: fixed-width integer arithmetic overflow",
        ),
        (
            ArithmeticError::IntegerConversionOverflow,
            ".integer-conversion-overflow: integer conversion result is outside the destination type",
        ),
        (
            ArithmeticError::NegativeShiftCount,
            ".negative-shift-count: negative integer shift count",
        ),
        (
            ArithmeticError::ShiftCountTooLarge,
            ".resource-error: integer shift count cannot be represented on this target",
        ),
    ];
    for (failure, expected) in cases {
        assert_eq!(failure.render(), expected);
    }
}
