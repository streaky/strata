use std::path::PathBuf;

const HELLO: &str = "namespace hello\n\nfrom /core output import .print\n\nprint = .print\n\nfunction main\n  print; 'hello from strata'\n";

#[test]
fn hello_lowers_deterministically() {
    let first = strata_compiler::compile(PathBuf::from("case.strata"), HELLO.to_owned()).unwrap();
    let second = strata_compiler::compile(PathBuf::from("case.strata"), HELLO.to_owned()).unwrap();
    assert_eq!(first.rust, second.rust);
    assert!(
        first
            .rust
            .contains("println!(\"{}\", \"hello from strata\")")
    );
}

#[test]
fn rejects_mixed_indentation() {
    let source = HELLO.replace("  print", " \tprint");
    let diagnostics = strata_compiler::compile("mixed.strata", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S0001")
    );
}

#[test]
fn rejects_unterminated_string() {
    let source = HELLO.replace("'hello from strata'", "'hello from strata");
    let diagnostics = strata_compiler::compile("string.strata", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S0002")
    );
}

#[test]
fn rejects_unresolved_object() {
    let source = HELLO.replace("print = .print", "print = .missing");
    let diagnostics = strata_compiler::compile("object.strata", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S0003")
    );
}

#[test]
fn rejects_wrong_call() {
    let source = HELLO.replace("print; 'hello from strata'", "print; hello");
    let diagnostics = strata_compiler::compile("call.strata", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S0004")
    );
}
