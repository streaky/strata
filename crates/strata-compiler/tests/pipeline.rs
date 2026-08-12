use std::path::PathBuf;

const HELLO: &str = include_str!("../../../tests/conformance/run/hello/case.strata");

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
fn blank_lines_do_not_select_indentation_style() {
    let source = HELLO
        .replace(
            "function main\n  print; >>",
            "function main\n \n\tprint; >>",
        )
        .replace("\n    Hello from Strata!", "\n\t\tHello from Strata!")
        .replace("\n    Tail strings", "\n\t\tTail strings");
    strata_compiler::compile("blank-indent.strata", source).unwrap();
}

#[test]
fn diagnoses_content_after_a_closed_quote() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Strata!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; 'hello' # unsupported trailing comment",
    );
    let failure = strata_compiler::compile("trailing-content.strata", source).unwrap_err();
    assert!(failure.iter().any(|diagnostic| {
        diagnostic.code == "S0004" && diagnostic.message.contains("after closing string quote")
    }));
    assert!(
        !failure
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unterminated"))
    );
}

#[test]
fn compilation_failure_owns_the_original_source() {
    let source = HELLO.replace("print = .print", "print = .missing");
    let failure = strata_compiler::compile("owned.strata", source.clone()).unwrap_err();
    assert_eq!(failure.source.text(), source);
    assert_eq!(
        failure.source.path(),
        PathBuf::from("owned.strata").as_path()
    );
    assert!(failure.iter().any(|diagnostic| diagnostic.code == "S0003"));
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
fn tail_string_can_be_empty() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Strata!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; >",
    );
    let compilation = strata_compiler::compile("empty-tail.strata", source).unwrap();
    assert!(compilation.rust.contains("println!(\"{}\", \"\");"));
}

#[test]
fn tail_string_preserves_leading_whitespace() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Strata!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; > hello",
    );
    let compilation = strata_compiler::compile("leading-space.strata", source).unwrap();
    assert!(compilation.rust.contains("println!(\"{}\", \" hello\");"));
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
fn rejects_trailing_content_after_block_marker() {
    let source = HELLO.replace("print; >>", "print; >> ");
    let diagnostics = strata_compiler::compile("marker.strata", source).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "S0004" && diagnostic.message.contains("final content")
    }));
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
