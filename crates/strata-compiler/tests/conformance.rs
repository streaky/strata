use std::fs;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance")
}

#[test]
fn every_manifest_drives_a_conformance_case() {
    let manifests = manifests_below(&corpus());
    assert!(!manifests.is_empty());
    for manifest_path in manifests {
        let case = manifest_path.parent().unwrap();
        let manifest = fs::read_to_string(&manifest_path).unwrap();
        let phase = field(&manifest, "phase").unwrap();
        let status = field(&manifest, "status").unwrap();
        let entrypoint = field(&manifest, "entrypoint").unwrap_or("case.strata");
        let source_path = case.join(entrypoint);
        let source = fs::read_to_string(&source_path).unwrap();

        match (phase, status) {
            ("run", "accept") => {
                let expected = fs::read_to_string(case.join("lower.rs")).unwrap();
                let compilation = strata_compiler::compile(&source_path, source).unwrap();
                assert_eq!(compilation.rust, expected, "{}", case.display());
            }
            ("check", "reject") => {
                let code = field(&manifest, "code").unwrap();
                let diagnostics = strata_compiler::compile(&source_path, source).unwrap_err();
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic.code == code),
                    "{} did not report {code}: {diagnostics:?}",
                    case.display()
                );
            }
            _ => panic!(
                "unsupported conformance manifest {}: phase={phase}, status={status}",
                manifest_path.display()
            ),
        }
    }
}

fn manifests_below(root: &Path) -> Vec<PathBuf> {
    let mut manifests = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            manifests.extend(manifests_below(&path));
        } else if path.file_name().is_some_and(|name| name == "case.toml") {
            manifests.push(path);
        }
    }
    manifests.sort();
    manifests
}

fn field<'manifest>(manifest: &'manifest str, name: &str) -> Option<&'manifest str> {
    manifest.lines().find_map(|line| {
        line.strip_prefix(name)?
            .strip_prefix(" = \"")?
            .strip_suffix('"')
    })
}
