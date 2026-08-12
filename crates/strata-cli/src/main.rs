use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match run(&std::env::args_os().skip(1).collect::<Vec<_>>()) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[OsString]) -> Result<ExitCode, String> {
    let Some(command) = arguments.first().and_then(|value| value.to_str()) else {
        return Err(usage());
    };
    if command == "--version" || command == "-V" {
        println!("strata {}", strata_compiler::VERSION);
        return Ok(ExitCode::SUCCESS);
    }
    if !matches!(command, "check" | "rust" | "build" | "run") {
        return Err(usage());
    }
    let source_path = arguments.get(1).map(PathBuf::from).ok_or_else(usage)?;
    let source_text = fs::read_to_string(&source_path)
        .map_err(|error| format!("{}: error[S0000]: {error}", source_path.display()))?;
    let diagnostic_source = source_text.clone();
    let compilation = match strata_compiler::compile(&source_path, source_text) {
        Ok(compilation) => compilation,
        Err(diagnostics) => {
            let source =
                strata_compiler::SourceFile::new(0, source_path.clone(), diagnostic_source);
            for diagnostic in diagnostics {
                eprint!("{}", diagnostic.render(&source));
            }
            return Ok(ExitCode::FAILURE);
        }
    };
    if command == "rust" {
        print!("{}", compilation.rust);
        return Ok(ExitCode::SUCCESS);
    }
    ensure_rust_toolchain()?;
    let crate_dir = generated_crate_path(&source_path, &compilation.rust)?;
    write_generated_crate(&crate_dir, &compilation.rust)?;
    let cargo_command = if command == "check" { "check" } else { "build" };
    let status = Command::new("cargo")
        .args([cargo_command, "--quiet", "--manifest-path"])
        .arg(crate_dir.join("Cargo.toml"))
        .status()
        .map_err(|error| format!("failed to start Cargo: {error}"))?;
    if !status.success() {
        return Ok(ExitCode::FAILURE);
    }
    if command == "check" {
        return Ok(ExitCode::SUCCESS);
    }
    let executable = crate_dir.join("target/debug/strata_program");
    if command == "build" {
        println!("{}", executable.display());
        return Ok(ExitCode::SUCCESS);
    }
    let separator = arguments.iter().position(|argument| argument == "--");
    let program_arguments = separator.map_or(&[][..], |index| &arguments[index + 1..]);
    let status = Command::new(executable)
        .args(program_arguments)
        .status()
        .map_err(|error| format!("failed to run generated program: {error}"))?;
    Ok(ExitCode::from(
        u8::try_from(status.code().unwrap_or(1)).unwrap_or(1),
    ))
}

fn generated_crate_path(source: &Path, rust: &str) -> Result<PathBuf, String> {
    let root = std::env::current_dir()
        .map_err(|error| format!("cannot locate working directory: {error}"))?;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in source.to_string_lossy().bytes().chain(rust.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Ok(root.join(".strata/build").join(format!("{hash:016x}")))
}

fn write_generated_crate(directory: &Path, rust: &str) -> Result<(), String> {
    fs::create_dir_all(directory.join("src"))
        .map_err(|error| format!("cannot create generated crate: {error}"))?;
    fs::write(
        directory.join("Cargo.toml"),
        "[package]\nname = \"strata_program\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n",
    )
    .map_err(|error| format!("cannot write generated manifest: {error}"))?;
    write_if_changed(&directory.join("src/main.rs"), rust.as_bytes())
        .map_err(|error| format!("cannot write generated Rust: {error}"))?;
    Ok(())
}

fn write_if_changed(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if fs::read(path).is_ok_and(|existing| existing == content) {
        return Ok(());
    }
    fs::write(path, content)
}
fn ensure_rust_toolchain() -> Result<(), String> {
    let status = Command::new("cargo")
        .arg("--version")
        .output()
        .map_err(|error| {
            format!("error[S9001]: Cargo is required to compile generated Rust: {error}")
        })?;
    if status.status.success() {
        Ok(())
    } else {
        Err("error[S9001]: Cargo prerequisite check failed".to_owned())
    }
}

fn usage() -> String {
    "usage: strata <check|rust|build|run> <source.strata> [-- program arguments]".to_owned()
}
