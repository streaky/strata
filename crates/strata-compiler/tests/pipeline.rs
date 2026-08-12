use std::path::PathBuf;

const HELLO: &str = "namespace hello\n\nfrom /core output import .print\n\nprint = .print\n\nfunction main\n  print; >>\n    Hello from Strata!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".\n";

#[test]
fn hello_lowers_deterministically() {
    let first = strata_compiler::compile(PathBuf::from("case.strata"), HELLO.to_owned()).unwrap();
    let second = strata_compiler::compile(PathBuf::from("case.strata"), HELLO.to_owned()).unwrap();
    assert_eq!(first.rust, second.rust);
    assert!(
        first
            .rust
            .contains("Hello from Strata!\\n\\nTail strings make punctuation literal")
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
fn tail_string_preserves_every_remaining_character() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Strata!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; >Hello! From, \"Strata\"! >> # literal",
    );
    let compilation = strata_compiler::compile("tail.strata", source).unwrap();
    assert!(
        compilation
            .rust
            .contains("Hello! From, \\\"Strata\\\"! >> # literal")
    );
}

#[test]
fn rejects_empty_block_string() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Strata!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; >>",
    );
    let diagnostics = strata_compiler::compile("string.strata", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S0004")
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
    let source = HELLO.replace(
        "print; >>\n    Hello from Strata!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; hello",
    );
    let diagnostics = strata_compiler::compile("call.strata", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S0004")
    );
}
