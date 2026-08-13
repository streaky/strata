use num_bigint::BigInt;
use terrane_int_support::{ArithmeticError, Int, Tier};

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
fn division_and_modulo_follow_flooring_semantics() {
    let cases = [
        (-7_i64, 3_i64, "-3", "2"),
        (7, -3, "-3", "-2"),
        (-7, -3, "2", "-1"),
        (7, 3, "2", "1"),
    ];
    for (left, right, quotient, remainder) in cases {
        assert_eq!(
            Int::from(left)
                .floor_div(&Int::from(right))
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
        Int::from(1_i64).floor_div(&Int::from(0_i64)),
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
