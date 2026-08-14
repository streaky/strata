use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
        let entrypoint = field(&manifest, "entrypoint").unwrap_or("case.trn");
        let source_path = case.join(entrypoint);
        let package_case = entrypoint == terrane_compiler::MANIFEST_FILE_NAME;

        match (phase, status) {
            ("run" | "check", "accept") => {
                let expected = fs::read_to_string(case.join("lower.rs")).unwrap();
                let compilation = if package_case {
                    let package = terrane_compiler::Package::load(&source_path).unwrap();
                    terrane_compiler::compile_package(&package).unwrap()
                } else {
                    let source = fs::read_to_string(&source_path).unwrap();
                    terrane_compiler::compile(&source_path, source).unwrap()
                };
                let normalized = compilation
                    .rust
                    .replace(terrane_compiler::VERSION, "<version>");
                assert_eq!(normalized, expected, "{}", case.display());
                compile_and_maybe_run(case, phase, &compilation.rust);
            }
            ("check", "reject") => {
                let code = field(&manifest, "code").unwrap();
                let diagnostics = if package_case {
                    let package = terrane_compiler::Package::load(&source_path).unwrap();
                    terrane_compiler::compile_package(&package)
                        .unwrap_err()
                        .diagnostics
                } else {
                    let source = fs::read_to_string(&source_path).unwrap();
                    terrane_compiler::compile(&source_path, source)
                        .unwrap_err()
                        .diagnostics
                };
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

fn compile_and_maybe_run(case: &Path, phase: &str, rust: &str) {
    let case_name = case
        .strip_prefix(corpus())
        .unwrap()
        .to_string_lossy()
        .replace(['/', '\\'], "-");
    let build_dir = std::env::temp_dir().join(format!(
        "terrane-conformance-{}-{case_name}",
        std::process::id()
    ));
    if build_dir.exists() {
        fs::remove_dir_all(&build_dir).unwrap();
    }
    fs::create_dir_all(build_dir.join("src")).unwrap();
    write_support_crates(&build_dir);
    fs::write(
        build_dir.join("Cargo.toml"),
        "[package]\nname = \"terrane_conformance_program\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\nterrane-int-support = { path = \"support/terrane-int-support\" }\n\
         terrane-scalar-support = { path = \"support/terrane-scalar-support\" }\n\
         terrane-string-support = { path = \"support/terrane-string-support\" }\n\n[workspace]\n",
    )
    .unwrap();
    fs::write(build_dir.join("src/main.rs"), rust).unwrap();
    let output = Command::new("cargo")
        .args(["build", "--quiet", "--manifest-path"])
        .arg(build_dir.join("Cargo.toml"))
        .env("RUSTFLAGS", "-Dwarnings")
        .output()
        .unwrap();
    let mut binary_path = build_dir.join("target/debug/terrane_conformance_program");
    binary_path.set_extension(std::env::consts::EXE_EXTENSION);
    assert!(
        output.status.success(),
        "{} generated Rust failed to compile:\n{}",
        case.display(),
        String::from_utf8_lossy(&output.stderr)
    );

    if phase == "run" {
        let output = Command::new(&binary_path).output().unwrap();
        let expected_stdout = fs::read(case.join("stdout.txt")).unwrap();
        let expected_stderr = optional_bytes(case.join("stderr.txt"));
        let expected_code = optional_text(case.join("exit-code.txt"))
            .map_or(0, |text| text.trim().parse().unwrap());
        assert_eq!(output.stdout, expected_stdout, "{} stdout", case.display());
        assert_eq!(output.stderr, expected_stderr, "{} stderr", case.display());
        assert_eq!(
            output.status.code(),
            Some(expected_code),
            "{} exit code",
            case.display()
        );
    }
    fs::remove_dir_all(build_dir).unwrap();
}

fn write_support_crates(directory: &Path) {
    let int = directory.join("support/terrane-int-support");
    let scalar = directory.join("support/terrane-scalar-support");
    let string = directory.join("support/terrane-string-support");
    fs::create_dir_all(int.join("src")).unwrap();
    fs::create_dir_all(scalar.join("src")).unwrap();
    fs::create_dir_all(string.join("src")).unwrap();
    fs::write(
        int.join("Cargo.toml"),
        "[package]\nname = \"terrane-int-support\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nnum-bigint = \"0.4\"\nnum-integer = \"0.1\"\nnum-traits = \"0.2\"\n",
    )
    .unwrap();
    fs::write(
        int.join("src/lib.rs"),
        include_bytes!("../../terrane-int-support/src/lib.rs"),
    )
    .unwrap();
    fs::write(
        scalar.join("Cargo.toml"),
        "[package]\nname = \"terrane-scalar-support\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nterrane-int-support = { path = \"../terrane-int-support\" }\n",
    )
    .unwrap();
    fs::write(
        scalar.join("src/lib.rs"),
        include_bytes!("../../terrane-scalar-support/src/lib.rs"),
    )
    .unwrap();
    fs::write(
        string.join("Cargo.toml"),
        "[package]\nname = \"terrane-string-support\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nunicode-segmentation = \"1\"\n",
    )
    .unwrap();
    fs::write(
        string.join("src/lib.rs"),
        include_bytes!("../../terrane-string-support/src/lib.rs"),
    )
    .unwrap();
}

fn optional_bytes(path: PathBuf) -> Vec<u8> {
    fs::read(path).unwrap_or_default()
}

fn optional_text(path: PathBuf) -> Option<String> {
    fs::read_to_string(path).ok()
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
