use std::path::PathBuf;

use terrane_compiler::semantics::SymbolKind;
use terrane_compiler::syntax::SyntaxKind;
use terrane_compiler::{
    EvaluationKind, Package, ScalarType, SourceFile, SourceUnit, ValueType, analyze,
};

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
    assert_eq!(enabled.descriptor_constructs.len(), ScalarType::ALL.len());
    assert_eq!(disabled.descriptor_constructs.len(), ScalarType::ALL.len());
    for ty in ScalarType::ALL {
        let construct = disabled
            .descriptor_constructs
            .get(ty.source_name())
            .unwrap();
        assert_eq!(construct.descriptor_type(), Some(ty));
    }
}

#[test]
fn descriptor_construct_aliases_are_typed_without_the_prelude() {
    let analyzed = analyze(&package(
        false,
        &[(
            "main.trn",
            "namespace app\ntarget = int8\nsame = target\nfunction main\n  value int8 = 1\n  result bool = value is a same\n",
        )],
    ))
    .unwrap();
    let unit = &analyzed.units[0];
    for name in ["target", "same"] {
        assert!(unit.typed_bindings.iter().any(|binding| {
            binding.name == name
                && binding.value_type
                    == terrane_compiler::semantics::ValueType::TypeDescriptor(ScalarType::Int8)
        }));
    }
}

#[test]
fn bootstrap_registry_contains_versioned_modules_and_fixed_width_types() {
    let analyzed = analyze(&package(false, &[("main.trn", "namespace app\n")])).unwrap();

    assert_eq!(analyzed.bootstrap_version, "1");
    for namespace in [
        "/core/output",
        "/core/types",
        "/core/errors",
        "/core/collections",
    ] {
        assert!(analyzed.namespaces.contains_key(namespace));
    }
    assert!(!analyzed.namespaces.contains_key("/collections"));
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
    let analyzed = analyze(&package(true, &[("main.trn", source)])).unwrap();
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
        assert_eq!(
            descriptor.descriptor_type(),
            ScalarType::from_source_name(name)
        );
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

    let bindings = &analyzed.units[0].typed_bindings;
    assert_eq!(
        bindings
            .iter()
            .find(|binding| binding.name == "byte")
            .unwrap()
            .value_type,
        ValueType::TypeDescriptor(ScalarType::Uint8)
    );
    assert_eq!(
        bindings
            .iter()
            .find(|binding| binding.name == "maximum")
            .unwrap()
            .value_type,
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

#[test]
fn records_typed_parameters_defaults_and_return_contracts() {
    let analyzed = analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "function connect bool; host string, retries int = 2\n",
                "  return true\n",
            ),
        )],
    ))
    .unwrap();

    let contract = &analyzed.units[0].functions[0];
    assert_eq!(contract.name, "connect");
    assert_eq!(contract.return_type, Some(ScalarType::Bool));
    assert_eq!(contract.parameters[0].value_type, Some(ScalarType::String));
    assert!(!contract.parameters[0].optional);
    assert_eq!(contract.parameters[1].value_type, Some(ScalarType::Int));
    assert!(contract.parameters[1].optional);
}

#[test]
fn descriptor_aliases_resolve_function_contracts_in_source_order() {
    let analyzed = analyze(&package(
        false,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "from /core types import .uint8\n",
                "byte = .uint8\n",
                "function identity byte; value byte\n",
                "  return value\n",
            ),
        )],
    ))
    .unwrap();

    let contract = &analyzed.units[0].functions[0];
    assert_eq!(contract.return_type, Some(ScalarType::Uint8));
    assert_eq!(contract.parameters[0].value_type, Some(ScalarType::Uint8));

    let failure = analyze(&package(
        false,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "from /core types import .uint8\n",
                "function identity byte; value byte\n",
                "  return value\n",
                "byte = .uint8\n",
            ),
        )],
    ))
    .unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "T0001");
}

#[test]
fn rejects_required_parameters_after_optional_parameters() {
    let failure = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction connect; timeout int = 2, host string\n",
        )],
    ))
    .unwrap_err();

    assert_eq!(failure.diagnostics[0].code, "T0005");
}

#[test]
fn rejects_defaults_incompatible_with_parameter_types() {
    let failure = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction connect; timeout bool = 2\n",
        )],
    ))
    .unwrap_err();

    assert_eq!(failure.diagnostics[0].code, "T0006");
}

