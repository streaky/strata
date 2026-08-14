use std::path::PathBuf;

const HELLO: &str = include_str!("../../../tests/conformance/run/hello/case.trn");

#[test]
fn hello_lowers_deterministically() {
    let first = terrane_compiler::compile(PathBuf::from("case.trn"), HELLO.to_owned()).unwrap();
    let second = terrane_compiler::compile(PathBuf::from("case.trn"), HELLO.to_owned()).unwrap();
    assert_eq!(first.rust, second.rust);
    assert!(
        first
            .rust
            .contains("Hello from Terrane!\\n\\nTail strings make punctuation literal")
    );
}

#[test]
fn inferred_local_first_assignment_lowers_as_a_declaration() {
    let source = "namespace inferred\nfunction main\n  total = 5\n  total = total + 1\n";
    let compilation = terrane_compiler::compile("inferred.trn", source.to_owned()).unwrap();

    assert!(compilation.rust.contains("let mut total: i128 = 5;"));
    assert!(compilation.rust.contains("total = (total + 1);"));
}

#[test]
fn rejects_duplicate_declarations() {
    let cases = [
        (
            "namespace hello",
            "S0005",
            "duplicate namespace declaration",
        ),
        ("print = .print", "S2005", "duplicate declaration `print`"),
        ("function main", "S2005", "duplicate declaration `main`"),
    ];

    for (construct, code, message) in cases {
        let source = HELLO.replacen(construct, &format!("{construct}\n{construct}"), 1);
        let diagnostics = terrane_compiler::compile("duplicate.trn", source).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code && diagnostic.message == message)
        );
    }
}

#[test]
fn rejects_mixed_indentation() {
    let source = HELLO.replace("  print", " \tprint");
    let diagnostics = terrane_compiler::compile("mixed.trn", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "L0003")
    );
}

#[test]
fn blank_lines_do_not_select_indentation_style() {
    let source = HELLO
        .replace(
            "function main\n  print; >>",
            "function main\n \n\tprint; >>",
        )
        .replace("\n    Hello from Terrane!", "\n\t\tHello from Terrane!")
        .replace("\n    Tail strings", "\n\t\tTail strings");
    terrane_compiler::compile("blank-indent.trn", source).unwrap();
}

#[test]
fn permits_a_comment_after_a_closed_quote() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; 'hello' # trailing comment",
    );
    let compilation = terrane_compiler::compile("trailing-comment.trn", source).unwrap();
    assert!(
        compilation
            .rust
            .contains("println!(\"{}\", String::from(\"hello\"));")
    );
}

#[test]
fn compilation_failure_owns_the_original_source() {
    let source = HELLO.replace("print = .print", "print = .missing");
    let failure = terrane_compiler::compile("owned.trn", source.clone()).unwrap_err();
    assert_eq!(failure.source.text(), source);
    assert_eq!(failure.source.path(), PathBuf::from("owned.trn").as_path());
    assert!(failure.iter().any(|diagnostic| diagnostic.code == "S2014"));
}

#[test]
fn tail_string_preserves_every_remaining_character() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; >Hello! From, \"Terrane\"! >> # literal",
    );
    let compilation = terrane_compiler::compile("tail.trn", source).unwrap();
    assert!(
        compilation
            .rust
            .contains("Hello! From, \\\"Terrane\\\"! >> # literal")
    );
}

#[test]
fn tail_string_can_be_empty() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; >",
    );
    let compilation = terrane_compiler::compile("empty-tail.trn", source).unwrap();
    assert!(
        compilation
            .rust
            .contains("println!(\"{}\", String::from(\"\"));")
    );
}

#[test]
fn tail_string_preserves_leading_whitespace() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; > hello",
    );
    let compilation = terrane_compiler::compile("leading-space.trn", source).unwrap();
    assert!(
        compilation
            .rust
            .contains("println!(\"{}\", String::from(\" hello\"));")
    );
}
#[test]
fn block_string_can_be_empty() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; >>",
    );
    let compilation = terrane_compiler::compile("string.trn", source).unwrap();
    assert!(
        compilation
            .rust
            .contains("println!(\"{}\", String::from(\"\"));")
    );
}

#[test]
fn rejects_trailing_content_after_block_marker() {
    let source = HELLO.replace("print; >>", "print; >> ");
    let diagnostics = terrane_compiler::compile("marker.trn", source).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "L0008" && diagnostic.message.contains("final content")
    }));
}

#[test]
fn rejects_unresolved_object() {
    let source = HELLO.replace("print = .print", "print = .missing");
    let diagnostics = terrane_compiler::compile("object.trn", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S2014")
    );
}

#[test]
fn rejects_unresolved_call_argument() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; hello",
    );
    let diagnostics = terrane_compiler::compile("call.trn", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S2013")
    );
}

#[test]
fn compilation_uses_the_shared_parser_before_semantics() {
    let source = HELLO.replace("function main", "function main; ,");
    let diagnostics = terrane_compiler::compile("syntax.trn", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S1007")
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S0005")
    );
}
