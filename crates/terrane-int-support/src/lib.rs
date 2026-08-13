use std::fmt;
use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use num_bigint::BigInt;
use num_traits::ToPrimitive;

/// Exact Terrane `int`, normalized to the smallest representation after every operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Int {
    Small(i64),
    Wide(i128),
    Big(BigInt),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArithmeticError {
    DivisionByZero,
    ArithmeticOverflow,
    IntegerConversionOverflow,
    NegativeShiftCount,
    ShiftCountTooLarge,
}

impl fmt::Display for ArithmeticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DivisionByZero => "integer division by zero",
            Self::ArithmeticOverflow => "fixed-width integer arithmetic overflow",
            Self::IntegerConversionOverflow => {
                "integer conversion result is outside the destination type"
            }
            Self::NegativeShiftCount => "negative integer shift count",
            Self::ShiftCountTooLarge => "integer shift count cannot be represented on this target",
        })
    }
}

impl std::error::Error for ArithmeticError {}

impl ArithmeticError {
    /// Stable Terrane object-form name used by generated failure paths.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::DivisionByZero => ".division-by-zero",
            Self::ArithmeticOverflow => ".arithmetic-overflow",
            Self::IntegerConversionOverflow => ".integer-conversion-overflow",
            Self::NegativeShiftCount => ".negative-shift-count",
            Self::ShiftCountTooLarge => ".resource-error",
        }
    }

    /// Deterministic source-oriented text suitable for an uncaught failure.
    #[must_use]
    pub fn render(self) -> String {
        format!("{}: {self}", self.source_name())
    }
}

impl Int {
    #[must_use]
    pub fn from_big(value: BigInt) -> Self {
        if let Some(value) = value.to_i64() {
            Self::Small(value)
        } else if let Some(value) = value.to_i128() {
            Self::Wide(value)
        } else {
            Self::Big(value)
        }
    }

    #[must_use]
    pub const fn tier(&self) -> Tier {
        match self {
            Self::Small(_) => Tier::I64,
            Self::Wide(_) => Tier::I128,
            Self::Big(_) => Tier::Arbitrary,
        }
    }

    #[must_use]
    pub fn as_big(&self) -> BigInt {
        match self {
            Self::Small(value) => BigInt::from(*value),
            Self::Wide(value) => BigInt::from(*value),
            Self::Big(value) => value.clone(),
        }
    }

    fn binary_big(&self, rhs: &Self, operation: impl FnOnce(BigInt, BigInt) -> BigInt) -> Self {
        Self::from_big(operation(self.as_big(), rhs.as_big()))
    }

    /// Exact flooring division, matching Terrane's signed integer contract.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::DivisionByZero`] for a zero divisor.
    pub fn floor_div(&self, rhs: &Self) -> Result<Self, ArithmeticError> {
        let rhs = rhs.as_big();
        if rhs == BigInt::from(0_u8) {
            return Err(ArithmeticError::DivisionByZero);
        }
        let left = self.as_big();
        let quotient = &left / &rhs;
        let remainder = &left % &rhs;
        let floors_down = remainder != BigInt::from(0_u8)
            && ((remainder < BigInt::from(0_u8)) != (rhs < BigInt::from(0_u8)));
        Ok(Self::from_big(if floors_down {
            quotient - 1
        } else {
            quotient
        }))
    }

    /// Remainder paired with [`Self::floor_div`], carrying the divisor's sign.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::DivisionByZero`] for a zero divisor.
    pub fn modulo(&self, rhs: &Self) -> Result<Self, ArithmeticError> {
        let quotient = self.floor_div(rhs)?;
        Ok(Self::from_big(
            self.as_big() - quotient.as_big() * rhs.as_big(),
        ))
    }

    /// Exact left shift.
    ///
    /// # Errors
    ///
    /// Returns a stable error for a negative or target-unrepresentable count.
    pub fn shift_left(&self, count: &Self) -> Result<Self, ArithmeticError> {
        let count = shift_count(count)?;
        Ok(Self::from_big(self.as_big() << count))
    }

    /// Arithmetic/flooring right shift over infinite two's-complement integers.
    ///
    /// # Errors
    ///
    /// Returns a stable error for a negative or target-unrepresentable count.
    pub fn shift_right(&self, count: &Self) -> Result<Self, ArithmeticError> {
        let count = shift_count(count)?;
        Ok(Self::from_big(self.as_big() >> count))
    }
}

