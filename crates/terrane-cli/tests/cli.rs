use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn hello() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance/run/hello/case.trn")
}

#[test]
fn all_commands_share_the_hello_pipeline() {
    let binary = env!("CARGO_BIN_EXE_terrane");
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
    let displayed_rust = String::from_utf8(rust.stdout)
        .unwrap()
        .replace(terrane_compiler::VERSION, "<version>");
    let authored_rust = fs::read_to_string(hello().parent().unwrap().join("lower.rs")).unwrap();
    assert!(displayed_rust.starts_with(&authored_rust));
    assert!(
        displayed_rust
            .contains("// Authored generated modules: src/authored/unit-0000.rs, src/main.rs")
    );
    assert!(displayed_rust.contains("// Vendored support crates: terrane-int-support"));

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
    assert!(Path::new(executable.trim()).starts_with(source_root.join(".trn")));

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
    let binary = env!("CARGO_BIN_EXE_terrane");
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
    let binary = env!("CARGO_BIN_EXE_terrane");
    let missing = Command::new(binary)
        .args(["check", "missing.trn"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(3));
    let missing_stderr = String::from_utf8(missing.stderr).unwrap();
    assert!(missing_stderr.contains("missing.trn: error[S0000]"));
    assert!(!missing_stderr.contains("missing.trn:1:1"));

    let invalid_path = std::env::temp_dir().join(format!(
        "terrane-invalid-{}-{}.trn",
        std::process::id(),
        std::thread::current().name().unwrap_or("cli")
    ));
    fs::write(
        &invalid_path,
        "namespace invalid\nfunction main\n  missing;\n",
    )
    .unwrap();
    let invalid = Command::new(binary)
        .args(["check", invalid_path.to_str().unwrap()])
        .output()
        .unwrap();
    fs::remove_file(invalid_path).unwrap();
    assert_eq!(invalid.status.code(), Some(3));
    assert!(
        String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("unresolved name `missing`")
    );
}

#[test]
fn uncaught_source_errors_render_causes_and_terrane_frames() {
    let directory = std::env::temp_dir().join(format!(
        "terrane-runtime-error-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("cli")
    ));
    fs::create_dir_all(&directory).unwrap();
    let source = directory.join("case.trn");
    fs::write(
        &source,
        concat!(
            "namespace runtime-error\n",
            "from /core/errors import arithmetic-overflow, coercion-error\n",
            "throws function inner int\n",
            "  throw arithmetic-overflow\n",
            "throws function outer int\n",
            "  try\n",
            "    return inner;\n",
            "  catch arithmetic-overflow\n",
            "    throw coercion-error\n",
            "function main\n",
            "  outer;\n",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_terrane"))
        .args(["run", source.to_str().unwrap()])
        .output()
        .unwrap();
    fs::remove_dir_all(&directory).unwrap();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.starts_with(".coercion-error: coercion has no compatible result\n"));
    assert!(stderr.contains("caused by: .arithmetic-overflow"));
    assert!(stderr.contains("at /runtime-error::inner (case.trn:4:3)"));
    assert!(stderr.contains("at /runtime-error::outer (case.trn:7:12)"));
    assert!(stderr.contains("at /runtime-error::outer (case.trn:9:5)"));
    assert!(stderr.contains("at /runtime-error::main (case.trn:11:3)"));
    assert!(!stderr.contains("panicked"));
    assert!(!stderr.contains("src/authored"));
}
