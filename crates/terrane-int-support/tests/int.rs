use num_bigint::BigInt;
use terrane_int_support::{Int, Tier};

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
