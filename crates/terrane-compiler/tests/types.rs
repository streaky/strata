use terrane_compiler::ScalarType;

#[test]
fn every_native_scalar_has_the_contract_preserving_rust_representation() {
    let expected = [
        (ScalarType::Bool, "bool"),
        (ScalarType::Int8, "i8"),
        (ScalarType::Int16, "i16"),
        (ScalarType::Int32, "i32"),
        (ScalarType::Int64, "i64"),
        (ScalarType::Int128, "i128"),
        (ScalarType::Uint8, "u8"),
        (ScalarType::Uint16, "u16"),
        (ScalarType::Uint32, "u32"),
        (ScalarType::Uint64, "u64"),
        (ScalarType::Uint128, "u128"),
        (ScalarType::Float32, "f32"),
        (ScalarType::Float64, "f64"),
        (ScalarType::String, "String"),
        (ScalarType::None, "()"),
    ];

    for (ty, rust) in expected {
        assert_eq!(ty.rust_type(), Some(rust), "{ty}");
        assert_eq!(ScalarType::from_source_name(ty.source_name()), Some(ty));
    }

    assert_eq!(
        ScalarType::from_source_name("float"),
        Some(ScalarType::Float64)
    );
}

#[test]
fn adaptive_int_cannot_be_lowered_to_a_bounded_rust_primitive() {
    assert_eq!(ScalarType::Int.rust_type(), None);
    assert_eq!(ScalarType::Int.lowering_type(), "terrane_int_support::Int");
    assert!(ScalarType::Int.is_integer());
}
