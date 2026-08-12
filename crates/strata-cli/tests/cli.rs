use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn hello() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/run/hello/case.strata")
}

#[test]
fn all_commands_share_the_hello_pipeline() {
    let binary = env!("CARGO_BIN_EXE_strata");
    let rust = Command::new(binary)
        .args(["rust", hello().to_str().unwrap()])
        .output()
        .unwrap();
    let rust_again = Command::new(binary)
        .args(["rust", hello().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(rust.status.success());
    assert!(rust_again.status.success());
    assert_eq!(rust.stdout, rust_again.stdout);
    assert_eq!(
        String::from_utf8(rust.stdout)
            .unwrap()
            .replace(strata_compiler::VERSION, "<version>"),
        fs::read_to_string(hello().parent().unwrap().join("lower.rs")).unwrap()
    );

    let check = Command::new(binary)
        .args(["check", hello().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(check.status.success());

    let build = Command::new(binary)
        .args(["build", hello().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(build.status.success());
    let executable = String::from_utf8(build.stdout).unwrap();
    assert!(Path::new(executable.trim()).is_file());
    let source_root = hello().parent().unwrap().canonicalize().unwrap();
    assert!(Path::new(executable.trim()).starts_with(source_root.join(".strata")));

    let run = Command::new(binary)
        .args(["run", hello().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(run.status.success());
    assert_eq!(
        run.stdout,
        fs::read(hello().parent().unwrap().join("stdout.txt")).unwrap()
    );
}

#[test]
fn help_succeeds_and_extra_arguments_are_rejected() {
    let binary = env!("CARGO_BIN_EXE_strata");
    let help = Command::new(binary).arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("commands:")
    );

    let extra = Command::new(binary)
        .args(["check", hello().to_str().unwrap(), "unexpected"])
        .output()
        .unwrap();
    assert_eq!(extra.status.code(), Some(2));
    assert!(
        String::from_utf8(extra.stderr)
            .unwrap()
            .starts_with("usage:")
    );
}

#[test]
fn failures_use_distinct_exit_codes_and_compiler_diagnostics() {
    let binary = env!("CARGO_BIN_EXE_strata");
    let missing = Command::new(binary)
        .args(["check", "missing.strata"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(3));
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("error[S0000]")
    );

    let invalid_path = std::env::temp_dir().join(format!(
        "strata-invalid-{}-{}.strata",
        std::process::id(),
        std::thread::current().name().unwrap_or("cli")
    ));
    fs::write(&invalid_path, "namespace invalid\n").unwrap();
    let invalid = Command::new(binary)
        .args(["check", invalid_path.to_str().unwrap()])
        .output()
        .unwrap();
    fs::remove_file(invalid_path).unwrap();
    assert_eq!(invalid.status.code(), Some(3));
    assert!(
        String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("error[S0003]")
    );
}
