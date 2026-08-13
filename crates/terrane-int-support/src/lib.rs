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
