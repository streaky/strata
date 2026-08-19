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
fn manifest_discovers_sources_in_deterministic_path_order() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "# discovered source set\npackage = \"example.tools\"\nprelude = false\n[namespaces]\nzed = \"zed\"\nalpha = \"nested\"\n",
    );
    package.write("zed/zed.trn", "namespace zed\n");
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
        [Path::new("nested/alpha.trn"), Path::new("zed/zed.trn")]
    );
    assert_eq!(loaded.units[0].source.id(), 0);
    assert_eq!(loaded.units[1].source.id(), 1);
}

#[test]
fn package_compilation_parses_every_discovered_unit() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"example.multi\"\n[namespaces]\nhello = \"hello\"\n\"hello/helpers\" = \"hello/helpers\"\n",
    );
    package.write(
        "hello/helpers/support.trn",
        "namespace hello/helpers\nconstant value = 1\n",
    );
    package.write(
        "hello/main.trn",
        "namespace hello\nfrom /core/output import print\nfunction main\n  print; >package pipeline\n",
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
        "package = \"example.multi\"\n[namespaces]\nhello = \"src\"\n",
    );
    package.write(
        "src/main.trn",
        "namespace hello\nfrom /core/output import print\nfunction main\n  print; (helper;)\n",
    );
    package.write(
        "src/support.trn",
        "namespace hello\nconstant value int = 41\nfunction helper int\n  return value + 1\n",
    );

    let compilation = compile_package(&Package::load(&package.0).unwrap()).unwrap();

    assert!(
        compilation
            .rust
            .contains("static __TERRANE_F1_VALUE: std::sync::LazyLock<terrane_int_support::Int>")
    );
    assert!(
        compilation
            .rust
            .contains("fn helper() -> terrane_int_support::Int")
    );
    assert!(compilation.rust.contains(
        "return (*__TERRANE_F1_VALUE).clone() + terrane_int_support::Int::from(1_i128);"
    ));
}

#[test]
fn package_entry_point_comes_from_resolved_function_declarations() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"example.entry\"\n[namespaces]\ndecoy = \"decoy\"\nactual = \"actual\"\n",
    );
    package.write(
        "decoy/decoy.trn",
        "namespace decoy\nconstant text = >>\n  function main\n",
    );
    package.write(
        "actual/main.trn",
        "namespace actual\nfrom /core/output import print\nfunction main\n  print; >real entry\n",
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
        "package = \"example.entry\"\n[namespaces]\nfirst = \"first\"\nsecond = \"second\"\n",
    );
    package.write("first/first.trn", "namespace first\nconstant value = 1\n");
    package.write(
        "second/second.trn",
        "namespace second\nconstant value = 2\n",
    );

    let missing = compile_package(&Package::load(&package.0).unwrap()).unwrap_err();
    assert_eq!(missing.diagnostics[0].code, "S2015");

    package.write("first/first.trn", "namespace first\nfunction main\n");
    package.write("second/second.trn", "namespace second\nfunction main\n");
    let ambiguous = compile_package(&Package::load(&package.0).unwrap()).unwrap_err();
    assert_eq!(ambiguous.diagnostics[0].code, "S2016");
}

#[test]
fn syntax_failure_in_non_main_unit_stops_package_compilation() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"example.invalid\"\n[namespaces]\nhello = \"src\"\n",
    );
    package.write(
        "src/main.trn",
        "namespace hello\nfrom /core/output import print\nfunction main\n  print; >unreachable\n",
    );
    package.write("src/support.trn", "namespace hello\nvalue =\n");

    let failure = compile_package(&Package::load(&package.0).unwrap()).unwrap_err();

    assert!(failure.source.path().ends_with("support.trn"));
    assert_eq!(failure.diagnostics[0].code, "S1019");
}

#[test]
fn malformed_manifests_report_all_manifest_errors() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "prelude = \"perhaps\"\nmystery = \"field\"\n[namespaces]\n\"Bad Root\" = \"../escape\"\nvalid = \"../escape\"\n",
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
            .any(|message| message.contains("must match `[a-z]"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("relative path"))
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
            "[namespaces]\n",
            "shared = \"shared\"\n",
            "\"app/support\" = \"app/support\"\n",
            "\"app/child\" = \"app/child\"\n",
        ),
    );
    package.write(
        "shared/exports.trn",
        "namespace shared\npublic constant item = 1\n",
    );
    package.write(
        "app/support/parent.trn",
        "namespace app/support\npublic constant parent = 1\n",
    );
    package.write(
        "app/child/consumer.trn",
        concat!(
            "namespace app/child\n",
            "from /core/types import int\n",
            "from /shared import item\n",
            "from ../support import parent\n",
            "function run; argument int\n",
            "  from /core/output import print as local-print\n",
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
    assert!(analyzed.symbol("/app/child", "item").is_some());
    assert!(analyzed.symbol("/app/child", "parent").is_some());
    assert!(
        analyzed
            .resolve_name_at(consumer, body_offset, "argument")
            .is_some()
    );
    assert!(
        analyzed
            .resolve_name_at(consumer, body_offset, "local-print")
            .is_some()
    );
}

#[test]
fn missing_namespace_directories_are_package_errors() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"missing-source\"\n[namespaces]\nabsent = \"absent\"\n",
    );

    let errors = Package::load(package.0.join("package.toml")).unwrap_err();

    assert!(!errors.is_empty());
    assert!(
        errors[0]
            .diagnostic
            .message
            .contains("cannot read namespace directory")
    );
}

