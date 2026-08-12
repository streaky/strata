use strata_compiler::{
    SourceFile,
    lexer::lex,
    tokens::{Attachment, TokenKind, TriviaKind},
};

fn lex_source(text: &str) -> strata_compiler::tokens::LexedSource {
    lex(&SourceFile::new(0, "case.strata".into(), text.to_owned())).unwrap()
}

fn significant(text: &str) -> Vec<(TokenKind, String, Attachment)> {
    lex_source(text)
        .tokens
        .into_iter()
        .filter(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
        .map(|token| (token.kind, token.text, token.attachment))
        .collect()
}

#[test]
fn identifiers_and_spaced_operators_have_stable_boundaries() {
    assert_eq!(
        significant("ipv4/ipv6"),
        vec![(
            TokenKind::Identifier,
            "ipv4/ipv6".into(),
            Attachment::Detached
        )]
    );
    assert_eq!(
        significant("ipv4 / ipv6"),
        vec![
            (TokenKind::Identifier, "ipv4".into(), Attachment::Detached),
            (TokenKind::Operator, "/".into(), Attachment::Detached),
            (TokenKind::Identifier, "ipv6".into(), Attachment::Detached),
        ]
    );
    assert_eq!(
        significant("a+b"),
        vec![(TokenKind::Identifier, "a+b".into(), Attachment::Detached)]
    );
    assert_eq!(
        significant("a +b"),
        vec![
            (TokenKind::Identifier, "a".into(), Attachment::Detached),
            (TokenKind::Operator, "+".into(), Attachment::Right),
            (TokenKind::Identifier, "b".into(), Attachment::Left),
        ]
    );
}

#[test]
fn punctuation_comparisons_and_shifts_are_deterministic() {
    assert_eq!(
        significant("value===other")
            .iter()
            .map(|(_, text, _)| text.as_str())
            .collect::<Vec<_>>(),
        vec!["value", "==", "=", "other"]
    );
    assert_eq!(
        significant("list<string>")
            .iter()
            .map(|(_, text, _)| text.as_str())
            .collect::<Vec<_>>(),
        vec!["list<string", ">"]
    );
    assert_eq!(
        significant("list<string>= x")
            .iter()
            .map(|(_, text, _)| text.as_str())
            .collect::<Vec<_>>(),
        vec!["list<string", ">=", "x"]
    );
    assert_eq!(significant("i++").last().unwrap().0, TokenKind::Increment);
}

#[test]
fn strings_comments_and_trivia_retain_exact_source() {
    let lexed =
        lex_source("name = 'a\\n' # note\n// second\n/* block */\nmessage = >literal # text");
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::String && token.text == "'a\\n'")
    );
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::TailString && token.text == ">literal # text")
    );
    assert_eq!(
        lexed
            .trivia
            .iter()
            .filter(|item| item.kind == TriviaKind::LineComment)
            .count(),
        2
    );
    assert_eq!(
        lexed
            .trivia
            .iter()
            .filter(|item| item.kind == TriviaKind::BlockComment)
            .count(),
        1
    );
}

#[test]
fn comments_do_not_change_expression_start() {
    let lexed = lex_source("x = /* c */ >tail text");
    assert!(lexed.tokens.iter().any(|token| {
        token.kind == TokenKind::TailString && token.text == ">tail text"
    }));
}

#[test]
fn block_strings_are_contextual_and_require_a_clean_marker_line() {
    let lexed = lex_source("message = >>\n  literal # text\nnext = left >> right");
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::BlockString)
    );
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Operator && token.text == ">>")
    );

    let source = SourceFile::new(0, "case.strata".into(), "message = >> ".to_owned());
    assert!(
        lex(&source)
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic.code == "S0004")
    );
}

#[test]
fn block_string_token_covers_body_and_uses_its_selected_prefix() {
    let lexed = lex_source("x = >>\n    first\n  second\n");
    let block = lexed.tokens.iter().find(|token| token.kind == TokenKind::BlockString).unwrap();
    assert_eq!(block.text, ">>\n    first\n");
    assert!(lexed.tokens.iter().any(|token| token.text == "second"));
}

#[test]
fn comments_and_shift_operators_do_not_open_block_strings() {
    for source in ["x = 1 # use >>\n  kept = 2\nafter = 3\n", "x = value >>\n  8\n"] {
        let lexed = lex_source(source);
        assert!(
            lexed.tokens.iter().any(|token| token.text == "kept" || token.text == "8"),
            "{source}"
        );
    }
}

#[test]
fn indentation_ignores_blank_and_comment_only_lines() {
    let lexed = lex_source("function main\n  value\n\n    # ignored\n  next\nafter\n");
    assert_eq!(
        lexed
            .tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Indent)
            .count(),
        1
    );
    assert_eq!(
        lexed
            .tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Dedent)
            .count(),
        1
    );
}

#[test]
fn code_after_a_multiline_comment_terminator_keeps_indentation() {
    let lexed = lex_source("function main\n  /* c\n  */ value\nnext\n");
    let kinds = lexed.tokens.iter().map(|token| token.kind).collect::<Vec<_>>();
    assert_eq!(kinds.iter().filter(|kind| **kind == TokenKind::Indent).count(), 1);
    assert_eq!(kinds.iter().filter(|kind| **kind == TokenKind::Dedent).count(), 1);
    assert!(lexed.tokens.iter().any(|token| token.text == "value"));
}

#[test]
fn malformed_lexemes_report_originating_bytes() {
    for (text, code, start) in [
        ("count-1", "L0005", 5),
        ("a+ b", "L0006", 1),
        ("function main\n \tvalue", "S0001", 14),
        ("/* open", "L0002", 0),
        ("naïve", "L0001", 2),
    ] {
        let source = SourceFile::new(0, "case.strata".into(), text.to_owned());
        let error = lex(&source)
            .unwrap_err()
            .into_iter()
            .find(|diagnostic| diagnostic.code == code)
            .unwrap();
        assert_eq!(error.primary.unwrap().start, start, "{text}");
    }
}

#[test]
fn multibyte_invalid_character_is_rendered_as_unicode() {
    let source = SourceFile::new(0, "case.strata".into(), "naïve".to_owned());
    let error = lex(&source).unwrap_err().remove(0);
    assert_eq!(error.message, "invalid source character `ï`");
    assert_eq!(error.primary.unwrap(), strata_compiler::Span::new(0, 2, 4));
}

#[test]
fn escaped_quote_does_not_terminate_a_quoted_string() {
    let source = SourceFile::new(0, "case.strata".into(), "name = 'it\\'".to_owned());
    let diagnostics = lex(&source).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.code == "S0002"));
}