fn shift_count(count: &Int) -> Result<usize, ArithmeticError> {
    let count = count.as_big();
    if count < BigInt::from(0_u8) {
        return Err(ArithmeticError::NegativeShiftCount);
    }
    count.to_usize().ok_or(ArithmeticError::ShiftCountTooLarge)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tier {
    I64,
    I128,
    Arbitrary,
}

impl From<i64> for Int {
    fn from(value: i64) -> Self {
        Self::Small(value)
    }
}

impl From<i128> for Int {
    fn from(value: i128) -> Self {
        if let Ok(value) = i64::try_from(value) {
            Self::Small(value)
        } else {
            Self::Wide(value)
        }
    }
}

impl fmt::Display for Int {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Small(value) => value.fmt(formatter),
            Self::Wide(value) => value.fmt(formatter),
            Self::Big(value) => value.fmt(formatter),
        }
    }
}

macro_rules! exact_binary {
    ($trait:ident, $method:ident, $checked:ident, $operator:tt) => {
        impl $trait for Int {
            type Output = Self;

            fn $method(self, rhs: Self) -> Self::Output {
                match (&self, &rhs) {
                    (Self::Small(left), Self::Small(right)) => left
                        .$checked(*right)
                        .map_or_else(|| Self::from(i128::from(*left) $operator i128::from(*right)), Self::Small),
                    (Self::Wide(left), Self::Wide(right)) => left
                        .$checked(*right)
                        .map_or_else(|| self.binary_big(&rhs, |left, right| left $operator right), Self::from),
                    _ => self.binary_big(&rhs, |left, right| left $operator right),
                }
            }
        }
    };
}

exact_binary!(Add, add, checked_add, +);
exact_binary!(Sub, sub, checked_sub, -);
exact_binary!(Mul, mul, checked_mul, *);

macro_rules! big_binary {
    ($trait:ident, $method:ident, $operator:tt) => {
        impl $trait for Int {
            type Output = Self;

            fn $method(self, rhs: Self) -> Self::Output {
                self.binary_big(&rhs, |left, right| left $operator right)
            }
        }
    };
}

big_binary!(BitAnd, bitand, &);
big_binary!(BitOr, bitor, |);
big_binary!(BitXor, bitxor, ^);

impl Neg for Int {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Self::Small(value) => value
                .checked_neg()
                .map_or(Self::Wide(-i128::from(value)), Self::Small),
            Self::Wide(value) => value
                .checked_neg()
                .map_or_else(|| Self::from_big(-BigInt::from(value)), Self::from),
            Self::Big(value) => Self::from_big(-value),
        }
    }
}

impl Not for Int {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self::from_big(!self.as_big())
    }
}

/// The four explicit overflow policies shared by every fixed-width integer.
pub trait FixedWidthArithmetic: Sized + Copy {
    fn checked_addition(self, rhs: Self) -> Option<Self>;
    #[must_use]
    fn wrapping_addition(self, rhs: Self) -> Self;
    #[must_use]
    fn saturating_addition(self, rhs: Self) -> Self;
    fn overflowing_addition(self, rhs: Self) -> (Self, bool);
    fn checked_subtraction(self, rhs: Self) -> Option<Self>;
    #[must_use]
    fn wrapping_subtraction(self, rhs: Self) -> Self;
    #[must_use]
    fn saturating_subtraction(self, rhs: Self) -> Self;
    fn overflowing_subtraction(self, rhs: Self) -> (Self, bool);
    fn checked_multiplication(self, rhs: Self) -> Option<Self>;
    #[must_use]
    fn wrapping_multiplication(self, rhs: Self) -> Self;
    #[must_use]
    fn saturating_multiplication(self, rhs: Self) -> Self;
    fn overflowing_multiplication(self, rhs: Self) -> (Self, bool);
    /// # Errors
    /// Returns division by zero when `rhs` is zero.
    fn checked_division(self, rhs: Self) -> Result<Option<Self>, ArithmeticError>;
    /// # Errors
    /// Returns division by zero when `rhs` is zero.
    fn wrapping_division(self, rhs: Self) -> Result<Self, ArithmeticError>;
    /// # Errors
    /// Returns division by zero when `rhs` is zero.
    fn saturating_division(self, rhs: Self) -> Result<Self, ArithmeticError>;
    /// # Errors
    /// Returns division by zero when `rhs` is zero.
    fn overflowing_division(self, rhs: Self) -> Result<(Self, bool), ArithmeticError>;
    /// # Errors
    /// Returns division by zero when `rhs` is zero.
    fn checked_remainder(self, rhs: Self) -> Result<Option<Self>, ArithmeticError>;
    /// # Errors
    /// Returns division by zero when `rhs` is zero.
    fn wrapping_remainder(self, rhs: Self) -> Result<Self, ArithmeticError>;
    /// # Errors
    /// Returns division by zero when `rhs` is zero.
    fn saturating_remainder(self, rhs: Self) -> Result<Self, ArithmeticError>;
    /// # Errors
    /// Returns division by zero when `rhs` is zero.
    fn overflowing_remainder(self, rhs: Self) -> Result<(Self, bool), ArithmeticError>;
}