#[test]
fn accepts_reads_after_unconditional_assignment() {
    analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction main\n  value int\n  value = 1\n  result = value\n",
        )],
    ))
    .unwrap();
}

#[test]
fn rejects_reads_before_assignment() {
    let failure = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction main\n  value int\n  result = value\n",
        )],
    ))
    .unwrap_err();

    assert_eq!(failure.diagnostics[0].code, "T0007");
}

#[test]
fn branch_assignment_is_definite_only_when_every_path_assigns() {
    analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "function main; ready bool\n",
                "  value int\n",
                "  if ready\n",
                "    value = 1\n",
                "  else\n",
                "    value = 2\n",
                "  result = value\n",
            ),
        )],
    ))
    .unwrap();

    let failure = analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "function main; ready bool\n",
                "  value int\n",
                "  if ready\n",
                "    value = 1\n",
                "  result = value\n",
            ),
        )],
    ))
    .unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "T0007");
}

#[test]
fn collection_for_targets_are_typed_only_inside_the_loop_body() {
    let wrong_argument = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction consume; item int\nfunction main\n  text string = 'ab'\n  for value in text\n    consume; value\n",
        )],
    ))
    .unwrap_err();
    assert_eq!(wrong_argument.diagnostics[0].code, "T0012");

    let wrong_collection = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction main\n  value int = 1\n  for value in value\n    item = value\n",
        )],
    ))
    .unwrap_err();
    assert_eq!(wrong_collection.diagnostics[0].code, "T0016");
}

#[test]
fn rejects_implicit_cross_type_reassignment() {
    let failure = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction main\n  value int = 1\n  value = true\n",
        )],
    ))
    .unwrap_err();

    assert_eq!(failure.diagnostics[0].code, "T0002");
    assert_eq!(
        failure.diagnostics[0].message,
        "cannot assign `bool` to `value` of type `int`"
    );
}

#[test]
fn integer_literals_may_assign_only_when_representable() {
    analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction main\n  value int8\n  value = 127\n",
        )],
    ))
    .unwrap();

    let failure = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction main\n  value int8\n  value = 128\n",
        )],
    ))
    .unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "T0003");
}

#[test]
fn types_canonical_integer_coercion_family() {
    let analyzed = analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "from /core types import .int8, .int16, .uint8\n",
                "int8 = .int8\n",
                "int16 = .int16\n",
                "uint8 = .uint8\n",
                "function main\n",
                "  value int = 300\n",
                "  exact = value.coerce; int16\n",
                "  checked = value.coerce.checked; int8\n",
                "  wrapped = value.coerce.wrap; uint8\n",
                "  saturated = value.coerce.saturate; uint8\n",
            ),
        )],
    ))
    .unwrap();
    let bindings = &analyzed.units[0].typed_bindings;
    let type_of = |name| {
        bindings
            .iter()
            .find(|binding| binding.name == name)
            .unwrap()
            .value_type
    };

    assert_eq!(type_of("exact"), ValueType::Scalar(ScalarType::Int16));
    assert_eq!(
        type_of("checked"),
        ValueType::ScalarOrNone(ScalarType::Int8)
    );
    assert_eq!(type_of("wrapped"), ValueType::Scalar(ScalarType::Uint8));
    assert_eq!(type_of("saturated"), ValueType::Scalar(ScalarType::Uint8));
}

#[test]
fn rejects_unsupported_integer_coercion_destinations() {
    let failure = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction main\n  value int = 1\n  converted = value.coerce; float\n",
        )],
    ))
    .unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "T0008");

    for expression in ["value.coerce.wrap; int", "value.coerce.checked; int"] {
        let failure = analyze(&package(
            true,
            &[(
                "main.trn",
                &format!(
                    "namespace app\nfunction main\n  value int = 1\n  converted = {expression}\n"
                ),
            )],
        ))
        .unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "T0010");
    }
}

#[test]
fn rejects_obsolete_flat_integer_coercion_members() {
    for member in ["checked-coerce", "wrapping-coerce", "saturating-coerce"] {
        let failure = analyze(&package(
            true,
            &[(
                "main.trn",
                &format!(
                    "namespace app\nfunction main\n  value int = 1\n  converted = value.{member}; int\n"
                ),
            )],
        ))
        .unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "T0017");
    }
}

#[test]
fn rejects_unbound_integer_coercion_family() {
    let failure = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction main\n  value int = 1\n  family = value.coerce\n",
        )],
    ))
    .unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "T0018");
}

