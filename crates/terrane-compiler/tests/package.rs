use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use terrane_compiler::{IMPLICIT_PACKAGE_ID, Package, analyze, compile_package};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempPackage(std::path::PathBuf);

impl TempPackage {
    fn new() -> Self {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("terrane-package-{}-{serial}", std::process::id()));
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
    let package = Package::implicit("examples/hello.trn", "namespace hello\n".to_owned());

    assert_eq!(package.identity, IMPLICIT_PACKAGE_ID);
    assert!(package.prelude);
    assert_eq!(package.root, Path::new("examples"));
    assert_eq!(package.units.len(), 1);
    assert_eq!(package.units[0].relative_path, Path::new("hello.trn"));
    assert_eq!(package.units[0].source.id(), 0);
}

#[test]
fn bare_implicit_source_uses_current_directory_as_root() {
    let package = Package::implicit("hello.trn", "namespace hello\n".to_owned());

    assert_eq!(package.root, Path::new("."));
    assert_eq!(package.units[0].relative_path, Path::new("hello.trn"));
}

#[test]
fn manifest_enumerates_sources_in_deterministic_path_order() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "# complete source set\npackage = \"example.tools\"\nprelude = false\nsources = [\"zed.trn\", \"nested/alpha.trn\"]\n",
    );
    package.write("zed.trn", "namespace zed\n");
    package.write("nested/alpha.trn", "namespace alpha\n");

    let loaded = Package::load(&package.0).unwrap();

    assert_eq!(loaded.identity, "example.tools");
    assert!(!loaded.prelude);
    assert_eq!(
        loaded
            .units
            .iter()
            .map(|unit| unit.relative_path.as_path())
            .collect::<Vec<_>>(),
        [Path::new("nested/alpha.trn"), Path::new("zed.trn")]
    );
    assert_eq!(loaded.units[0].source.id(), 0);
    assert_eq!(loaded.units[1].source.id(), 1);
}

#[test]
fn package_compilation_parses_every_enumerated_unit() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"example.multi\"\nsources = [\"support.trn\", \"main.trn\"]\n",
    );
    package.write("support.trn", "namespace hello helpers\nvalue = 1\n");
    package.write(
        "main.trn",
        "namespace hello\nfrom /core output import .print\nprint = .print\nfunction main\n  print; >package pipeline\n",
    );

    let loaded = Package::load(&package.0).unwrap();
    let compilation = compile_package(&loaded).unwrap();

    assert!(compilation.rust.contains("// Namespace: hello\n"));
    assert!(
        compilation
            .rust
            .contains("println!(\"{}\", terrane_scalar_support::scalar_text(&(String::from(\"package pipeline\"))));")
    );
}

#[test]
fn package_compilation_emits_functions_and_bindings_from_every_unit() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"example.multi\"\nsources = [\"main.trn\", \"support.trn\"]\n",
    );
    package.write(
        "main.trn",
        "namespace hello\nfrom /core output import .print\nprint = .print\nfunction main\n  print; (helper;)\n",
    );
    package.write(
        "support.trn",
        "namespace hello\nvalue int = 41\nfunction helper int\n  return value + 1\n",
    );

    let compilation = compile_package(&Package::load(&package.0).unwrap()).unwrap();

    assert!(
        compilation
            .rust
            .contains("static __TERRANE_F1_VALUE: i128 = 41;")
    );
    assert!(compilation.rust.contains("fn helper() -> i128"));
    assert!(
        compilation
            .rust
            .contains("return (__TERRANE_F1_VALUE + 1);")
    );
}

#[test]
fn package_entry_point_comes_from_resolved_function_declarations() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"example.entry\"\nsources = [\"decoy.trn\", \"main.trn\"]\n",
    );
    package.write("decoy.trn", "namespace decoy\ntext = >>\n  function main\n");
    package.write(
        "main.trn",
        "namespace actual\nfrom /core output import .print\nprint = .print\nfunction main\n  print; >real entry\n",
    );

    let compilation = compile_package(&Package::load(&package.0).unwrap()).unwrap();

    assert!(compilation.rust.contains("// Namespace: actual\n"));
    assert!(compilation.rust.contains(
        "println!(\"{}\", terrane_scalar_support::scalar_text(&(String::from(\"real entry\"))));"
    ));
}

#[test]
fn package_requires_one_unambiguous_main_function() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"example.entry\"\nsources = [\"first.trn\", \"second.trn\"]\n",
    );
    package.write("first.trn", "namespace first\nvalue = 1\n");
    package.write("second.trn", "namespace second\nvalue = 2\n");

    let missing = compile_package(&Package::load(&package.0).unwrap()).unwrap_err();
    assert_eq!(missing.diagnostics[0].code, "S2015");

    package.write("first.trn", "namespace first\nfunction main\n");
    package.write("second.trn", "namespace second\nfunction main\n");
    let ambiguous = compile_package(&Package::load(&package.0).unwrap()).unwrap_err();
    assert_eq!(ambiguous.diagnostics[0].code, "S2016");
}

#[test]
fn syntax_failure_in_non_main_unit_stops_package_compilation() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"example.invalid\"\nsources = [\"main.trn\", \"support.trn\"]\n",
    );
    package.write(
        "main.trn",
        "namespace hello\nfrom /core output import .print\nprint = .print\nfunction main\n  print; >unreachable\n",
    );
    package.write("support.trn", "value =\n");

    let failure = compile_package(&Package::load(&package.0).unwrap()).unwrap_err();

    assert!(failure.source.path().ends_with("support.trn"));
    assert_eq!(failure.diagnostics[0].code, "S1019");
}

#[test]
fn malformed_manifests_report_all_manifest_errors() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "prelude = \"perhaps\"\nsources = [\"../escape.trn\", \"repeated.trn\", \"repeated.trn\"]\nunknown = \"field\"\n",
    );

    let errors = Package::load(&package.0).unwrap_err();
    let messages = errors
        .iter()
        .map(|error| error.diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert!(messages.iter().any(|message| message.contains("prelude")));
    assert!(
        messages
            .iter()
            .any(|message| message.contains("relative `.trn`"))
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
        "package.toml",
        concat!(
            "package = \"namespace-contract\"\n",
            "prelude = false\n",
            "sources = [\"consumer.trn\", \"exports.trn\", \"parent.trn\"]\n",
        ),
    );
    package.write("exports.trn", "namespace shared\npublic .item = 1\n");
    package.write("parent.trn", "namespace app support\npublic .parent = 1\n");
    package.write(
        "consumer.trn",
        concat!(
            "namespace app child\n",
            "from /core types import .int\n",
            "int = .int\n",
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
        "package.toml",
        "package = \"missing-source\"\nsources = [\"absent.trn\"]\n",
    );

    let errors = Package::load(package.0.join("package.toml")).unwrap_err();

    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .diagnostic
            .message
            .contains("cannot read package source")
    );
}
