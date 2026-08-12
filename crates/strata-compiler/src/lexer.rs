use crate::tokens::{Attachment, LexedSource, Token, TokenKind, Trivia, TriviaKind};
use crate::{Diagnostic, SourceFile, Span};

pub fn lex(source: &SourceFile) -> Result<LexedSource, Vec<Diagnostic>> {
    let text = source.text();
    let mut tokens = Vec::new();
    let mut trivia = Vec::new();
    let mut diagnostics = Vec::new();
    let mut logical_lines = Vec::new();
    let mut offset = 0;
    let mut block_string_indent = None;
    let mut block_comment_start = None;
    let mut indent_style = None;
    let mut indent_stack = vec![0];
    for raw in text.split_inclusive('\n') {
        let line = raw.trim_end_matches(['\n', '\r']);
        logical_lines.push((offset, line.to_owned()));
        let indent = indentation_len(line);
        let in_block_string = match block_string_indent {
            Some(parent) if line.trim().is_empty() || indent > parent => true,
            Some(_) => {
                block_string_indent = None;
                false
            }
            None => false,
        };
        let trimmed = line[indent..].trim_start();
        let comment_only = block_comment_start.is_some()
            || trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("//")
            || (trimmed.starts_with("/*") && !trimmed.contains("*/"));
        if !in_block_string {
            if !comment_only {
                check_indent(
                    source,
                    offset,
                    &line.as_bytes()[..indent],
                    &mut indent_style,
                    &mut diagnostics,
                );
                emit_indentation(
                    source,
                    offset,
                    indent,
                    &mut indent_stack,
                    &mut tokens,
                    &mut diagnostics,
                );
            }
            lex_line(
                source,
                line,
                offset,
                &mut tokens,
                &mut trivia,
                &mut diagnostics,
                &mut block_comment_start,
            );
            if line.trim_end().ends_with(">>") {
                block_string_indent = Some(indent);
            }
        }
        if raw.ends_with('\n') {
            push_token(
                source,
                &mut tokens,
                TokenKind::Newline,
                offset + line.len(),
                offset + raw.len(),
                Attachment::Detached,
            );
        }
        offset += raw.len();
    }
    if text.is_empty() {
        logical_lines.push((0, String::new()));
    } else if !text.ends_with('\n') {
        push_token(
            source,
            &mut tokens,
            TokenKind::Newline,
            text.len(),
            text.len(),
            Attachment::Detached,
        );
    }
    if let Some(start) = block_comment_start {
        diagnostics.push(Diagnostic::error(
            "L0002",
            "unterminated block comment",
            Span::new(source.id(), start, start + 2),
        ));
    }
    while indent_stack.len() > 1 {
        indent_stack.pop();
        push_token(
            source,
            &mut tokens,
            TokenKind::Dedent,
            text.len(),
            text.len(),
            Attachment::Detached,
        );
    }
    push_token(
        source,
        &mut tokens,
        TokenKind::Eof,
        text.len(),
        text.len(),
        Attachment::Detached,
    );

    if diagnostics.is_empty() {
        Ok(LexedSource {
            tokens,
            trivia,
            logical_lines,
        })
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
    block_comment_start: &mut Option<usize>,
) {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if block_comment_start.is_some() {
            if let Some(relative_end) = line[index..].find("*/") {
                let end = index + relative_end + 2;
                trivia.push(Trivia {
                    kind: TriviaKind::BlockComment,
                    span: Span::new(source.id(), base + index, base + end),
                    text: line[index..end].to_owned(),
                });
                *block_comment_start = None;
                index = end;
                continue;
            }
            trivia.push(Trivia {
                kind: TriviaKind::BlockComment,
                span: Span::new(source.id(), base + index, base + line.len()),
                text: line[index..].to_owned(),
            });
            break;
        }
        let start = index;
        match bytes[index] {
            b' ' | b'\t' => {
                index += 1;
                while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
                    index += 1;
                }
                trivia.push(Trivia {
                    kind: TriviaKind::Whitespace,
                    span: Span::new(source.id(), base + start, base + index),
                    text: line[start..index].to_owned(),
                });
            }
            b'#' => {
                trivia.push(Trivia {
                    kind: TriviaKind::LineComment,
                    span: Span::new(source.id(), base + start, base + bytes.len()),
                    text: line[start..].to_owned(),
                });
                break;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                trivia.push(Trivia {
                    kind: TriviaKind::LineComment,
                    span: Span::new(source.id(), base + start, base + bytes.len()),
                    text: line[start..].to_owned(),
                });
                break;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                if let Some(relative_end) = line[index + 2..].find("*/") {
                    index += relative_end + 4;
                    trivia.push(Trivia {
                        kind: TriviaKind::BlockComment,
                        span: Span::new(source.id(), base + start, base + index),
                        text: line[start..index].to_owned(),
                    });
                } else {
                    trivia.push(Trivia {
                        kind: TriviaKind::BlockComment,
                        span: Span::new(source.id(), base + start, base + line.len()),
                        text: line[start..].to_owned(),
                    });
                    *block_comment_start = Some(base + start);
                    break;
                }
            }
            byte if byte.is_ascii_alphabetic() => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index].is_ascii_alphanumeric() {
                        index += 1;
                        continue;
                    }
                    if !is_joiner(bytes[index]) {
                        break;
                    }
                    let joiner_start = index;
                    while index < bytes.len() && is_joiner(bytes[index]) {
                        index += 1;
                    }
                    let unit_start = index;
                    while index < bytes.len() && bytes[index].is_ascii_alphanumeric() {
                        index += 1;
                    }
                    if unit_start == index {
                        index = joiner_start;
                        break;
                    }
                    if !bytes[unit_start..index].iter().any(u8::is_ascii_alphabetic) {
                        diagnostics.push(Diagnostic::error(
                            "L0005",
                            "identifier joiner cannot introduce a digits-only terminal unit; add spaces for an operator expression",
                            Span::new(source.id(), base + joiner_start, base + index),
                        ));
                        break;
                    }
                }
                push_token(
                    source,
                    tokens,
                    TokenKind::Identifier,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            byte if byte.is_ascii_digit() => {
                index += 1;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                push_token(
                    source,
                    tokens,
                    TokenKind::Number,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b'.' => {
                index += 1;
                push_token(
                    source,
                    tokens,
                    TokenKind::Dot,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b';' => {
                index += 1;
                push_token(
                    source,
                    tokens,
                    TokenKind::Semicolon,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b',' => {
                index += 1;
                push_token(
                    source,
                    tokens,
                    TokenKind::Comma,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b'=' => {
                index += 1;
                let kind = if bytes.get(index) == Some(&b'=') {
                    index += 1;
                    TokenKind::Operator
                } else {
                    TokenKind::Assign
                };
                push_token(
                    source,
                    tokens,
                    kind,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b':' => {
                index += 1;
                push_token(
                    source,
                    tokens,
                    TokenKind::Colon,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b'|' => {
                index += 1;
                push_token(
                    source,
                    tokens,
                    TokenKind::Pipe,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b'(' => {
                index += 1;
                push_token(
                    source,
                    tokens,
                    TokenKind::OpenParen,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b')' => {
                index += 1;
                push_token(
                    source,
                    tokens,
                    TokenKind::CloseParen,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b'[' => {
                index += 1;
                push_token(
                    source,
                    tokens,
                    TokenKind::OpenBracket,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b']' => {
                index += 1;
                push_token(
                    source,
                    tokens,
                    TokenKind::CloseBracket,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b'{' => {
                index += 1;
                push_token(
                    source,
                    tokens,
                    TokenKind::OpenBrace,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b'}' => {
                index += 1;
                push_token(
                    source,
                    tokens,
                    TokenKind::CloseBrace,
                    base + start,
                    base + index,
                    attachment(line, start, index),
                );
            }
            b'\'' => {
                index += 1;
                let mut escaped = false;
                while index < bytes.len() {
                    if bytes[index] == b'\'' && !escaped {
                        index += 1;
                        break;
                    }
                    escaped = bytes[index] == b'\\' && !escaped;
                    if bytes[index] != b'\\' {
                        escaped = false;
                    }
                    index += 1;
                }
                if !line[start + 1..index].ends_with('\'') {
                    diagnostics.push(Diagnostic::error(
                        "S0002",
                        "unterminated string literal",
                        Span::new(source.id(), base + start, base + index),
                    ));
                } else {
                    push_token(
                        source,
                        tokens,
                        TokenKind::String,
                        base + start,
                        base + index,
                        attachment(line, start, index),
                    );
                }
            }
            b'>' if expression_start(line, start) => {
                if bytes.get(index + 1) == Some(&b'>') {
                    index += 2;
                    if index != bytes.len() {
                        diagnostics.push(Diagnostic::error(
                            "S0004",
                            "block string marker `>>` must be the final content on its line",
                            Span::new(source.id(), base + start, base + line.len()),
                        ));
                        break;
                    }
                    push_token(
                        source,
                        tokens,
                        TokenKind::BlockString,
                        base + start,
                        base + index,
                        attachment(line, start, index),
                    );
                    break;
                } else {
                    index = bytes.len();
                    push_token(
                        source,
                        tokens,
                        TokenKind::TailString,
                        base + start,
                        base + index,
                        attachment(line, start, index),
                    );
                    break;
                }
            }
            byte if is_joiner(byte) || matches!(byte, b'!' | b'<' | b'>' | b'%') => {
                index += 1;
                if index < bytes.len()
                    && bytes[index] == byte
                    && matches!(byte, b'+' | b'-' | b'<' | b'>')
                {
                    index += 1;
                } else if index < bytes.len() && bytes[index] == b'=' {
                    index += 1;
                }
                let text = &line[start..index];
                let kind = match text {
                    "++" => TokenKind::Increment,
                    "--" => TokenKind::Decrement,
                    _ => TokenKind::Operator,
                };
                let attached = attachment(line, start, index);
                if matches!(attached, Attachment::Left | Attachment::Both)
                    && !matches!(kind, TokenKind::Increment | TokenKind::Decrement)
                    && !matches!(text, ">" | ">=")
                {
                    diagnostics.push(Diagnostic::error(
                        "L0006",
                        format!("operator `{text}` cannot be left-attached; add a space before it"),
                        Span::new(source.id(), base + start, base + index),
                    ));
                }
                push_token(source, tokens, kind, base + start, base + index, attached);
            }
            other => {
                let width = line[start..].chars().next().map_or(1, char::len_utf8);
                diagnostics.push(Diagnostic::error(
                    "L0001",
                    format!("invalid source character `{}`", char::from(other)),
                    Span::new(source.id(), base + start, base + start + width),
                ));
                index += width;
            }
        }
    }
}

fn check_indent(
    source: &SourceFile,
    offset: usize,
    indent: &[u8],
    style: &mut Option<u8>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if indent.contains(&b' ') && indent.contains(&b'\t') {
        diagnostics.push(Diagnostic::error(
            "S0001",
            "mixed tabs and spaces in indentation",
            Span::new(source.id(), offset, offset + indent.len()),
        ));
    } else if let Some(first) = indent.first().copied() {
        match style {
            Some(selected) if *selected != first => diagnostics.push(Diagnostic::error(
                "S0001",
                "indentation style changes within the file",
                Span::new(source.id(), offset, offset + indent.len()),
            )),
            None => *style = Some(first),
            _ => {}
        }
    }
}

fn emit_indentation(
    source: &SourceFile,
    offset: usize,
    indent: usize,
    stack: &mut Vec<usize>,
    tokens: &mut Vec<Token>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let current = *stack.last().expect("indentation stack is never empty");
    if indent > current {
        stack.push(indent);
        push_token(
            source,
            tokens,
            TokenKind::Indent,
            offset,
            offset + indent,
            Attachment::Detached,
        );
    } else if indent < current {
        while stack.last().is_some_and(|level| *level > indent) {
            stack.pop();
            push_token(
                source,
                tokens,
                TokenKind::Dedent,
                offset + indent,
                offset + indent,
                Attachment::Detached,
            );
        }
        if stack.last() != Some(&indent) {
            diagnostics.push(Diagnostic::error(
                "L0004",
                "inconsistent dedent",
                Span::new(source.id(), offset, offset + indent),
            ));
        }
    }
}

fn is_joiner(byte: u8) -> bool {
    matches!(byte, b'+' | b'-' | b'*' | b'/' | b'%' | b'<' | b'>')
}

fn indentation_len(line: &str) -> usize {
    line.bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count()
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

fn push_token(
    source: &SourceFile,
    tokens: &mut Vec<Token>,
    kind: TokenKind,
    start: usize,
    end: usize,
    attachment: Attachment,
) {
    tokens.push(Token {
        kind,
        span: Span::new(source.id(), start, end),
        text: source.text()[start..end].to_owned(),
        attachment,
    });
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
