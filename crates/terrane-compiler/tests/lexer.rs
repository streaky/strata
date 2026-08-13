use terrane_compiler::{
    SourceFile,
    lexer::lex,
    tokens::{Attachment, TokenKind, TriviaKind},
};

fn lex_source(text: &str) -> terrane_compiler::tokens::LexedSource {
    lex(&SourceFile::new(0, "case.trn".into(), text.to_owned())).unwrap()
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
fn required_attachment_spellings_remain_distinct() {
    assert_eq!(significant("a + b")[1].2, Attachment::Detached);
    assert_eq!(significant("-einval")[0].1, "-");
    assert_eq!(significant("print.concat")[1].2, Attachment::Both);
    assert_eq!(significant("print .concat")[1].2, Attachment::Right);
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
fn bitwise_operators_and_numeric_forms_are_single_tokens() {
    for source in ["a & b", "a ^ b", "~value"] {
        assert!(
            significant(source)
                .iter()
                .any(|token| token.0 == TokenKind::Operator)
        );
    }
    for source in ["1.5", "0xff", "1_000"] {
        assert_eq!(
            significant(source),
            vec![(TokenKind::Number, source.into(), Attachment::Detached)]
        );
    }
    assert_eq!(
        significant("1.type")
            .iter()
            .map(|(kind, _, _)| *kind)
            .collect::<Vec<_>>(),
        vec![TokenKind::Number, TokenKind::Dot, TokenKind::Identifier]
    );
}

#[test]
fn malformed_numeric_literals_are_rejected_whole() {
    for source in ["1e9", "0x", "0xzz", "1_", "1__0", "123abc", "0b101"] {
        let file = SourceFile::new(0, "case.trn".into(), source.to_owned());
        let error = lex(&file)
            .unwrap_err()
            .into_iter()
            .find(|diagnostic| diagnostic.code == "L0009")
            .unwrap_or_else(|| panic!("{source} was accepted"));
        let primary = error.primary.unwrap();
        assert_eq!(
            (primary.start, primary.end),
            (0, source.len()),
            "{source} must be reported as one literal"
        );
    }
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
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| { token.kind == TokenKind::TailString && token.text == ">tail text" })
    );
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

    let source = SourceFile::new(0, "case.trn".into(), "message = >> ".to_owned());
    assert!(
        lex(&source)
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic.code == "L0008")
    );
}

#[test]
fn block_string_token_covers_body_and_uses_its_selected_prefix() {
    let lexed = lex_source("x = >>\n    first\n  second\n");
    let block = lexed
        .tokens
        .iter()
        .find(|token| token.kind == TokenKind::BlockString)
        .unwrap();
    assert_eq!(block.text, ">>\n    first");
    assert!(lexed.tokens.iter().any(|token| token.text == "second"));
}

#[test]
fn tokens_and_trivia_cover_every_source_byte_once() {
    for source in [
        "function main\n  value\n\n  # note\nafter\n",
        "x = >>\n  a\n\n  b\nafter = 1\n",
        "function main\n  /* c\n  */ value\nnext\n",
        "x = 'text' # trailing\n",
    ] {
        let lexed = lex_source(source);
        let covered: usize = lexed
            .tokens
            .iter()
            .filter(|token| token.kind != TokenKind::Eof)
            .map(|token| token.span.end - token.span.start)
            .sum::<usize>()
            + lexed
                .trivia
                .iter()
                .map(|item| item.span.end - item.span.start)
                .sum::<usize>();
        assert_eq!(covered, source.len(), "{source:?}");
    }
}

#[test]
fn a_block_string_body_terminates_exactly_one_statement() {
    let lexed = lex_source("x = >>\n  a\n\n  b\nafter = 1\n");
    let block = lexed
        .tokens
        .iter()
        .find(|token| token.kind == TokenKind::BlockString)
        .unwrap();
    assert_eq!(block.text, ">>\n  a\n\n  b");
    let newlines = lexed
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Newline)
        .count();
    assert_eq!(newlines, 2, "one terminator per statement");
    let covered: usize = lexed
        .tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Eof)
        .map(|token| token.span.end - token.span.start)
        .sum();
    let trivia: usize = lexed
        .trivia
        .iter()
        .map(|item| item.span.end - item.span.start)
        .sum();
    assert_eq!(covered + trivia, "x = >>\n  a\n\n  b\nafter = 1\n".len());
}

