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
