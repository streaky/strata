use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

struct CliFailure {
    code: u8,
    message: String,
}

impl CliFailure {
    fn usage() -> Self {
        Self {
            code: 2,
            message: usage(),
        }
    }

    fn diagnostic(path: PathBuf, code: &'static str, message: String, exit_code: u8) -> Self {
        let source = terrane_compiler::SourceFile::new(0, path, String::new());
        let diagnostic = terrane_compiler::Diagnostic::unlocated_error(code, message);
        Self {
            code: exit_code,
            message: diagnostic.render(&source),
        }
    }

    fn backend(message: String) -> Self {
        Self::diagnostic(PathBuf::from("<generated Rust>"), "S9002", message, 5)
    }
}

fn main() -> ExitCode {
    match run(&std::env::args_os().skip(1).collect::<Vec<_>>()) {
        Ok(code) => code,
        Err(failure) => {
            eprint!("{}", failure.message);
            ExitCode::from(failure.code)
        }
    }
}

fn run(arguments: &[OsString]) -> Result<ExitCode, CliFailure> {
    let Some(command) = arguments.first().and_then(|value| value.to_str()) else {
        return Err(CliFailure::usage());
    };
    if command == "--version" || command == "-V" {
        println!("terrane {}", terrane_compiler::VERSION);
        return Ok(ExitCode::SUCCESS);
    }
    if command == "--help" || command == "-h" {
        println!("{}", usage());
        return Ok(ExitCode::SUCCESS);
    }
    if !matches!(command, "check" | "rust" | "build" | "run") {
        return Err(CliFailure::usage());
    }
    let has_valid_arity = if command == "run" {
        arguments.len() == 2 || (arguments.len() >= 3 && arguments[2] == "--")
    } else {
        arguments.len() == 2
    };
    if !has_valid_arity {
        return Err(CliFailure::usage());
    }
    let input_path = arguments
        .get(1)
        .map(PathBuf::from)
        .ok_or_else(CliFailure::usage)?;
    let package = if input_path
        .extension()
        .is_some_and(|extension| extension == "trn")
    {
        let source_text = fs::read_to_string(&input_path).map_err(|error| {
            CliFailure::diagnostic(input_path.clone(), "S0000", error.to_string(), 3)
        })?;
        terrane_compiler::Package::implicit(&input_path, source_text)
    } else {
        terrane_compiler::Package::load(&input_path).map_err(|errors| CliFailure {
            code: 3,
            message: errors
                .into_iter()
                .map(|error| error.diagnostic.render(&error.source))
                .collect(),
        })?
    };
    let compilation = match terrane_compiler::compile_package(&package) {
        Ok(compilation) => compilation,
        Err(failure) => {
            return Err(CliFailure {
                code: 3,
                message: failure
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.render(&failure.source))
                    .collect(),
            });
        }
    };
    if command == "rust" {
        print_rust(&compilation);
        return Ok(ExitCode::SUCCESS);
    }
    ensure_rust_toolchain()?;
    let crate_dir = generated_crate_path(&package.root, &compilation.rust_files)?;
    write_generated_crate(&crate_dir, &compilation.rust_files, &package.units)?;
    record_and_prune_generated_crates(&crate_dir)?;
    let target_dir = package.root.join(".trn/cache/target");
    let executable = prepare_artifact(
        command,
        &crate_dir,
        &target_dir,
        &compilation.rust_files,
        &package.units,
    )?;
    if command == "check" {
        return Ok(ExitCode::SUCCESS);
    }
    let executable = executable.expect("build and run prepare an executable");
    if command == "build" {
        println!("{}", executable.display());
        return Ok(ExitCode::SUCCESS);
    }
    let separator = arguments.iter().position(|argument| argument == "--");
    let program_arguments = separator.map_or(&[][..], |index| &arguments[index + 1..]);
    let status = Command::new(executable)
        .args(program_arguments)
        .status()
        .map_err(|error| {
            CliFailure::backend(format!("failed to run generated program: {error}"))
        })?;
    Ok(ExitCode::from(
        u8::try_from(status.code().unwrap_or(1)).unwrap_or(1),
    ))
}

