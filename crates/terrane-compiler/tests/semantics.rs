use std::path::PathBuf;

use terrane_compiler::semantics::SymbolKind;
use terrane_compiler::{Package, ScalarType, SourceFile, SourceUnit, ValueType, analyze};

fn package(prelude: bool, sources: &[(&str, &str)]) -> Package {
    Package {
        identity: "semantic-test".to_owned(),
        root: PathBuf::from("."),
        prelude,
        units: sources
            .iter()
            .enumerate()
            .map(|(id, (path, text))| SourceUnit {
                relative_path: PathBuf::from(path),
                source: SourceFile::new(
                    u32::try_from(id).unwrap(),
                    PathBuf::from(path),
                    (*text).to_owned(),
                ),
            })
            .collect(),
    }
}

#[test]
fn assembles_namespaces_symmetrically_before_import_resolution() {
    let analyzed = analyze(&package(
        false,
        &[
            ("consumer.trn", "namespace app\nfrom /shared import .item\n"),
            ("second.trn", "namespace shared\n.thing = 2\n"),
            ("first.trn", "namespace shared\n.item = 1\n"),
        ],
    ))
    .unwrap();

    assert_eq!(
        analyzed.object("/app", "item").unwrap().identity,
        "/shared::item"
    );
    assert!(analyzed.object("/shared", "thing").is_some());
}

#[test]
fn namespace_diagnostics_use_source_spelling() {
    let failure = analyze(&package(
        false,
        &[(
            "main.trn",
            "namespace app\nfrom /missing nested import .item\n",
        )],
    ))
    .unwrap_err();

    assert_eq!(
        failure.diagnostics[0].message,
        "unresolved object `.item` in `/missing nested`"
    );
}

#[test]
fn compiler_owned_namespaces_cannot_be_extended() {
    let failure = analyze(&package(
        false,
        &[("main.trn", "namespace core output\npublic .injected = 1\n")],
    ))
    .unwrap_err();

    assert_eq!(failure.diagnostics[0].code, "S2017");
    assert_eq!(
        failure.diagnostics[0].message,
        "cannot declare into compiler-owned namespace `/core output`"
    );
}
#[test]
fn resolves_exact_root_and_parent_namespace_anchors() {
    let analyzed = analyze(&package(
        false,
        &[
            ("exports.trn", "namespace parent shared\n.item = 1\n"),
            (
                "root.trn",
                "namespace root\nfrom /parent shared import .item as .root-item\n",
            ),
            (
                "child.trn",
                "namespace parent child\nfrom .. shared import .item as .parent-item\n",
            ),
        ],
    ))
    .unwrap();

    assert!(analyzed.object("/root", "root-item").is_some());
    assert!(analyzed.object("/parent/child", "parent-item").is_some());
}

#[test]
fn imports_are_object_form_and_require_explicit_ordinary_binding() {
    let analyzed = analyze(&package(
        false,
        &[(
            "main.trn",
            "namespace app\nfrom /core output import .print\nprinter = .print\n",
        )],
    ))
    .unwrap();

    assert!(analyzed.object("/app", "print").is_some());
    assert!(analyzed.ordinary("/app", "print").is_none());
    assert!(analyzed.ordinary("/app", "printer").is_some());
}

#[test]
fn identical_reimport_is_idempotent_and_collisions_need_aliases() {
    let accepted = analyze(&package(
        false,
        &[(
            "main.trn",
            "namespace app\nfrom /core output import .print\nfrom /core output import .print\n",
        )],
    ));
    assert!(accepted.is_ok());

    let rejected = analyze(&package(
        false,
        &[
            ("one.trn", "namespace one\n.item = 1\n"),
            ("two.trn", "namespace two\n.item = 2\n"),
            (
                "main.trn",
                "namespace app\nfrom /one import .item\nfrom /two import .item\n",
            ),
        ],
    ))
    .unwrap_err();
    assert_eq!(rejected.diagnostics[0].code, "S2011");
}

#[test]
fn prelude_has_exact_ordinary_bindings_and_can_be_disabled() {
    let enabled = analyze(&package(true, &[("main.trn", "namespace app\n")])).unwrap();
    let names = enabled.prelude_bindings.keys().cloned().collect::<Vec<_>>();
    assert_eq!(
        names,
        ["bool", "bytes", "float", "int", "none", "print", "string"]
    );

    let disabled = analyze(&package(false, &[("main.trn", "namespace app\n")])).unwrap();
    assert!(disabled.prelude_bindings.is_empty());
}

