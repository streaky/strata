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
    Float,
    Float32,
    Float64,
    String,
    None,
}

impl ScalarType {
    pub const ALL: [Self; 17] = [
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
        Self::Float,
        Self::Float32,
        Self::Float64,
        Self::String,
        Self::None,
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
            Self::Float => "float",
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
            Self::Float | Self::Float64 => Some("f64"),
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
        Self::ALL.into_iter().find(|ty| ty.source_name() == name)
    }

    #[must_use]
    pub const fn is_integer(self) -> bool {
        matches!(
            self,
            Self::Int
                | Self::Int8
                | Self::Int16
                | Self::Int32
                | Self::Int64
                | Self::Int128
                | Self::Uint8
                | Self::Uint16
                | Self::Uint32
                | Self::Uint64
                | Self::Uint128
        )
    }
}

impl fmt::Display for ScalarType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.source_name())
    }
}
