use crate::{Diagnostic, SourceFile, Span};
use crate::tokens::{Attachment, LexedSource, Token, TokenKind, Trivia, TriviaKind};

pub fn lex(source: &SourceFile) -> Result<LexedSource, Vec<Diagnostic>> {
    let text = source.text();
    let mut tokens = Vec::new();
    let mut trivia = Vec::new();
    let mut diagnostics = Vec::new();
    let mut logical_lines = Vec::new();
    let mut offset = 0;
    let mut block_indent = None;

    for raw in text.split_inclusive('\n') {
        let line = raw.trim_end_matches(['\n', '\r']);
        logical_lines.push((offset, line.to_owned()));
        let indent = indentation_len(line);
        let in_block = match block_indent {
            Some(parent) if line.trim().is_empty() || indent > parent => true,
            Some(_) => {
                block_indent = None;
                false
            }
            None => false,
        };
        if !in_block {
            lex_line(source, line, offset, &mut tokens, &mut trivia, &mut diagnostics);
            if line.trim_end().ends_with(">>") {
                block_indent = Some(indent);
            }
        }
        if raw.ends_with('\n') {
            push_token(source, &mut tokens, TokenKind::Newline, offset + line.len(), offset + raw.len(), Attachment::Detached);
        }
        offset += raw.len();
    }
    if text.is_empty() {
        logical_lines.push((0, String::new()));
    } else if !text.ends_with('\n') {
        push_token(source, &mut tokens, TokenKind::Newline, text.len(), text.len(), Attachment::Detached);
    }
    push_token(source, &mut tokens, TokenKind::Eof, text.len(), text.len(), Attachment::Detached);

    if diagnostics.is_empty() {
        Ok(LexedSource { tokens, trivia, logical_lines })
    } else {
        Err(diagnostics)
    }
}