#[test]
fn a_comment_only_terminator_line_stays_out_of_indentation() {
    let lexed = lex_source("function main\n  /* c\n  */ # still a comment\nnext\n");
    assert!(
        !lexed
            .tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Indent | TokenKind::Dedent))
    );
}

#[test]
fn block_string_content_follows_the_file_indentation_style() {
    let source = SourceFile::new(
        0,
        "case.trn".into(),
        "function main\n  x = >>\n\t\t\tcontent\n".to_owned(),
    );
    assert!(
        lex(&source)
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic.code == "L0003")
    );
}

#[test]
fn comments_and_shift_operators_do_not_open_block_strings() {
    for source in [
        "x = 1 # use >>\n  kept = 2\nafter = 3\n",
        "x = value >>\n  8\n",
    ] {
        let lexed = lex_source(source);
        assert!(
            lexed
                .tokens
                .iter()
                .any(|token| token.text == "kept" || token.text == "8"),
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
fn tab_indentation_and_style_changes_are_covered() {
    assert_eq!(
        lex_source("function main\n\tvalue\nnext\n")
            .tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Indent)
            .count(),
        1
    );
    let source = SourceFile::new(
        0,
        "case.trn".into(),
        "function main\n  value\n\tnext\n".to_owned(),
    );
    assert!(
        lex(&source)
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic.code == "L0003")
    );
}

#[test]
fn inconsistent_dedent_is_rejected() {
    let source = SourceFile::new(
        0,
        "case.trn".into(),
        "root\n    deep\n  invalid\n".to_owned(),
    );
    assert!(
        lex(&source)
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic.code == "L0004")
    );
}

#[test]
fn structural_tokens_and_trivia_have_exact_spans() {
    let source = ":|(),[]{}--";
    let lexed = lex_source(source);
    let kinds = lexed
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect::<Vec<_>>();
    for kind in [
        TokenKind::Colon,
        TokenKind::Pipe,
        TokenKind::OpenParen,
        TokenKind::CloseParen,
        TokenKind::Comma,
        TokenKind::OpenBracket,
        TokenKind::CloseBracket,
        TokenKind::OpenBrace,
        TokenKind::CloseBrace,
        TokenKind::Decrement,
    ] {
        assert!(kinds.contains(&kind), "{kind:?}");
    }
    let trivia = lex_source("x # note").trivia.pop().unwrap();
    assert_eq!(trivia.text, "# note");
    assert_eq!(trivia.span, terrane_compiler::Span::new(0, 2, 8));
}

#[test]
fn code_after_a_multiline_comment_terminator_keeps_indentation() {
    let lexed = lex_source("function main\n  /* c\n  */ value\nnext\n");
    let kinds = lexed
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == TokenKind::Indent)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == TokenKind::Dedent)
            .count(),
        1
    );
    assert!(lexed.tokens.iter().any(|token| token.text == "value"));
}

#[test]
fn malformed_lexemes_report_originating_bytes() {
    for (text, code, start) in [
        ("count-1", "L0005", 5),
        ("a+ b", "L0006", 1),
        ("function main\n \tvalue", "L0003", 14),
        ("/* open", "L0002", 0),
        ("naïve", "L0001", 2),
    ] {
        let source = SourceFile::new(0, "case.trn".into(), text.to_owned());
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
    let source = SourceFile::new(0, "case.trn".into(), "naïve".to_owned());
    let error = lex(&source).unwrap_err().remove(0);
    assert_eq!(error.message, "invalid source character `ï`");
    assert_eq!(error.primary.unwrap(), terrane_compiler::Span::new(0, 2, 4));
}

#[test]
fn escaped_quote_does_not_terminate_a_quoted_string() {
    let source = SourceFile::new(0, "case.trn".into(), "name = 'it\\'".to_owned());
    let diagnostics = lex(&source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "L0007")
    );
}