macro_rules! fixed_width_arithmetic {
    ($($type:ty),+ $(,)?) => {$(
        impl FixedWidthArithmetic for $type {
            fn checked_addition(self, rhs: Self) -> Option<Self> { self.checked_add(rhs) }
            fn wrapping_addition(self, rhs: Self) -> Self { self.wrapping_add(rhs) }
            fn saturating_addition(self, rhs: Self) -> Self { self.saturating_add(rhs) }
            fn overflowing_addition(self, rhs: Self) -> (Self, bool) { self.overflowing_add(rhs) }
            fn checked_subtraction(self, rhs: Self) -> Option<Self> { self.checked_sub(rhs) }
            fn wrapping_subtraction(self, rhs: Self) -> Self { self.wrapping_sub(rhs) }
            fn saturating_subtraction(self, rhs: Self) -> Self { self.saturating_sub(rhs) }
            fn overflowing_subtraction(self, rhs: Self) -> (Self, bool) { self.overflowing_sub(rhs) }
            fn checked_multiplication(self, rhs: Self) -> Option<Self> { self.checked_mul(rhs) }
            fn wrapping_multiplication(self, rhs: Self) -> Self { self.wrapping_mul(rhs) }
            fn saturating_multiplication(self, rhs: Self) -> Self { self.saturating_mul(rhs) }
            fn overflowing_multiplication(self, rhs: Self) -> (Self, bool) { self.overflowing_mul(rhs) }
            fn checked_division(self, rhs: Self) -> Result<Option<Self>, ArithmeticError> {
                if rhs == 0 { Err(ArithmeticError::DivisionByZero) } else { Ok(self.checked_div(rhs)) }
            }
            fn wrapping_division(self, rhs: Self) -> Result<Self, ArithmeticError> {
                if rhs == 0 { Err(ArithmeticError::DivisionByZero) } else { Ok(self.wrapping_div(rhs)) }
            }
            fn saturating_division(self, rhs: Self) -> Result<Self, ArithmeticError> {
                if rhs == 0 { Err(ArithmeticError::DivisionByZero) } else { Ok(self.saturating_div(rhs)) }
            }
            fn overflowing_division(self, rhs: Self) -> Result<(Self, bool), ArithmeticError> {
                if rhs == 0 { Err(ArithmeticError::DivisionByZero) } else { Ok(self.overflowing_div(rhs)) }
            }
            fn checked_remainder(self, rhs: Self) -> Result<Option<Self>, ArithmeticError> {
                if rhs == 0 { Err(ArithmeticError::DivisionByZero) } else { Ok(self.checked_rem(rhs)) }
            }
            fn wrapping_remainder(self, rhs: Self) -> Result<Self, ArithmeticError> {
                if rhs == 0 { Err(ArithmeticError::DivisionByZero) } else { Ok(self.wrapping_rem(rhs)) }
            }
            fn saturating_remainder(self, rhs: Self) -> Result<Self, ArithmeticError> {
                if rhs == 0 { Err(ArithmeticError::DivisionByZero) } else { Ok(self.wrapping_rem(rhs)) }
            }
            fn overflowing_remainder(self, rhs: Self) -> Result<(Self, bool), ArithmeticError> {
                if rhs == 0 { Err(ArithmeticError::DivisionByZero) } else { Ok(self.overflowing_rem(rhs)) }
            }
        }
    )+};
}

fixed_width_arithmetic!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);