#[test]
fn bootstrap_registry_contains_versioned_modules_and_fixed_width_types() {
    let analyzed = analyze(&package(false, &[("main.trn", "namespace app\n")])).unwrap();

    assert_eq!(analyzed.bootstrap_version, "1");
    for namespace in [
        "/core/output",
        "/core/types",
        "/core/errors",
        "/collections",
    ] {
        assert!(analyzed.namespaces.contains_key(namespace));
    }
    for name in [
        "int8", "int16", "int32", "int64", "int128", "uint8", "uint16", "uint32", "uint64",
        "uint128", "float32", "float64",
    ] {
        assert!(analyzed.object("/core/types", name).is_some(), "{name}");
    }
}

#[test]
fn ordinary_import_binding_cannot_change_structural_imports() {
    let analyzed = analyze(&package(
        false,
        &[(
            "main.trn",
            "namespace app\nimport = 1\nfrom /core output import .print\n",
        )],
    ))
    .unwrap();

    assert!(analyzed.ordinary("/app", "import").is_some());
    assert!(analyzed.object("/app", "print").is_some());
}

#[test]
fn duplicate_declarations_and_private_imports_are_rejected() {
    let duplicate = analyze(&package(
        false,
        &[
            ("one.trn", "namespace app\nvalue = 1\n"),
            ("two.trn", "namespace app\nvalue = 2\n"),
        ],
    ))
    .unwrap_err();
    assert_eq!(duplicate.diagnostics[0].code, "S2005");

    let private = analyze(&package(
        false,
        &[
            ("exports.trn", "namespace hidden\nprivate .secret = 1\n"),
            (
                "consumer.trn",
                "namespace app\nfrom /hidden import .secret\n",
            ),
        ],
    ))
    .unwrap_err();
    assert_eq!(private.diagnostics[0].code, "S2010");
}

#[test]
fn global_replacement_is_distinct_from_namespace_local_assignment() {
    let analyzed = analyze(&package(
        false,
        &[
            ("one.trn", "namespace first\nglobal shared = 1\nlocal = 1\n"),
            (
                "two.trn",
                "namespace second\nglobal shared = 2\nlocal = 2\n",
            ),
        ],
    ))
    .unwrap();

    assert!(analyzed.ordinary("/first", "local").is_some());
    assert!(analyzed.ordinary("/second", "local").is_some());
    assert!(analyzed.ordinary("/first", "shared").is_none());
}

#[test]
fn namespace_local_bindings_may_shadow_program_globals() {
    let analyzed = analyze(&package(
        false,
        &[
            ("global.trn", "namespace owner\nglobal shared = 1\n"),
            ("local.trn", "namespace consumer\nshared = 2\n"),
            ("peer.trn", "namespace peer\n"),
        ],
    ))
    .unwrap();

    assert_eq!(
        analyzed
            .resolve_ordinary("/consumer", "shared")
            .unwrap()
            .namespace,
        "/consumer"
    );
    assert_eq!(
        analyzed
            .resolve_ordinary("/peer", "shared")
            .unwrap()
            .namespace,
        "/owner"
    );
}

#[test]
fn ordinary_lookup_uses_namespace_global_and_prelude_tiers() {
    let analyzed = analyze(&package(
        true,
        &[
            (
                "parent.trn",
                "namespace app\nparent-value = 1\nprotected inherited = 1\nprivate hidden = 1\n",
            ),
            (
                "child.trn",
                "namespace app child\nparent-value = 2\nglobal shared = 1\n",
            ),
            ("peer.trn", "namespace peer\n"),
        ],
    ))
    .unwrap();

    assert_eq!(
        analyzed
            .resolve_ordinary("/app/child", "parent-value")
            .unwrap()
            .namespace,
        "/app/child"
    );
    assert!(
        analyzed
            .resolve_ordinary("/app/child", "inherited")
            .is_some()
    );
    assert!(analyzed.resolve_ordinary("/peer", "hidden").is_none());
    assert!(analyzed.resolve_ordinary("/peer", "shared").is_some());
    assert!(analyzed.resolve_ordinary("/peer", "print").is_some());
}

