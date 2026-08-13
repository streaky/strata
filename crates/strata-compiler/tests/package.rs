use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use strata_compiler::{IMPLICIT_PACKAGE_ID, Package, compile_package, semantics::analyze};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempPackage(std::path::PathBuf);

impl TempPackage {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("strata-package-{}-{serial}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn implicit_source_has_stable_package_contract() {
    let package = Package::implicit("examples/hello.strata", "namespace hello\n".to_owned());

    assert_eq!(package.identity, IMPLICIT_PACKAGE_ID);
    assert!(package.prelude);
    assert_eq!(package.root, Path::new("examples"));
    assert_eq!(package.units.len(), 1);
    assert_eq!(package.units[0].relative_path, Path::new("hello.strata"));
    assert_eq!(package.units[0].source.id(), 0);
}

#[test]
fn manifest_enumerates_sources_in_deterministic_path_order() {
    let package = TempPackage::new();
    package.write(
        "strata.package",
        "# complete source set\npackage example.tools\nprelude false\nsource zed.strata\nsource nested/alpha.strata\n",
    );
    package.write("zed.strata", "namespace zed\n");
    package.write("nested/alpha.strata", "namespace alpha\n");

    let loaded = Package::load(&package.0).unwrap();

    assert_eq!(loaded.identity, "example.tools");
    assert!(!loaded.prelude);
    assert_eq!(
        loaded
            .units
            .iter()
            .map(|unit| unit.relative_path.as_path())
            .collect::<Vec<_>>(),
        [Path::new("nested/alpha.strata"), Path::new("zed.strata")]
    );
    assert_eq!(loaded.units[0].source.id(), 0);
    assert_eq!(loaded.units[1].source.id(), 1);
}

#[test]
fn package_compilation_parses_every_enumerated_unit() {
    let package = TempPackage::new();
    package.write(
        "strata.package",
        "package example.multi\nsource support.strata\nsource main.strata\n",
    );
    package.write("support.strata", "namespace hello helpers\nvalue = 1\n");
    package.write(
        "main.strata",
        "namespace hello\nfrom /core output import .print\nprint = .print\nfunction main\n  print; >package pipeline\n",
    );

    let loaded = Package::load(&package.0).unwrap();
    let compilation = compile_package(&loaded).unwrap();

    assert_eq!(compilation.program.namespace, "hello");
    assert_eq!(compilation.program.message, "package pipeline");
}

#[test]
fn syntax_failure_in_non_main_unit_stops_package_compilation() {
    let package = TempPackage::new();
    package.write(
        "strata.package",
        "package example.invalid\nsource main.strata\nsource support.strata\n",
    );
    package.write(
        "main.strata",
        "namespace hello\nfrom /core output import .print\nprint = .print\nfunction main\n  print; >unreachable\n",
    );
    package.write("support.strata", "value =\n");

    let failure = compile_package(&Package::load(&package.0).unwrap()).unwrap_err();

    assert!(failure.source.path().ends_with("support.strata"));
    assert_eq!(failure.diagnostics[0].code, "S1019");
}

#[test]
fn malformed_manifests_report_all_manifest_errors() {
    let package = TempPackage::new();
    package.write(
        "strata.package",
        "prelude perhaps\nsource ../escape.strata\nsource repeated.strata\nsource repeated.strata\nunknown field\n",
    );

    let errors = Package::load(&package.0).unwrap_err();
    let messages = errors
        .iter()
        .map(|error| error.message.as_str())
        .collect::<Vec<_>>();

    assert!(messages.iter().any(|message| message.contains("prelude")));
    assert!(
        messages
            .iter()
            .any(|message| message.contains("relative `.strata`"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("duplicate source"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("unknown manifest field"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("missing `package`"))
    );
}

#[test]
fn manifest_package_drives_complete_namespace_and_scope_resolution() {
    let package = TempPackage::new();
    package.write(
        "strata.package",
        concat!(
            "package namespace-contract\n",
            "prelude false\n",
            "source consumer.strata\n",
            "source exports.strata\n",
            "source parent.strata\n",
        ),
    );
    package.write("exports.strata", "namespace shared\npublic .item = 1\n");
    package.write(
        "parent.strata",
        "namespace app support\npublic .parent = 1\n",
    );
    package.write(
        "consumer.strata",
        concat!(
            "namespace app child\n",
            "from /shared import .item\n",
            "from .. support import .parent\n",
            "function run; argument int\n",
            "  from /core output import .print as .local-print\n",
            "  value = argument\n",
        ),
    );

    let loaded = Package::load(&package.0).unwrap();
    let analyzed = analyze(&loaded).unwrap();
    let consumer = analyzed
        .units
        .iter()
        .find(|unit| unit.namespace == "/app/child")
        .unwrap();
    let body_offset = consumer.source.text().find("value =").unwrap();

    assert_eq!(analyzed.identity, "namespace-contract");
    assert!(!analyzed.prelude);
    assert!(analyzed.object("/app/child", "item").is_some());
    assert!(analyzed.object("/app/child", "parent").is_some());
    assert!(
        analyzed
            .resolve_ordinary_at(consumer, body_offset, "argument")
            .is_some()
    );
    assert!(
        analyzed
            .resolve_object_at(consumer, body_offset, "local-print")
            .is_some()
    );
}

#[test]
fn missing_enumerated_sources_are_package_errors() {
    let package = TempPackage::new();
    package.write(
        "strata.package",
        "package missing-source\nsource absent.strata\n",
    );

    let errors = Package::load(package.0.join("strata.package")).unwrap_err();

    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("cannot read package source"));
}