fn prepare_artifact(
    command: &str,
    crate_dir: &Path,
    target_dir: &Path,
    rust_files: &[terrane_compiler::rust_ir::RenderedFile],
    units: &[terrane_compiler::SourceUnit],
) -> Result<Option<PathBuf>, CliFailure> {
    if command == "check" {
        let stamp = crate_dir.join("artifacts/check-success");
        if !stamp.is_file() {
            run_cargo("check", crate_dir, target_dir, rust_files, units)?;
            fs::create_dir_all(stamp.parent().expect("artifact stamp has a parent")).map_err(
                |error| CliFailure::backend(format!("cannot create artifact cache: {error}")),
            )?;
            fs::write(&stamp, []).map_err(|error| {
                CliFailure::backend(format!("cannot record checked artifact: {error}"))
            })?;
        }
        return Ok(None);
    }
    let executable = executable_path(&crate_dir.join("artifacts"));
    if !executable.is_file() {
        run_cargo("build", crate_dir, target_dir, rust_files, units)?;
        let built = executable_path(&target_dir.join("debug"));
        fs::create_dir_all(executable.parent().expect("cached executable has a parent")).map_err(
            |error| CliFailure::backend(format!("cannot create artifact cache: {error}")),
        )?;
        fs::copy(&built, &executable)
            .map_err(|error| CliFailure::backend(format!("cannot cache built program: {error}")))?;
    }
    executable
        .canonicalize()
        .map(Some)
        .map_err(|error| CliFailure::backend(format!("cannot locate built program: {error}")))
}

fn executable_path(directory: &Path) -> PathBuf {
    let mut path = directory.join("terrane_program");
    path.set_extension(std::env::consts::EXE_EXTENSION);
    path
}

fn print_rust(compilation: &terrane_compiler::Compilation) {
    print!("{}", compilation.rust);
    println!(
        "// Authored generated modules: {}",
        compilation
            .rust_files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "// Vendored support crates: terrane-int-support, terrane-scalar-support, terrane-string-support"
    );
}

