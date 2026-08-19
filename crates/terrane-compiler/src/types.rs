use std::fmt;

/// Scalar types supported by the first Terrane compiler.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScalarType {
    Bool,
    Int,
    Int8,
    Int16,
    Int32,
    Int64,
    Int128,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Uint128,
    Float32,
    Float64,
    String,
    None,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeCategory {
    Value,
    Object,
    Number,
    Integer,
    FixedInteger,
    SignedFixedInteger,
    UnsignedFixedInteger,
    Floating,
}

impl TypeCategory {
    pub const ABSTRACT_SOURCE_NAMES: [(&'static str, Self); 6] = [
        ("number", Self::Number),
        ("integer", Self::Integer),
        ("fixed-integer", Self::FixedInteger),
        ("signed-fixed-integer", Self::SignedFixedInteger),
        ("unsigned-fixed-integer", Self::UnsignedFixedInteger),
        ("floating", Self::Floating),
    ];

    #[must_use]
    pub fn from_source_name(name: &str) -> Option<Self> {
        Self::ABSTRACT_SOURCE_NAMES
            .into_iter()
            .find_map(|(source_name, category)| (source_name == name).then_some(category))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorSchema {
    pub categories: &'static [TypeCategory],
}

const VALUE_CATEGORIES: &[TypeCategory] = &[TypeCategory::Value, TypeCategory::Object];
const INTEGER_CATEGORIES: &[TypeCategory] = &[
    TypeCategory::Value,
    TypeCategory::Object,
    TypeCategory::Number,
    TypeCategory::Integer,
];
const SIGNED_FIXED_CATEGORIES: &[TypeCategory] = &[
    TypeCategory::Value,
    TypeCategory::Object,
    TypeCategory::Number,
    TypeCategory::Integer,
    TypeCategory::FixedInteger,
    TypeCategory::SignedFixedInteger,
];
const UNSIGNED_FIXED_CATEGORIES: &[TypeCategory] = &[
    TypeCategory::Value,
    TypeCategory::Object,
    TypeCategory::Number,
    TypeCategory::Integer,
    TypeCategory::FixedInteger,
    TypeCategory::UnsignedFixedInteger,
];
const FLOATING_CATEGORIES: &[TypeCategory] = &[
    TypeCategory::Value,
    TypeCategory::Object,
    TypeCategory::Number,
    TypeCategory::Floating,
];

impl ScalarType {
    pub const ALL: [Self; 16] = [
        Self::Bool,
        Self::Int,
        Self::Int8,
        Self::Int16,
        Self::Int32,
        Self::Int64,
        Self::Int128,
        Self::Uint8,
        Self::Uint16,
        Self::Uint32,
        Self::Uint64,
        Self::Uint128,
        Self::Float32,
        Self::Float64,
        Self::String,
        Self::None,
    ];

    pub const SOURCE_NAMES: [(&'static str, Self); 17] = [
        ("bool", Self::Bool),
        ("int", Self::Int),
        ("int8", Self::Int8),
        ("int16", Self::Int16),
        ("int32", Self::Int32),
        ("int64", Self::Int64),
        ("int128", Self::Int128),
        ("uint8", Self::Uint8),
        ("uint16", Self::Uint16),
        ("uint32", Self::Uint32),
        ("uint64", Self::Uint64),
        ("uint128", Self::Uint128),
        ("float", Self::Float64),
        ("float32", Self::Float32),
        ("float64", Self::Float64),
        ("string", Self::String),
        ("none", Self::None),
    ];

    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Int8 => "int8",
            Self::Int16 => "int16",
            Self::Int32 => "int32",
            Self::Int64 => "int64",
            Self::Int128 => "int128",
            Self::Uint8 => "uint8",
            Self::Uint16 => "uint16",
            Self::Uint32 => "uint32",
            Self::Uint64 => "uint64",
            Self::Uint128 => "uint128",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
            Self::String => "string",
            Self::None => "none",
        }
    }

    /// Returns the direct Rust representation when Rust preserves the complete
    /// Terrane value contract. Adaptive `int` intentionally has no direct type.
    #[must_use]
    pub const fn rust_type(self) -> Option<&'static str> {
        match self {
            Self::Bool => Some("bool"),
            Self::Int => None,
            Self::Int8 => Some("i8"),
            Self::Int16 => Some("i16"),
            Self::Int32 => Some("i32"),
            Self::Int64 => Some("i64"),
            Self::Int128 => Some("i128"),
            Self::Uint8 => Some("u8"),
            Self::Uint16 => Some("u16"),
            Self::Uint32 => Some("u32"),
            Self::Uint64 => Some("u64"),
            Self::Uint128 => Some("u128"),
            Self::Float64 => Some("f64"),
            Self::Float32 => Some("f32"),
            Self::String => Some("String"),
            Self::None => Some("()"),
        }
    }
    /// Returns the Rust type used by generated code, including compiler support
    /// components for contracts a native primitive cannot preserve.
    #[must_use]
    pub const fn lowering_type(self) -> &'static str {
        match self.rust_type() {
            Some(native) => native,
            None => "terrane_int_support::Int",
        }
    }

    #[must_use]
    pub fn from_source_name(name: &str) -> Option<Self> {
        Self::SOURCE_NAMES
            .into_iter()
            .find_map(|(source_name, ty)| (source_name == name).then_some(ty))
    }

    #[must_use]
    pub const fn descriptor_schema(self) -> DescriptorSchema {
        let categories = match self {
            Self::Int => INTEGER_CATEGORIES,
            Self::Int8 | Self::Int16 | Self::Int32 | Self::Int64 | Self::Int128 => {
                SIGNED_FIXED_CATEGORIES
            }
            Self::Uint8 | Self::Uint16 | Self::Uint32 | Self::Uint64 | Self::Uint128 => {
                UNSIGNED_FIXED_CATEGORIES
            }
            Self::Float32 | Self::Float64 => FLOATING_CATEGORIES,
            Self::Bool | Self::String | Self::None => VALUE_CATEGORIES,
        };
        DescriptorSchema { categories }
    }

    #[must_use]
    pub const fn is_integer(self) -> bool {
        self.conforms_to(TypeCategory::Integer)
    }

    #[must_use]
    pub const fn conforms_to(self, category: TypeCategory) -> bool {
        let categories = self.descriptor_schema().categories;
        let mut index = 0;
        while index < categories.len() {
            if categories[index] as u8 == category as u8 {
                return true;
            }
            index += 1;
        }
        false
    }
}

impl fmt::Display for ScalarType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.source_name())
    }
}