#[test]
fn lexical_scopes_resolve_parameters_bindings_and_object_imports() {
    let source = concat!(
        "namespace app\n",
        "function render; argument int\n",
        "  from /core output import .print as .local-print\n",
        "  value = argument\n",
        "  if true\n",
        "    inner = value\n",
    );
    let analyzed = analyze(&package(false, &[("main.trn", source)])).unwrap();
    let unit = &analyzed.units[0];
    let inner_offset = source.find("inner =").unwrap();
    let value_offset = source.find("value =").unwrap();

    assert!(
        analyzed
            .resolve_ordinary_at(unit, value_offset, "argument")
            .is_some()
    );
    assert!(
        analyzed
            .resolve_ordinary_at(unit, inner_offset, "value")
            .is_some()
    );
    assert!(
        analyzed
            .resolve_ordinary_at(unit, value_offset, "inner")
            .is_none()
    );
    assert!(
        analyzed
            .resolve_object_at(unit, inner_offset, "local-print")
            .is_some()
    );
}

#[test]
fn duplicate_parameters_and_same_scope_bindings_are_rejected() {
    for source in [
        "namespace app\nfunction run; value int, value int\n",
        "namespace app\nfunction run\n  private value = 1\n  private value = 2\n",
    ] {
        let failure = analyze(&package(false, &[("main.trn", source)])).unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "S2012");
    }
}

#[test]
fn nested_global_declarations_populate_the_package_global_tier() {
    let analyzed = analyze(&package(
        false,
        &[(
            "main.trn",
            "namespace app\nfunction run\n  public global counter = 1\n",
        )],
    ))
    .unwrap();

    let counter = analyzed.globals.get("counter").unwrap();
    assert!(counter.global);
    assert_eq!(
        counter.visibility,
        terrane_compiler::semantics::Visibility::Public
    );
}

#[test]
fn nested_object_form_declarations_are_rejected_explicitly() {
    let failure = analyze(&package(
        false,
        &[("main.trn", "namespace app\nfunction run\n  .thing = 1\n")],
    ))
    .unwrap_err();

    assert_eq!(failure.diagnostics[0].code, "S2017");
}

#[test]
fn imports_report_inaccessible_exports_consistently_at_every_scope() {
    for consumer in [
        "namespace unrelated\nfrom /hidden import .item\n",
        "namespace unrelated\nfunction run\n  from /hidden import .item\n",
    ] {
        let failure = analyze(&package(
            false,
            &[
                ("hidden.trn", "namespace hidden\nprotected .item = 1\n"),
                ("consumer.trn", consumer),
            ],
        ))
        .unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "S2010");
    }
}

#[test]
fn imported_fixed_width_objects_remain_canonical_type_descriptors() {
    let analyzed = analyze(&package(
        false,
        &[(
            "main.trn",
            "namespace app\nfrom /core types import .int8, .uint128, .float32\n",
        )],
    ))
    .unwrap();

    for name in ["int8", "uint128", "float32"] {
        let descriptor = analyzed.object("/app", name).unwrap();
        assert_eq!(descriptor.kind, SymbolKind::TypeDescriptor);
        assert_eq!(descriptor.identity, format!("/core/types::{name}"));
    }
}

#[test]
fn infers_core_literal_types_and_checks_fixed_width_destinations() {
    let analyzed = analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "count = 42\n",
                "enabled = true\n",
                "message = 'ready'\n",
                "minimum int8 = -128\n",
            ),
        )],
    ))
    .unwrap();

    let bindings = &analyzed.units[0].typed_bindings;
    for (name, ty) in [
        ("count", ScalarType::Int),
        ("enabled", ScalarType::Bool),
        ("message", ScalarType::String),
        ("minimum", ScalarType::Int8),
    ] {
        assert_eq!(
            bindings
                .iter()
                .find(|binding| binding.name == name)
                .unwrap()
                .value_type,
            ValueType::Scalar(ty)
        );
    }
}

#[test]
fn imported_descriptor_aliases_drive_explicit_binding_types() {
    let analyzed = analyze(&package(
        false,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "from /core types import .uint8\n",
                "byte = .uint8\n",
                "maximum byte = 255\n",
            ),
        )],
    ))
    .unwrap();

    assert_eq!(
        analyzed.units[0].typed_bindings[0].value_type,
        ValueType::TypeDescriptor(ScalarType::Uint8)
    );
    assert_eq!(
        analyzed.units[0].typed_bindings[1].value_type,
        ValueType::Scalar(ScalarType::Uint8)
    );
}

#[test]
fn rejects_out_of_range_integer_constants_at_the_initializer() {
    let failure = analyze(&package(
        true,
        &[("main.trn", "namespace app\nvalue int8 = 128\n")],
    ))
    .unwrap_err();

    assert_eq!(failure.diagnostics[0].code, "T0003");
    assert_eq!(
        failure.diagnostics[0].message,
        "constant `128` is outside the range of `int8`"
    );
}
