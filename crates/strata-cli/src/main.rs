use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    match arguments.next().as_deref() {
        Some(argument) if argument == "--version" || argument == "-V" => {
            println!("strata {}", strata_compiler::VERSION);
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: strata <check|rust|build|run> <source.strata> [-- program arguments]");
            ExitCode::from(2)
        }
    }
}