#[test]
fn rejects_nested_and_escaped_coercion_family_shapes() {
    for expression in [
        "value.coerce.clamp; int8",
        "value.coerce.wrap.checked; int8",
        "value.coerce.checked.wrap; int8",
        "value.coerce.clamp.wrap; int8",
    ] {
        let failure = analyze(&package(
            true,
            &[(
                "main.trn",
                &format!(
                    "namespace app\nfrom /core types import .int8\nint8 = .int8\nfunction main\n  value int = 1\n  converted = {expression}\n"
                ),
            )],
        ))
        .unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "T0010");
        assert_eq!(
            failure.diagnostics[0].message,
            format!(
                "`{}` is not an available coercion policy",
                expression
                    .split_once("; ")
                    .map_or(expression, |(callee, _)| callee)
                    .trim_start_matches("value")
            )
        );
    }

    for expression in ["value.coerce.checked", "value.coerce.wrap + 1"] {
        let failure = analyze(&package(
            true,
            &[(
                "main.trn",
                &format!(
                    "namespace app\nfunction main\n  value int = 1\n  converted = {expression}\n"
                ),
            )],
        ))
        .unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "T0018");
    }
}

#[test]
fn validates_calls_inside_coercion_receivers() {
    for arguments in ["'text'", "1, 2"] {
        let failure = analyze(&package(
            true,
            &[(
                "main.trn",
                &format!(
                    "namespace app\nfrom /core types import .int8\nint8 = .int8\nfunction observed int; item int\n  return item\nfunction main\n  converted = (observed; {arguments}).coerce; int8\n"
                ),
            )],
        ))
        .unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "T0012");
    }
}

#[test]
fn infers_cross_unit_call_results_before_binding_types() {
    let analyzed = analyze(&package(
        true,
        &[
            (
                "helper.trn",
                "namespace app\nfunction helper int\n  return 300\n",
            ),
            (
                "main.trn",
                "namespace app\nfrom /core types import .uint8\nuint8 = .uint8\nfunction main\n  value = (helper;).coerce.wrap; uint8\n",
            ),
        ],
    ))
    .unwrap();
    assert_eq!(
        analyzed.units[1]
            .typed_bindings
            .iter()
            .find(|binding| binding.name == "value")
            .unwrap()
            .value_type,
        ValueType::Scalar(ScalarType::Uint8)
    );
}

#[test]
fn function_parameters_are_in_scope_during_binding_analysis() {
    analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "from /core types import .int8\n",
                "tiny = .int8\n",
                "function convert int8; item int\n",
                "  result int8\n",
                "  result = item.coerce; tiny\n",
                "  return result\n",
            ),
        )],
    ))
    .unwrap();
}

#[test]
fn types_valid_unary_binary_and_comparison_operators() {
    let analyzed = analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "function main\n",
                "  count = 5\n",
                "  negative = -count\n",
                "  mask = ~count\n",
                "  sum = count + 2\n",
                "  shifted = count << 1\n",
                "  ordered = count >= 2\n",
                "  selected = true and false\n",
            ),
        )],
    ))
    .unwrap();
    let bindings = &analyzed.units[0].typed_bindings;
    for name in ["negative", "mask", "sum", "shifted"] {
        assert_eq!(
            bindings
                .iter()
                .find(|binding| binding.name == name)
                .unwrap()
                .value_type,
            ValueType::Scalar(ScalarType::Int)
        );
    }
    for name in ["ordered", "selected"] {
        assert_eq!(
            bindings
                .iter()
                .find(|binding| binding.name == name)
                .unwrap()
                .value_type,
            ValueType::Scalar(ScalarType::Bool)
        );
    }
}

#[test]
fn string_length_has_integer_type_and_rejects_other_receivers() {
    let analyzed = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction main\n  text = 'Terrane'\n  size = text.length\n",
        )],
    ))
    .unwrap();
    assert_eq!(
        analyzed.units[0]
            .typed_bindings
            .iter()
            .find(|binding| binding.name == "size")
            .unwrap()
            .value_type,
        ValueType::Scalar(ScalarType::Int)
    );

    let failure = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction main\n  count = 1\n  size = count.length\n",
        )],
    ))
    .unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "T0013");
    assert_eq!(
        failure.diagnostics[0].message,
        "`.length` requires `string`, found `int`"
    );
}

