use std::cmp::Ordering;
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

/// Unwraps a runtime arithmetic result or terminates with stable source-oriented output.
pub fn unwrap_or_fail<T>(result: Result<T, ArithmeticError>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{}", error.render());
            std::process::exit(1);
        }
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

    /// Constructs an exact integer from a compiler-validated decimal literal.
    ///
    /// # Panics
    ///
    /// Panics when `value` is not a decimal integer. Generated code only calls this
    /// function with literals validated by the Terrane compiler.
    #[must_use]
    pub fn from_decimal(value: &str) -> Self {
        let value = BigInt::parse_bytes(value.as_bytes(), 10)
            .expect("Terrane compiler emitted an invalid integer literal");
        Self::from_big(value)
    }

    fn binary_big(&self, rhs: &Self, operation: impl FnOnce(BigInt, BigInt) -> BigInt) -> Self {
        Self::from_big(operation(self.as_big(), rhs.as_big()))
    }

    /// Exact Euclidean division, matching Terrane's signed integer contract.
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::DivisionByZero`] for a zero divisor.
    pub fn euclidean_div(&self, rhs: &Self) -> Result<Self, ArithmeticError> {
        let rhs = rhs.as_big();
        if rhs == BigInt::from(0_u8) {
            return Err(ArithmeticError::DivisionByZero);
        }
        let left = self.as_big();
        let quotient = &left / &rhs;
        let remainder = &left % &rhs;
        Ok(Self::from_big(if remainder < BigInt::from(0_u8) {
            if rhs < BigInt::from(0_u8) {
                quotient + 1
            } else {
                quotient - 1
            }
        } else {
            quotient
        }))
    }

    /// Nonnegative remainder paired with [`Self::euclidean_div`].
    ///
    /// # Errors
    ///
    /// Returns [`ArithmeticError::DivisionByZero`] for a zero divisor.
    pub fn modulo(&self, rhs: &Self) -> Result<Self, ArithmeticError> {
        let quotient = self.euclidean_div(rhs)?;
        Ok(Self::from_big(
            self.as_big() - quotient.as_big() * rhs.as_big(),
        ))
    }

    /// Exact left shift.
    ///
    /// # Errors
    ///
    /// Returns a stable error for a negative count or a result beyond this
    /// runtime's materialization limit.
    pub fn shift_left(&self, count: &Self) -> Result<Self, ArithmeticError> {
        if self == &Self::from(0_i64) {
            shift_count_sign(count)?;
            return Ok(self.clone());
        }
        let count = bounded_left_shift_count(count)?;
        Ok(Self::from_big(self.as_big() << count))
    }

    /// Arithmetic/flooring right shift over infinite two's-complement integers.
    ///
    /// # Errors
    ///
    /// Returns a stable error for a negative count.
    pub fn shift_right(&self, count: &Self) -> Result<Self, ArithmeticError> {
        let count = shift_count_sign(count)?;
        let significant_bits = self.as_big().bits();
        if count >= BigInt::from(significant_bits) {
            return Ok(Self::from(if self < &Self::from(0_i64) {
                -1_i64
            } else {
                0_i64
            }));
        }
        let count = count
            .to_usize()
            .ok_or(ArithmeticError::ShiftCountTooLarge)?;
        Ok(Self::from_big(self.as_big() >> count))
    }
}

const MAX_MATERIALIZED_SHIFT_BITS: usize = 1 << 20;

fn shift_count_sign(count: &Int) -> Result<BigInt, ArithmeticError> {
    let count = count.as_big();
    if count < BigInt::from(0_u8) {
        return Err(ArithmeticError::NegativeShiftCount);
    }
    Ok(count)
}

fn bounded_left_shift_count(count: &Int) -> Result<usize, ArithmeticError> {
    let count = shift_count_sign(count)?;
    let count = count
        .to_usize()
        .ok_or(ArithmeticError::ShiftCountTooLarge)?;
    if count > MAX_MATERIALIZED_SHIFT_BITS {
        return Err(ArithmeticError::ShiftCountTooLarge);
    }
    Ok(count)
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

impl Ord for Int {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Small(left), Self::Small(right)) => left.cmp(right),
            (Self::Small(left), Self::Wide(right)) => i128::from(*left).cmp(right),
            (Self::Wide(left), Self::Small(right)) => left.cmp(&i128::from(*right)),
            (Self::Wide(left), Self::Wide(right)) => left.cmp(right),
            (Self::Big(left), Self::Big(right)) => left.cmp(right),
            _ => self.as_big().cmp(&other.as_big()),
        }
    }
}

impl PartialOrd for Int {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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
                    (Self::Small(left), Self::Small(right)) => left.$checked(*right).map_or_else(
                        || Self::from(i128::from(*left) $operator i128::from(*right)),
                        Self::Small,
                    ),
                    (Self::Small(left), Self::Wide(right)) => i128::from(*left)
                        .$checked(*right)
                        .map_or_else(
                            || self.binary_big(&rhs, |left, right| left $operator right),
                            Self::from,
                        ),
                    (Self::Wide(left), Self::Small(right)) => left
                        .$checked(i128::from(*right))
                        .map_or_else(
                            || self.binary_big(&rhs, |left, right| left $operator right),
                            Self::from,
                        ),
                    (Self::Wide(left), Self::Wide(right)) => left
                        .$checked(*right)
                        .map_or_else(
                            || self.binary_big(&rhs, |left, right| left $operator right),
                            Self::from,
                        ),
                    _ => self.binary_big(&rhs, |left, right| left $operator right),
                }
            }
        }
    };
}

exact_binary!(Add, add, checked_add, +);
exact_binary!(Sub, sub, checked_sub, -);
exact_binary!(Mul, mul, checked_mul, *);

macro_rules! exact_bitwise {
    ($trait:ident, $method:ident, $operator:tt) => {
        impl $trait for Int {
            type Output = Self;

            fn $method(self, rhs: Self) -> Self::Output {
                match (&self, &rhs) {
                    (Self::Small(left), Self::Small(right)) => Self::from(*left $operator *right),
                    (Self::Small(left), Self::Wide(right)) => {
                        Self::from(i128::from(*left) $operator *right)
                    }
                    (Self::Wide(left), Self::Small(right)) => {
                        Self::from(*left $operator i128::from(*right))
                    }
                    (Self::Wide(left), Self::Wide(right)) => Self::from(*left $operator *right),
                    _ => self.binary_big(&rhs, |left, right| left $operator right),
                }
            }
        }
    };
}

exact_bitwise!(BitAnd, bitand, &);
exact_bitwise!(BitOr, bitor, |);
exact_bitwise!(BitXor, bitxor, ^);

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
        match self {
            Self::Small(value) => Self::from(!value),
            Self::Wide(value) => Self::from(!value),
            Self::Big(value) => Self::from_big(!value),
        }
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

/// Converts an integer value to the exact representation used by coercion policies.
pub trait IntegerSource {
    fn integer_value(&self) -> BigInt;
}

impl IntegerSource for Int {
    fn integer_value(&self) -> BigInt {
        self.as_big()
    }
}

macro_rules! integer_sources {
    ($($type:ty),+ $(,)?) => {$(
        impl IntegerSource for $type {
            fn integer_value(&self) -> BigInt {
                BigInt::from(*self)
            }
        }
    )+};
}

integer_sources!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

/// Destination contract for explicit integer coercions.
pub trait IntegerDestination: Sized {
    fn checked_from_big(value: &BigInt) -> Option<Self>;
    fn wrapping_from_big(value: &BigInt) -> Self;
    fn saturating_from_big(value: &BigInt) -> Self;
}

impl IntegerDestination for Int {
    fn checked_from_big(value: &BigInt) -> Option<Self> {
        Some(Self::from_big(value.clone()))
    }

    fn wrapping_from_big(value: &BigInt) -> Self {
        Self::from_big(value.clone())
    }

    fn saturating_from_big(value: &BigInt) -> Self {
        Self::from_big(value.clone())
    }
}

macro_rules! signed_destinations {
    ($(($type:ty, $to:ident)),+ $(,)?) => {$(
        impl IntegerDestination for $type {
            fn checked_from_big(value: &BigInt) -> Option<Self> {
                value.$to()
            }

            fn wrapping_from_big(value: &BigInt) -> Self {
                let modulus = BigInt::from(1_u8) << <$type>::BITS;
                let sign = BigInt::from(1_u8) << (<$type>::BITS - 1);
                let mut wrapped = value % &modulus;
                if wrapped < BigInt::from(0_u8) {
                    wrapped += &modulus;
                }
                if wrapped >= sign {
                    wrapped -= modulus;
                }
                wrapped.$to().expect("wrapped integer must fit its destination")
            }

            fn saturating_from_big(value: &BigInt) -> Self {
                if value < &BigInt::from(<$type>::MIN) {
                    <$type>::MIN
                } else if value > &BigInt::from(<$type>::MAX) {
                    <$type>::MAX
                } else {
                    value.$to().expect("bounded integer must fit its destination")
                }
            }
        }
    )+};
}

macro_rules! unsigned_destinations {
    ($(($type:ty, $to:ident)),+ $(,)?) => {$(
        impl IntegerDestination for $type {
            fn checked_from_big(value: &BigInt) -> Option<Self> {
                value.$to()
            }

            fn wrapping_from_big(value: &BigInt) -> Self {
                let modulus = BigInt::from(1_u8) << <$type>::BITS;
                let mut wrapped = value % &modulus;
                if wrapped < BigInt::from(0_u8) {
                    wrapped += modulus;
                }
                wrapped.$to().expect("wrapped integer must fit its destination")
            }

            fn saturating_from_big(value: &BigInt) -> Self {
                if value < &BigInt::from(0_u8) {
                    0
                } else if value > &BigInt::from(<$type>::MAX) {
                    <$type>::MAX
                } else {
                    value.$to().expect("bounded integer must fit its destination")
                }
            }
        }
    )+};
}

signed_destinations!(
    (i8, to_i8),
    (i16, to_i16),
    (i32, to_i32),
    (i64, to_i64),
    (i128, to_i128),
);
unsigned_destinations!(
    (u8, to_u8),
    (u16, to_u16),
    (u32, to_u32),
    (u64, to_u64),
    (u128, to_u128),
);

/// Performs an exact integer coercion.
///
/// # Errors
///
/// Returns [`ArithmeticError::IntegerConversionOverflow`] when the result is
/// outside the destination type.
pub fn coerce<T: IntegerDestination>(value: &impl IntegerSource) -> Result<T, ArithmeticError> {
    T::checked_from_big(&value.integer_value()).ok_or(ArithmeticError::IntegerConversionOverflow)
}

/// Performs a non-failing checked integer coercion.
pub fn checked_coerce<T: IntegerDestination>(value: &impl IntegerSource) -> Option<T> {
    T::checked_from_big(&value.integer_value())
}

/// Performs a two's-complement wrapping integer coercion.
#[must_use]
pub fn wrapping_coerce<T: IntegerDestination>(value: &impl IntegerSource) -> T {
    T::wrapping_from_big(&value.integer_value())
}

/// Performs a bounded saturating integer coercion.
#[must_use]
pub fn saturating_coerce<T: IntegerDestination>(value: &impl IntegerSource) -> T {
    T::saturating_from_big(&value.integer_value())
}
