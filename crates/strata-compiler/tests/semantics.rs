use std::path::PathBuf;

use strata_compiler::{Package, SourceFile, SourceUnit, analyze};

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
                source: SourceFile::new(id as u32, PathBuf::from(path), (*text).to_owned()),
            })
            .collect(),
    }
}

#[test]
fn assembles_namespaces_symmetrically_before_import_resolution() {
    let analyzed = analyze(&package(
        false,
        &[
            (
                "consumer.strata",
                "namespace app\nfrom /shared import .item\n",
            ),
            ("second.strata", "namespace shared\n.thing = 2\n"),
            ("first.strata", "namespace shared\n.item = 1\n"),
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
fn resolves_exact_root_and_parent_namespace_anchors() {
    let analyzed = analyze(&package(
        false,
        &[
            ("exports.strata", "namespace parent shared\n.item = 1\n"),
            (
                "root.strata",
                "namespace root\nfrom /parent shared import .item as .root-item\n",
            ),
            (
                "child.strata",
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
            "main.strata",
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
            "main.strata",
            "namespace app\nfrom /core output import .print\nfrom /core output import .print\n",
        )],
    ));
    assert!(accepted.is_ok());

    let rejected = analyze(&package(
        false,
        &[
            ("one.strata", "namespace one\n.item = 1\n"),
            ("two.strata", "namespace two\n.item = 2\n"),
            (
                "main.strata",
                "namespace app\nfrom /one import .item\nfrom /two import .item\n",
            ),
        ],
    ))
    .unwrap_err();
    assert_eq!(rejected.diagnostics[0].code, "S2011");
}

#[test]
fn prelude_has_exact_ordinary_bindings_and_can_be_disabled() {
    let enabled = analyze(&package(true, &[("main.strata", "namespace app\n")])).unwrap();
    let names = enabled.prelude_bindings.keys().cloned().collect::<Vec<_>>();
    assert_eq!(
        names,
        ["bool", "bytes", "float", "int", "none", "print", "string"]
    );

    let disabled = analyze(&package(false, &[("main.strata", "namespace app\n")])).unwrap();
    assert!(disabled.prelude_bindings.is_empty());
}

#[test]
fn bootstrap_registry_contains_versioned_modules_and_fixed_width_types() {
    let analyzed = analyze(&package(false, &[("main.strata", "namespace app\n")])).unwrap();

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
            "main.strata",
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
            ("one.strata", "namespace app\nvalue = 1\n"),
            ("two.strata", "namespace app\nvalue = 2\n"),
        ],
    ))
    .unwrap_err();
    assert_eq!(duplicate.diagnostics[0].code, "S2005");

    let private = analyze(&package(
        false,
        &[
            ("exports.strata", "namespace hidden\nprivate .secret = 1\n"),
            (
                "consumer.strata",
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
            (
                "one.strata",
                "namespace first\nglobal shared = 1\nlocal = 1\n",
            ),
            (
                "two.strata",
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
fn ordinary_lookup_uses_namespace_global_and_prelude_tiers() {
    let analyzed = analyze(&package(
        true,
        &[
            (
                "parent.strata",
                "namespace app\nparent-value = 1\nprotected inherited = 1\nprivate hidden = 1\n",
            ),
            (
                "child.strata",
                "namespace app child\nparent-value = 2\nglobal shared = 1\n",
            ),
            ("peer.strata", "namespace peer\n"),
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