#[test]
fn type_membership_is_boolean_in_bindings_and_conditions() {
    let analyzed = analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "function main\n",
                "  value = 1\n",
                "  selected = value is a int\n",
                "  if value is a int\n",
                "    return\n",
            ),
        )],
    ))
    .unwrap();
    assert_eq!(
        analyzed.units[0]
            .typed_bindings
            .iter()
            .find(|binding| binding.name == "selected")
            .unwrap()
            .value_type,
        ValueType::Scalar(ScalarType::Bool)
    );
}

#[test]
fn identity_accepts_typed_scalars_and_canonical_descriptors() {
    let analyzed = analyze(&package(
        false,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "from /core types import .int8\n",
                "function produce\n",
                "byte = .int8\n",
                "same-type = byte is byte\n",
                "value = 1\n",
                "same-value = value is value\n",
                "same-result = (produce;) is (produce;)\n",
            ),
        )],
    ))
    .unwrap();
    for name in ["same-type", "same-value", "same-result"] {
        assert_eq!(
            analyzed.units[0]
                .typed_bindings
                .iter()
                .find(|binding| binding.name == name)
                .unwrap()
                .value_type,
            ValueType::Scalar(ScalarType::Bool)
        );
    }
}

#[test]
fn rejects_operators_with_incompatible_scalar_operands() {
    for source in [
        "namespace app\nfunction main\n  value = true + false\n",
        "namespace app\nfunction main\n  value = ~'text'\n",
        "namespace app\nfunction main\n  value = 1 and 2\n",
        "namespace app\nfunction main\n  value = 1 == true\n",
    ] {
        let failure = analyze(&package(true, &[("main.trn", source)])).unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "T0011");
    }
}

#[test]
fn binds_positional_named_and_default_arguments() {
    analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "function render; title string, count int, enabled bool = true\n",
                "function main\n",
                "  render; 'items', enabled=false, count=3\n",
                "  render; 'empty', 0\n",
            ),
        )],
    ))
    .unwrap();
}

#[test]
fn rejects_invalid_function_argument_binding() {
    let cases = [
        "render; enabled=true, 'items', count=3",
        "render; 'items', count=3, count=4",
        "render; 'items', missing=3",
        "render; 'items'",
        "render; 'items', 3, true, false",
    ];
    for call in cases {
        let source = format!(
            "namespace app\nfunction render; title string, count int, enabled bool = true\nfunction main\n  {call}\n"
        );
        let failure = analyze(&package(true, &[("main.trn", &source)])).unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "T0012", "{call}");
    }
}

#[test]
fn rejects_statically_incompatible_typed_arguments() {
    for call in ["consume; true", "consume; value"] {
        let source = format!(
            "namespace app\nfunction consume; item int\nfunction main\n  value = 'text'\n  {call}\n"
        );
        let failure = analyze(&package(true, &[("main.trn", &source)])).unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "T0012", "{call}");
        assert!(
            failure.diagnostics[0].message.contains("expected `int`"),
            "{call}"
        );
    }
}

#[test]
fn rejects_function_results_assigned_to_incompatible_scalar_types() {
    let failure = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction enabled bool\n  return true\nfunction main\n  count int = enabled;\n",
        )],
    ))
    .unwrap_err();

    assert_eq!(failure.diagnostics[0].code, "T0002");
    assert_eq!(
        failure.diagnostics[0].message,
        "cannot assign `bool` to `count` of type `int`"
    );
}

#[test]
fn typed_call_checks_follow_callee_and_argument_scope() {
    let parameter_failure = analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction consume; item int\nfunction main; value bool\n  consume; value\n",
        )],
    ))
    .unwrap_err();
    assert_eq!(parameter_failure.diagnostics[0].code, "T0012");

    analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "value = 1\n",
                "function consume; item int\n",
                "function main\n",
                "  consume = false\n",
                "  consume; value\n",
            ),
        )],
    ))
    .unwrap();
}

#[test]
fn typed_call_checks_cross_source_unit_contracts() {
    let failure = analyze(&package(
        true,
        &[
            ("api.trn", "namespace app\nfunction consume; item int\n"),
            (
                "main.trn",
                "namespace app\nfunction main\n  consume; 'text'\n",
            ),
        ],
    ))
    .unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "T0012");
}