fn lex_line(
    source: &SourceFile,
    line: &str,
    base: usize,
    tokens: &mut Vec<Token>,
    trivia: &mut Vec<Trivia>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        match bytes[index] {
            b' ' | b'\t' => {
                index += 1;
                while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') { index += 1; }
                trivia.push(Trivia { kind: TriviaKind::Whitespace, span: Span::new(source.id(), base + start, base + index), text: line[start..index].to_owned() });
            }
            b'#' => {
                trivia.push(Trivia { kind: TriviaKind::LineComment, span: Span::new(source.id(), base + start, base + bytes.len()), text: line[start..].to_owned() });
                break;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                trivia.push(Trivia { kind: TriviaKind::LineComment, span: Span::new(source.id(), base + start, base + bytes.len()), text: line[start..].to_owned() });
                break;
            }
            byte if byte.is_ascii_alphabetic() => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index].is_ascii_alphanumeric() {
                        index += 1;
                    } else if is_joiner(bytes[index])
                        && bytes[index..].iter().skip_while(|byte| is_joiner(**byte)).any(u8::is_ascii_alphabetic)
                    {
                        index += 1;
                    } else {
                        break;
                    }
                }
                push_token(source, tokens, TokenKind::Identifier, base + start, base + index, attachment(line, start, index));
            }
            byte if byte.is_ascii_digit() => {
                index += 1;
                while index < bytes.len() && bytes[index].is_ascii_digit() { index += 1; }
                push_token(source, tokens, TokenKind::Number, base + start, base + index, attachment(line, start, index));
            }
            b'.' => { index += 1; push_token(source, tokens, TokenKind::Dot, base + start, base + index, attachment(line, start, index)); }
            b';' => { index += 1; push_token(source, tokens, TokenKind::Semicolon, base + start, base + index, attachment(line, start, index)); }
            b',' => { index += 1; push_token(source, tokens, TokenKind::Comma, base + start, base + index, attachment(line, start, index)); }
            b'=' => { index += 1; push_token(source, tokens, TokenKind::Assign, base + start, base + index, attachment(line, start, index)); }
            b':' => { index += 1; push_token(source, tokens, TokenKind::Colon, base + start, base + index, attachment(line, start, index)); }
            b'|' => { index += 1; push_token(source, tokens, TokenKind::Pipe, base + start, base + index, attachment(line, start, index)); }
            b'(' => { index += 1; push_token(source, tokens, TokenKind::OpenParen, base + start, base + index, attachment(line, start, index)); }
            b')' => { index += 1; push_token(source, tokens, TokenKind::CloseParen, base + start, base + index, attachment(line, start, index)); }
            b'[' => { index += 1; push_token(source, tokens, TokenKind::OpenBracket, base + start, base + index, attachment(line, start, index)); }
            b']' => { index += 1; push_token(source, tokens, TokenKind::CloseBracket, base + start, base + index, attachment(line, start, index)); }
            b'{' => { index += 1; push_token(source, tokens, TokenKind::OpenBrace, base + start, base + index, attachment(line, start, index)); }
            b'}' => { index += 1; push_token(source, tokens, TokenKind::CloseBrace, base + start, base + index, attachment(line, start, index)); }
            b'\'' => {
                index += 1;
                let mut escaped = false;
                while index < bytes.len() {
                    if bytes[index] == b'\'' && !escaped {
                        index += 1;
                        break;
                    }
                    escaped = bytes[index] == b'\\' && !escaped;
                    if bytes[index] != b'\\' { escaped = false; }
                    index += 1;
                }
                if !line[start + 1..index].ends_with('\'') {
                    diagnostics.push(Diagnostic::error("S0002", "unterminated string literal", Span::new(source.id(), base + start, base + index)));
                } else {
                    push_token(source, tokens, TokenKind::String, base + start, base + index, attachment(line, start, index));
                }
            }
            b'>' if expression_start(line, start) => {
                if bytes.get(index + 1) == Some(&b'>') {
                    index += 2;
                    let kind = if index == bytes.len() { TokenKind::BlockString } else { TokenKind::Operator };
                    push_token(source, tokens, kind, base + start, base + index, attachment(line, start, index));
                    if kind == TokenKind::BlockString { break; }
                } else {
                    index = bytes.len();
                    push_token(source, tokens, TokenKind::TailString, base + start, base + index, attachment(line, start, index));
                    break;
                }
            }
            byte if is_joiner(byte) || matches!(byte, b'!' | b'<' | b'>' | b'%') => {
                index += 1;
                while index < bytes.len() && (is_joiner(bytes[index]) || matches!(bytes[index], b'!' | b'=' | b'<' | b'>')) { index += 1; }
                push_token(source, tokens, TokenKind::Operator, base + start, base + index, attachment(line, start, index));
            }
            other => {
                let width = line[start..].chars().next().map_or(1, char::len_utf8);
                diagnostics.push(Diagnostic::error("L0001", format!("invalid source character `{}`", char::from(other)), Span::new(source.id(), base + start, base + start + width)));
                index += width;
            }
        }
    }
}

fn is_joiner(byte: u8) -> bool {
    matches!(byte, b'+' | b'-' | b'*' | b'/' | b'%' | b'<' | b'>')
}

fn indentation_len(line: &str) -> usize {
    line.bytes().take_while(|byte| matches!(byte, b' ' | b'\t')).count()
}

fn expression_start(line: &str, index: usize) -> bool {
    let before = line[..index].trim_end();
    before.is_empty()
        || before.ends_with('=')
        || before.ends_with(';')
        || before.ends_with(',')
        || before.ends_with('(')
        || before.ends_with('[')
        || before.ends_with('{')
}

fn push_token(source: &SourceFile, tokens: &mut Vec<Token>, kind: TokenKind, start: usize, end: usize, attachment: Attachment) {
    tokens.push(Token { kind, span: Span::new(source.id(), start, end), text: source.text()[start..end].to_owned(), attachment });
}

fn attachment(line: &str, start: usize, end: usize) -> Attachment {
    let left = start > 0 && !line.as_bytes()[start - 1].is_ascii_whitespace();
    let right = end < line.len() && !line.as_bytes()[end].is_ascii_whitespace();
    match (left, right) {
        (false, false) => Attachment::Detached,
        (true, false) => Attachment::Left,
        (false, true) => Attachment::Right,
        (true, true) => Attachment::Both,
    }
}
