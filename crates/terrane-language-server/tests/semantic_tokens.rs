use terrane_compiler::highlight::{Highlight, HighlightKind, highlight};
use terrane_compiler::{SourceFile, Span};
use terrane_language_server::encode_semantic_tokens;

#[test]
fn converts_byte_spans_to_delta_encoded_utf16_positions() {
    let text = "'🙂' value";
    let highlights = [Highlight {
        span: Span::new(0, 7, 12),
        kind: HighlightKind::Variable,
        declaration: false,
    }];
    let tokens = encode_semantic_tokens(text, &highlights);

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].delta_line, 0);
    assert_eq!(tokens[0].delta_start, 5);
    assert_eq!(tokens[0].length, 5);
}

#[test]
fn splits_multiline_block_strings_into_valid_lsp_tokens() {
    let text = "value = >>\n  first\n  second\nafter = 1\n";
    let source = SourceFile::new(0, "block.trn".into(), text.to_owned());
    let highlights = highlight(&source).highlights;
    let tokens = encode_semantic_tokens(text, &highlights);

    let strings = tokens
        .iter()
        .filter(|token| token.token_type == 3)
        .collect::<Vec<_>>();
    assert_eq!(strings.len(), 3);
    assert_eq!(strings[0].length, 2);
    assert_eq!(strings[1].length, 7);
    assert_eq!(strings[2].length, 8);
}