#[test]
fn preserves_calls_member_access_and_dot_objects_as_distinct_forms() {
    let analyzed = analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "from /core output import .print as .renderer\n",
                "function consume; item\n",
                "function main\n",
                "  text = 'hello'\n",
                "  text.clear;\n",
                "  consume; .renderer\n",
                "  .renderer;\n",
            ),
        )],
    ))
    .unwrap();
    let root = &analyzed.units[0].tree.root;
    assert!(contains_kind(root, SyntaxKind::MemberExpression));
    assert!(contains_kind(root, SyntaxKind::ObjectName));
    assert_eq!(
        count_kind(root, SyntaxKind::CallExpression),
        3,
        "member, ordinary, and dot-object calls remain explicit call nodes"
    );
}

#[test]
fn checks_control_flow_and_records_unreachable_statements() {
    let analyzed = analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "function choose bool; ready bool\n",
                "  if ready\n",
                "    return true\n",
                "  else\n",
                "    return false\n",
                "  unreachable = 1\n",
                "function count\n",
                "  value int = 1\n",
                "  while true\n",
                "    value++\n",
                "    break\n",
            ),
        )],
    ))
    .unwrap();
    assert_eq!(analyzed.units[0].unreachable_spans.len(), 1);
}

#[test]
fn rejects_invalid_control_flow_contracts() {
    for (source, code) in [
        (
            "namespace app\nfunction choose bool; ready bool\n  if ready\n    return true\n",
            "T0015",
        ),
        (
            "namespace app\nfunction main\n  if 1\n    return\n",
            "T0014",
        ),
        ("namespace app\nfunction main\n  break\n", "T0014"),
        (
            "namespace app\nfunction main\n  value string = 'x'\n  value++\n",
            "T0014",
        ),
        ("namespace app\nfunction choose bool\n  return 1\n", "T0015"),
    ] {
        let failure = analyze(&package(true, &[("main.trn", source)])).unwrap_err();
        assert_eq!(failure.diagnostics[0].code, code, "{source}");
    }
}

#[test]
fn limits_collection_iteration_to_single_target_strings() {
    analyze(&package(
        true,
        &[(
            "main.trn",
            "namespace app\nfunction main\n  text string = 'abc'\n  for character in text\n    continue\n",
        )],
    ))
    .unwrap();
    for source in [
        "namespace app\nfunction main\n  count int = 3\n  for value in count\n    continue\n",
        "namespace app\nfunction main\n  text string = 'abc'\n  for index, character in text\n    continue\n",
    ] {
        let failure = analyze(&package(true, &[("main.trn", source)])).unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "T0016");
    }
}

#[test]
fn records_left_to_right_calls_and_short_circuit_boundaries() {
    let analyzed = analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "function first bool\n",
                "  return true\n",
                "function second bool\n",
                "  return false\n",
                "function main\n",
                "  first;\n",
                "  second;\n",
                "  ready bool = true and false\n",
            ),
        )],
    ))
    .unwrap();
    let unit = &analyzed.units[0];
    let calls = unit
        .evaluation_steps
        .iter()
        .filter(|step| step.kind == EvaluationKind::Call)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert!(unit.source.text()[calls[0].span.start..calls[0].span.end].contains("first"));
    assert!(!calls[0].conditional);
    assert!(unit.source.text()[calls[1].span.start..calls[1].span.end].contains("second"));
    assert!(!calls[1].conditional);
    let boundary = unit
        .evaluation_steps
        .iter()
        .find(|step| step.kind == EvaluationKind::ShortCircuitRhs)
        .unwrap();
    assert_eq!(
        &unit.source.text()[boundary.span.start..boundary.span.end],
        "false"
    );
    assert!(boundary.conditional);
}

#[test]
fn records_mutability_against_resolved_binding_identity() {
    let analyzed = analyze(&package(
        true,
        &[(
            "main.trn",
            concat!(
                "namespace app\n",
                "value = 1\n",
                "function main\n",
                "  value int = 2\n",
                "  value = 3\n",
            ),
        )],
    ))
    .unwrap();
    let values = analyzed.units[0]
        .typed_bindings
        .iter()
        .filter(|binding| binding.name == "value")
        .map(|binding| binding.mutable)
        .collect::<Vec<_>>();

    assert_eq!(values, [false, true]);
}

fn contains_kind(node: &terrane_compiler::syntax::SyntaxNode, kind: SyntaxKind) -> bool {
    node.kind == kind || node.children.iter().any(|child| contains_kind(child, kind))
}

fn count_kind(node: &terrane_compiler::syntax::SyntaxNode, kind: SyntaxKind) -> usize {
    usize::from(node.kind == kind)
        + node
            .children
            .iter()
            .map(|child| count_kind(child, kind))
            .sum::<usize>()
}