fn run_cargo(
    command: &str,
    crate_dir: &Path,
    target_dir: &Path,
    rust_files: &[terrane_compiler::rust_ir::RenderedFile],
    units: &[terrane_compiler::SourceUnit],
) -> Result<(), CliFailure> {
    let mut rustflags = std::env::var_os("RUSTFLAGS").unwrap_or_default();
    if !rustflags.is_empty() {
        rustflags.push(" ");
    }
    rustflags.push("-Dwarnings");
    let output = Command::new("cargo")
        .args([
            command,
            "--quiet",
            "--message-format=json",
            "--manifest-path",
        ])
        .arg(crate_dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTFLAGS", rustflags)
        .output()
        .map_err(|error| CliFailure::backend(format!("failed to start Cargo: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if message["reason"] != "compiler-message" || message["message"]["level"] != "error" {
            continue;
        }
        let raw = message["message"]["rendered"]
            .as_str()
            .unwrap_or("rustc reported a generated-code error");
        for span in message["message"]["spans"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|span| span["is_primary"].as_bool().unwrap_or(false))
        {
            let Some(file_name) = span["file_name"].as_str() else {
                continue;
            };
            let Some(file) = rust_files
                .iter()
                .find(|file| Path::new(file_name).ends_with(&file.path))
            else {
                continue;
            };
            let Ok(byte_start) = usize::try_from(span["byte_start"].as_u64().unwrap_or(0)) else {
                continue;
            };
            let Some(association) = file.associations.iter().find(|association| {
                association.generated_start <= byte_start && byte_start < association.generated_end
            }) else {
                continue;
            };
            let Some(source) = units
                .iter()
                .find(|unit| unit.source.id() == association.source.file)
                .map(|unit| &unit.source)
            else {
                continue;
            };
            let diagnostic = terrane_compiler::Diagnostic::error(
                "S9003",
                "generated Rust failed backend validation",
                association.source,
            );
            return Err(CliFailure {
                code: 5,
                message: format!(
                    "{}note: raw rustc diagnostic:\n{raw}",
                    diagnostic.render(source)
                ),
            });
        }
        return Err(CliFailure::backend(format!(
            "Cargo {command} failed\nnote: raw rustc diagnostic:\n{raw}"
        )));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(CliFailure::diagnostic(
        PathBuf::from("<toolchain>"),
        "S9001",
        format!("Cargo {command} failed: {}", stderr.trim()),
        4,
    ))
}

fn generated_crate_path(
    package_root: &Path,
    rust_files: &[terrane_compiler::rust_ir::RenderedFile],
) -> Result<PathBuf, CliFailure> {
    let root = package_root.canonicalize().map_err(|error| {
        CliFailure::backend(format!(
            "cannot locate package root {}: {error}",
            package_root.display()
        ))
    })?;
    let mut hash = Sha256::new();
    hash.update(b"terrane-generated-crate-v2\0");
    hash.update(terrane_compiler::VERSION.as_bytes());
    for variable in [
        "CARGO_BUILD_TARGET",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC",
        "RUSTFLAGS",
    ] {
        hash.update(variable.as_bytes());
        hash.update(b"=");
        hash.update(std::env::var(variable).unwrap_or_default());
        hash.update(b"\0");
    }
    hash.update(b"profile=debug\0");
    for file in rust_files {
        hash.update(file.path.as_bytes());
        hash.update(b"\0");
        hash.update(file.contents.as_bytes());
        hash.update(b"\0");
    }
    for support in [
        include_bytes!("../../terrane-int-support/src/lib.rs").as_slice(),
        include_bytes!("../../terrane-scalar-support/src/lib.rs").as_slice(),
        include_bytes!("../../terrane-string-support/src/lib.rs").as_slice(),
    ] {
        hash.update(support);
        hash.update(b"\0");
    }
    Ok(root
        .join(".trn/build")
        .join(format!("{:x}", hash.finalize())))
}

fn record_and_prune_generated_crates(active: &Path) -> Result<(), CliFailure> {
    const MAX_GENERATED_CRATES: usize = 8;

    fs::write(active.join(".last-used"), []).map_err(|error| {
        CliFailure::backend(format!("cannot record generated crate use: {error}"))
    })?;
    let root = active
        .parent()
        .expect("generated crate identity always has a build directory");
    let mut inactive = fs::read_dir(root)
        .map_err(|error| CliFailure::backend(format!("cannot inspect generated crates: {error}")))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path() != active && entry.path().is_dir())
        .filter_map(|entry| {
            let used = entry
                .path()
                .join(".last-used")
                .metadata()
                .or_else(|_| entry.metadata())
                .and_then(|metadata| metadata.modified())
                .ok()?;
            Some((used, entry.path()))
        })
        .collect::<Vec<_>>();
    inactive.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, path) in inactive.into_iter().skip(MAX_GENERATED_CRATES - 1) {
        fs::remove_dir_all(&path).map_err(|error| {
            CliFailure::backend(format!(
                "cannot evict stale generated crate {}: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn write_generated_crate(
    directory: &Path,
    rust_files: &[terrane_compiler::rust_ir::RenderedFile],
    units: &[terrane_compiler::SourceUnit],
) -> Result<(), CliFailure> {
    fs::create_dir_all(directory.join("src"))
        .map_err(|error| CliFailure::backend(format!("cannot create generated crate: {error}")))?;
    let manifest = "[package]\nname = \"terrane_program\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
        [dependencies]\nterrane-int-support = { path = \"support/terrane-int-support\" }\n\
        terrane-scalar-support = { path = \"support/terrane-scalar-support\" }\n\
        terrane-string-support = { path = \"support/terrane-string-support\" }\n\n[workspace]\n";
    write_if_changed(&directory.join("Cargo.toml"), manifest.as_bytes()).map_err(|error| {
        CliFailure::backend(format!("cannot write generated manifest: {error}"))
    })?;
    write_generated_support(directory).map_err(|error| {
        CliFailure::backend(format!("cannot write generated runtime support: {error}"))
    })?;
    for rust_file in rust_files {
        let path = directory.join(&rust_file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                CliFailure::backend(format!("cannot create generated source directory: {error}"))
            })?;
        }
        write_if_changed(&path, rust_file.contents.as_bytes()).map_err(|error| {
            CliFailure::backend(format!("cannot write generated Rust: {error}"))
        })?;
    }
    let mut sources = String::from("version = 1\n\n");
    for unit in units {
        write!(
            sources,
            "[[sources]]\npath = {:?}\n",
            unit.relative_path.to_string_lossy()
        )
        .expect("writing to a String cannot fail");
    }
    write_if_changed(&directory.join("terrane-build.toml"), sources.as_bytes())
        .map_err(|error| CliFailure::backend(format!("cannot write build metadata: {error}")))?;
    Ok(())
}

fn write_generated_support(directory: &Path) -> std::io::Result<()> {
    let int = directory.join("support/terrane-int-support");
    let scalar = directory.join("support/terrane-scalar-support");
    let string = directory.join("support/terrane-string-support");
    fs::create_dir_all(int.join("src"))?;
    fs::create_dir_all(scalar.join("src"))?;
    fs::create_dir_all(string.join("src"))?;
    write_if_changed(
        &int.join("Cargo.toml"),
        b"[package]\nname = \"terrane-int-support\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nnum-bigint = \"0.4\"\nnum-integer = \"0.1\"\nnum-traits = \"0.2\"\n",
    )?;
    write_if_changed(
        &int.join("src/lib.rs"),
        include_bytes!("../../terrane-int-support/src/lib.rs"),
    )?;
    write_if_changed(
        &scalar.join("Cargo.toml"),
        b"[package]\nname = \"terrane-scalar-support\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nterrane-int-support = { path = \"../terrane-int-support\" }\n",
    )?;
    write_if_changed(
        &scalar.join("src/lib.rs"),
        include_bytes!("../../terrane-scalar-support/src/lib.rs"),
    )?;
    write_if_changed(
        &string.join("Cargo.toml"),
        b"[package]\nname = \"terrane-string-support\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nunicode-casefold = \"0.2\"\nunicode-normalization = \"0.1\"\nunicode-segmentation = \"1\"\n",
    )?;
    write_if_changed(
        &string.join("src/lib.rs"),
        include_bytes!("../../terrane-string-support/src/lib.rs"),
    )
}

fn write_if_changed(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if fs::read(path).is_ok_and(|existing| existing == content) {
        return Ok(());
    }
    fs::write(path, content)
}
fn ensure_rust_toolchain() -> Result<(), CliFailure> {
    let status = Command::new("cargo")
        .arg("--version")
        .output()
        .map_err(|error| {
            CliFailure::diagnostic(
                PathBuf::from("<toolchain>"),
                "S9001",
                format!("Cargo is required to compile generated Rust: {error}"),
                4,
            )
        })?;
    if status.status.success() {
        Ok(())
    } else {
        Err(CliFailure::diagnostic(
            PathBuf::from("<toolchain>"),
            "S9001",
            "Cargo prerequisite check failed".to_owned(),
            4,
        ))
    }
}

fn usage() -> String {
    "usage: terrane <check|rust|build|run> <source.trn> [-- program arguments]\n\
     commands:\n  check  validate and compile generated Rust\n  rust   print generated Rust\n  \
     build  compile a native executable\n  run    compile and execute the program"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrane_compiler::{
        SourceFile, SourceUnit, Span,
        rust_ir::{RenderedFile, SourceAssociation},
    };

    #[test]
    fn backend_error_projects_to_terrane_source_and_retains_rustc() {
        let directory =
            std::env::temp_dir().join(format!("terrane-backend-diagnostic-{}", std::process::id()));
        if directory.exists() {
            fs::remove_dir_all(&directory).unwrap();
        }
        fs::create_dir_all(directory.join("src")).unwrap();
        fs::write(
            directory.join("Cargo.toml"),
            "[package]\nname = \"broken\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n",
        )
        .unwrap();
        let generated = "fn main() { missing_backend_name(); }\n";
        fs::write(directory.join("src/main.rs"), generated).unwrap();
        let rust_files = vec![RenderedFile {
            path: "src/main.rs".to_owned(),
            contents: generated.to_owned(),
            associations: vec![SourceAssociation {
                generated_start: 0,
                generated_end: generated.len(),
                source: Span::new(0, 0, 14),
            }],
        }];
        let units = vec![SourceUnit {
            relative_path: PathBuf::from("case.trn"),
            source: SourceFile::new(0, PathBuf::from("case.trn"), "function main\n".to_owned()),
            expected_namespace: None,
        }];

        let failure = run_cargo(
            "check",
            &directory,
            &directory.join("target"),
            &rust_files,
            &units,
        )
        .unwrap_err();
        assert_eq!(failure.code, 5);
        assert!(failure.message.contains("case.trn:1:1: error[S9003]"));
        assert!(failure.message.contains("raw rustc diagnostic"));
        assert!(failure.message.contains("missing_backend_name"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn generated_crate_cache_evicts_stale_identities() {
        let directory =
            std::env::temp_dir().join(format!("terrane-cache-eviction-{}", std::process::id()));
        if directory.exists() {
            fs::remove_dir_all(&directory).unwrap();
        }
        fs::create_dir_all(&directory).unwrap();
        for index in 0..10 {
            let identity = directory.join(format!("{index:02}"));
            fs::create_dir(&identity).unwrap();
            assert!(record_and_prune_generated_crates(&identity).is_ok());
        }
        let identities = fs::read_dir(&directory).unwrap().count();
        assert_eq!(identities, 8);
        assert!(directory.join("09").is_dir());
        fs::remove_dir_all(directory).unwrap();
    }
}
