use std::fs;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance")
}

#[test]
fn accepted_hello_matches_lowering_golden() {
    let case = corpus().join("run/hello");
    let source_path = case.join("case.strata");
    let source = fs::read_to_string(&source_path).unwrap();
    let expected = fs::read_to_string(case.join("lower.rs")).unwrap();
    let compilation = strata_compiler::compile(&source_path, source).unwrap();
    assert_eq!(compilation.rust, expected);
}

#[test]
fn rejected_cases_report_manifest_code() {
    for name in [
        "mixed-indentation",
        "unterminated-string",
        "unresolved-object",
        "wrong-call",
    ] {
        let case = corpus().join("reject").join(name);
        let manifest = fs::read_to_string(case.join("case.toml")).unwrap();
        let code = manifest
            .lines()
            .find_map(|line| line.strip_prefix("code = \""))
            .and_then(|value| value.strip_suffix('"'))
            .unwrap();
        let source_path = case.join("case.strata");
        let source = fs::read_to_string(&source_path).unwrap();
        let diagnostics = strata_compiler::compile(&source_path, source).unwrap_err();
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.code == code),
            "{name} did not report {code}: {diagnostics:?}"
        );
    }
}