#[test]
fn empty_namespace_roots_are_package_errors() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"empty-root\"\n[namespaces]\napp = \"app\"\ntools = \"tools\"\n",
    );
    package.write("app/main.trn", "namespace app\n");
    fs::create_dir_all(package.0.join("tools")).unwrap();

    let errors = Package::load(package.0.join("package.toml")).unwrap_err();

    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .diagnostic
            .message
            .contains("namespace root `/tools` contains no `.trn` source files")
    );
}

#[test]
fn namespace_mappings_discover_sources_in_sorted_order() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"mapped\"\nprelude = false\n[namespaces]\napp = \"src\"\n\"app/private\" = \"src/http/private\"\n",
    );
    package.write("src/zeta.trn", "namespace app\n");
    package.write("src/http/beta.trn", "namespace app/http\n");
    package.write("src/http/alpha.trn", "namespace app/http\n");
    package.write("src/http/private/internal.trn", "namespace app/private\n");
    package.write("outside.trn", "namespace ignored\n");

    let loaded = Package::load(&package.0).unwrap();
    assert_eq!(
        loaded
            .units
            .iter()
            .map(|unit| unit.relative_path.as_path())
            .collect::<Vec<_>>(),
        [
            Path::new("src/http/alpha.trn"),
            Path::new("src/http/beta.trn"),
            Path::new("src/http/private/internal.trn"),
            Path::new("src/zeta.trn"),
        ]
    );
    assert_eq!(
        loaded
            .units
            .iter()
            .map(|unit| unit.expected_namespace.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["/app/http", "/app/http", "/app/private", "/app"]
    );
}

#[test]
fn namespace_mappings_reject_duplicate_directories() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"mapped\"\n[namespaces]\napp = \"src\"\ntools = \"src\"\n",
    );

    let errors = Package::load(&package.0).unwrap_err();
    assert!(
        errors[0]
            .diagnostic
            .message
            .contains("map to the same directory")
    );
}

#[test]
fn semantic_analysis_checks_mapped_directory_correspondence() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"mapped\"\n[namespaces]\napp = \"src\"\n",
    );
    package.write("src/http/main.trn", "namespace app/wrong\n");

    let failure = analyze(&Package::load(&package.0).unwrap()).unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "S2020");
    assert!(
        failure.diagnostics[0]
            .message
            .contains("does not match `/app/http`")
    );
}

#[test]
fn namespace_mapping_directories_are_normalized_before_discovery_and_deduplication() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"mapped\"\n[namespaces]\napp = \"./src\"\ntools = \"src\"\n",
    );
    package.write("src/http/main.trn", "namespace app/http\n");

    let errors = Package::load(&package.0).unwrap_err();
    assert!(
        errors[0]
            .diagnostic
            .message
            .contains("map to the same directory")
    );

    package.write(
        "package.toml",
        "package = \"mapped\"\n[namespaces]\napp = \"./src\"\n",
    );
    let loaded = Package::load(&package.0).unwrap();
    assert_eq!(
        loaded.units[0].expected_namespace.as_deref(),
        Some("/app/http")
    );
}

#[test]
fn namespace_mapping_rejects_invalid_roots_and_directory_segments() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"mapped\"\n[namespaces]\n\"/\" = \"root\"\n\"app/con\" = \"reserved\"\n\"app/bad-\" = \"bad\"\nvalid = \"src\"\n",
    );
    package.write("src/Http/main.trn", "namespace valid/http\n");

    let errors = Package::load(&package.0).unwrap_err();
    let messages = errors
        .iter()
        .map(|error| error.diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("cannot be declared"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("`con` is reserved"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("`bad-` must match"))
    );
    package.write(
        "package.toml",
        "package = \"mapped\"\n[namespaces]\nvalid = \"src\"\n",
    );
    let errors = Package::load(&package.0).unwrap_err();
    assert!(
        errors[0]
            .diagnostic
            .message
            .contains("directory segment `Http` must match")
    );
}

#[cfg(unix)]
#[test]
fn namespace_discovery_follows_symlinked_source_files() {
    use std::os::unix::fs::symlink;

    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"mapped\"\n[namespaces]\napp = \"src\"\n",
    );
    package.write("elsewhere/helper.trn", "namespace app\n");
    fs::create_dir_all(package.0.join("src")).unwrap();
    symlink("../elsewhere/helper.trn", package.0.join("src/helper.trn")).unwrap();

    let loaded = Package::load(&package.0).unwrap();
    assert_eq!(loaded.units.len(), 1);
    assert_eq!(loaded.units[0].relative_path, Path::new("src/helper.trn"));
}

#[test]
fn parse_callbacks_resolve_through_import_aliases() {
    let package = TempPackage::new();
    package.write(
        "package.toml",
        "package = \"callback-alias\"\n[namespaces]\ncallbacks = \"callbacks\"\napp = \"app\"\n",
    );
    package.write(
        "callbacks/parse.trn",
        "namespace callbacks\npublic function decode int; text string\n  return text.radix; 10\n",
    );
    package.write(
        "app/main.trn",
        "namespace app\nfrom /callbacks import decode as parse-decimal\nfunction main\n  text string = >42\n  value int = text.parse; parse-decimal\n",
    );

    let compilation = compile_package(&Package::load(&package.0).unwrap()).unwrap();

    assert!(compilation.rust.contains("parse_radix"));
    assert!(compilation.rust.contains("decode"));
}
