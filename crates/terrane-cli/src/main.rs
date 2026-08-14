use std::ffi::OsString;
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
        print!("{}", compilation.rust);
        return Ok(ExitCode::SUCCESS);
    }
    ensure_rust_toolchain()?;
    let crate_dir = generated_crate_path(compilation.source.path(), &compilation.rust)?;
    write_generated_crate(&crate_dir, &compilation.rust)?;
    let cargo_command = if command == "check" { "check" } else { "build" };
    let status = Command::new("cargo")
        .args([cargo_command, "--quiet", "--manifest-path"])
        .arg(crate_dir.join("Cargo.toml"))
        .status()
        .map_err(|error| CliFailure::backend(format!("failed to start Cargo: {error}")))?;
    if !status.success() {
        return Err(CliFailure::backend(format!("Cargo {cargo_command} failed")));
    }
    if command == "check" {
        return Ok(ExitCode::SUCCESS);
    }
    let executable = crate_dir.join("target/debug/terrane_program");
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

fn generated_crate_path(source: &Path, rust: &str) -> Result<PathBuf, CliFailure> {
    let source = source.canonicalize().map_err(|error| {
        CliFailure::backend(format!(
            "cannot locate source file {}: {error}",
            source.display()
        ))
    })?;
    let root = source.parent().ok_or_else(|| {
        CliFailure::backend(format!(
            "source file {} has no parent directory",
            source.display()
        ))
    })?;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in rust.bytes().chain(
        include_bytes!("../../terrane-int-support/src/lib.rs")
            .iter()
            .chain(include_bytes!("../../terrane-string-support/src/lib.rs"))
            .copied(),
    ) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Ok(root.join(".trn/build").join(format!("{hash:016x}")))
}

fn write_generated_crate(directory: &Path, rust: &str) -> Result<(), CliFailure> {
    fs::create_dir_all(directory.join("src"))
        .map_err(|error| CliFailure::backend(format!("cannot create generated crate: {error}")))?;
    let manifest = "[package]\nname = \"terrane_program\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
        [dependencies]\nterrane-int-support = { path = \"support/terrane-int-support\" }\n\
        terrane-string-support = { path = \"support/terrane-string-support\" }\n\n[workspace]\n";
    fs::write(directory.join("Cargo.toml"), manifest).map_err(|error| {
        CliFailure::backend(format!("cannot write generated manifest: {error}"))
    })?;
    write_generated_support(directory).map_err(|error| {
        CliFailure::backend(format!("cannot write generated runtime support: {error}"))
    })?;
    write_if_changed(&directory.join("src/main.rs"), rust.as_bytes())
        .map_err(|error| CliFailure::backend(format!("cannot write generated Rust: {error}")))?;
    Ok(())
}

fn write_generated_support(directory: &Path) -> std::io::Result<()> {
    let int = directory.join("support/terrane-int-support");
    let string = directory.join("support/terrane-string-support");
    fs::create_dir_all(int.join("src"))?;
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
        &string.join("Cargo.toml"),
        b"[package]\nname = \"terrane-string-support\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nunicode-segmentation = \"1\"\n",
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
