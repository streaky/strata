use strata_compiler::{SourceFile, lexer::lex, tokens::{Attachment, TokenKind, TriviaKind}};

fn lex_source(text: &str) -> strata_compiler::tokens::LexedSource {
    lex(&SourceFile::new(0, "case.strata".into(), text.to_owned())).unwrap()
}

fn significant(text: &str) -> Vec<(TokenKind, String, Attachment)> {
    lex_source(text).tokens.into_iter()
        .filter(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
        .map(|token| (token.kind, token.text, token.attachment))
        .collect()
}

#[test]
fn identifiers_and_spaced_operators_have_stable_boundaries() {
    assert_eq!(significant("ipv4/ipv6"), vec![(TokenKind::Identifier, "ipv4/ipv6".into(), Attachment::Detached)]);
    assert_eq!(significant("ipv4 / ipv6"), vec![
        (TokenKind::Identifier, "ipv4".into(), Attachment::Detached),
        (TokenKind::Operator, "/".into(), Attachment::Detached),
        (TokenKind::Identifier, "ipv6".into(), Attachment::Detached),
    ]);
    assert_eq!(significant("a+b"), vec![(TokenKind::Identifier, "a+b".into(), Attachment::Detached)]);
    assert_eq!(significant("a +b"), vec![
        (TokenKind::Identifier, "a".into(), Attachment::Detached),
        (TokenKind::Operator, "+".into(), Attachment::Right),
        (TokenKind::Identifier, "b".into(), Attachment::Left),
    ]);
}

#[test]
fn punctuation_comparisons_and_shifts_are_deterministic() {
    assert_eq!(significant("value===other").iter().map(|(_, text, _)| text.as_str()).collect::<Vec<_>>(), vec!["value", "==", "=", "other"]);
    assert_eq!(significant("list<string>").iter().map(|(_, text, _)| text.as_str()).collect::<Vec<_>>(), vec!["list<string", ">"]);
    assert_eq!(significant("list<string>= x").iter().map(|(_, text, _)| text.as_str()).collect::<Vec<_>>(), vec!["list<string", ">=", "x"]);
    assert_eq!(significant("i++").last().unwrap().0, TokenKind::Increment);
}

#[test]
fn strings_comments_and_trivia_retain_exact_source() {
    let lexed = lex_source("name = 'a\\n' # note\n// second\n/* block */\nmessage = >literal # text");
    assert!(lexed.tokens.iter().any(|token| token.kind == TokenKind::String && token.text == "'a\\n'"));
    assert!(lexed.tokens.iter().any(|token| token.kind == TokenKind::TailString && token.text == ">literal # text"));
    assert_eq!(lexed.trivia.iter().filter(|item| item.kind == TriviaKind::LineComment).count(), 2);
    assert_eq!(lexed.trivia.iter().filter(|item| item.kind == TriviaKind::BlockComment).count(), 1);
}

#[test]
fn indentation_ignores_blank_and_comment_only_lines() {
    let lexed = lex_source("function main\n  value\n\n    # ignored\n  next\nafter\n");
    assert_eq!(lexed.tokens.iter().filter(|token| token.kind == TokenKind::Indent).count(), 1);
    assert_eq!(lexed.tokens.iter().filter(|token| token.kind == TokenKind::Dedent).count(), 1);
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
        let error = lex(&source).unwrap_err().into_iter().find(|diagnostic| diagnostic.code == code).unwrap();
        assert_eq!(error.primary.unwrap().start, start, "{text}");
    }
}
