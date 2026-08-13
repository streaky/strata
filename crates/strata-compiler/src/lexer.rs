use crate::tokens::{Attachment, LexedSource, Token, TokenKind, Trivia, TriviaKind};
use crate::{Diagnostic, SourceFile, Span};

/// Tokenizes one UTF-8 Strata source file.
///
/// # Errors
///
/// Returns every lexical diagnostic found while scanning the source.
#[expect(
    clippy::too_many_lines,
    reason = "top-level lexer state transitions remain visible"
)]
pub fn lex(source: &SourceFile) -> Result<LexedSource, Vec<Diagnostic>> {
    let text = source.text();
    let mut tokens = Vec::new();
    let mut trivia = Vec::new();
    let mut diagnostics = Vec::new();
    let mut logical_lines = Vec::new();
    let mut offset = 0;
    let mut block_string: Option<(usize, usize, Option<Vec<u8>>)> = None;
    let mut block_terminator: Option<(usize, usize)> = None;
    let mut block_comment_start = None;
    let mut indent_style = None;
    let mut indent_stack = vec![0];
    for raw in text.split_inclusive('\n') {
        let line = raw.trim_end_matches(['\n', '\r']);
        logical_lines.push((offset, line.to_owned()));
        let indent = indentation_len(line);
        let in_block_string = match &mut block_string {
            Some((_, token_index, _)) if line.trim().is_empty() => {
                extend_token(source, &mut tokens[*token_index], offset + line.len());
                true
            }
            Some((marker_indent, token_index, prefix @ None)) if indent > *marker_indent => {
                *prefix = Some(line.as_bytes()[..indent].to_vec());
                extend_token(source, &mut tokens[*token_index], offset + line.len());
                check_indent(
                    source,
                    offset,
                    &line.as_bytes()[..indent],
                    &mut indent_style,
                    &mut diagnostics,
                );
                true
            }
            Some((_, token_index, Some(prefix))) if line.as_bytes().starts_with(prefix) => {
                extend_token(source, &mut tokens[*token_index], offset + line.len());
                check_indent(
                    source,
                    offset,
                    &line.as_bytes()[..indent],
                    &mut indent_style,
                    &mut diagnostics,
                );
                true
            }
            Some(_) => {
                block_string = None;
                false
            }
            None => false,
        };
        if !in_block_string {
            if let Some((start, end)) = block_terminator.take() {
                push_token(
                    source,
                    &mut tokens,
                    TokenKind::Newline,
                    start,
                    end,
                    Attachment::Detached,
                );
            }
            if carries_code(line, indent, block_comment_start.is_some()) {
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
            let token_count = tokens.len();
            lex_line(
                source,
                line,
                offset,
                &mut tokens,
                &mut trivia,
                &mut diagnostics,
                &mut block_comment_start,
            );
            if let Some(relative_index) = tokens[token_count..]
                .iter()
                .position(|token| token.kind == TokenKind::BlockString)
            {
                block_string = Some((indent, token_count + relative_index, None));
            }
        }
        if raw.ends_with('\n') {
            if block_string.is_some() {
                block_terminator = Some((offset + line.len(), offset + raw.len()));
            } else {
                push_token(
                    source,
                    &mut tokens,
                    TokenKind::Newline,
                    offset + line.len(),
                    offset + raw.len(),
                    Attachment::Detached,
                );
            }
        }
        offset += raw.len();
    }
    if let Some((start, end)) = block_terminator {
        push_token(
            source,
            &mut tokens,
            TokenKind::Newline,
            start,
            end,
            Attachment::Detached,
        );
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

#[expect(
    clippy::too_many_lines,
    reason = "single scanner loop makes byte advancement auditable"
)]
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
                index = scan_number(bytes, index);
                let text = &line[start..index];
                if is_number(text) {
                    push_token(
                        source,
                        tokens,
                        TokenKind::Number,
                        base + start,
                        base + index,
                        attachment(line, start, index),
                    );
                } else {
                    diagnostics.push(Diagnostic::error(
                        "L0009",
                        format!(
                            "invalid numeric literal `{text}`; version one accepts decimal digits, one `.` fraction, `0x` hexadecimal, and `_` between digits"
                        ),
                        Span::new(source.id(), base + start, base + index),
                    ));
                }
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
                let mut terminated = false;
                while index < bytes.len() {
                    if bytes[index] == b'\'' && !escaped {
                        index += 1;
                        terminated = true;
                        break;
                    }
                    escaped = bytes[index] == b'\\' && !escaped;
                    if bytes[index] != b'\\' {
                        escaped = false;
                    }
                    index += 1;
                }
                if terminated {
                    push_token(
                        source,
                        tokens,
                        TokenKind::String,
                        base + start,
                        base + index,
                        attachment(line, start, index),
                    );
                } else {
                    diagnostics.push(Diagnostic::error(
                        "L0007",
                        "unterminated string literal",
                        Span::new(source.id(), base + start, base + index),
                    ));
                }
            }
            b'>' if expression_start(tokens) => {
                if bytes.get(index + 1) == Some(&b'>') {
                    index += 2;
                    if index != bytes.len() {
                        diagnostics.push(Diagnostic::error(
                            "L0008",
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
                }
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
            byte if is_joiner(byte)
                || matches!(byte, b'!' | b'<' | b'>' | b'%' | b'&' | b'^' | b'~') =>
            {
                index += 1;
                if index < bytes.len()
                    && ((bytes[index] == byte && matches!(byte, b'+' | b'-' | b'<' | b'>'))
                        || bytes[index] == b'=')
                {
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
            _ => {
                let character = line[start..].chars().next().expect("index is in bounds");
                let width = character.len_utf8();
                diagnostics.push(Diagnostic::error(
                    "L0001",
                    format!("invalid source character `{character}`"),
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
            "L0003",
            "mixed tabs and spaces in indentation",
            Span::new(source.id(), offset, offset + indent.len()),
        ));
    } else if let Some(first) = indent.first().copied() {
        match style {
            Some(selected) if *selected != first => diagnostics.push(Diagnostic::error(
                "L0003",
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

/// Reports whether a line contributes source outside comments, which is the only
/// case where it participates in indentation.
fn carries_code(line: &str, indent: usize, in_block_comment: bool) -> bool {
    let mut rest = if in_block_comment { line } else { &line[indent..] };
    if in_block_comment {
        let Some(end) = rest.find("*/") else {
            return false;
        };
        rest = &rest[end + 2..];
    }
    loop {
        rest = rest.trim_start();
        if rest.is_empty() || rest.starts_with('#') || rest.starts_with("//") {
            return false;
        }
        let Some(after) = rest.strip_prefix("/*") else {
            return true;
        };
        let Some(end) = after.find("*/") else {
            return false;
        };
        rest = &after[end + 2..];
    }
}

/// Consumes the maximal run a numeric literal could occupy, so a malformed
/// spelling is reported as one literal instead of splitting into a name.
fn scan_number(bytes: &[u8], start: usize) -> usize {
    let index = digit_run(bytes, start);
    if bytes.get(index) == Some(&b'.') && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
        return digit_run(bytes, index + 1);
    }
    index
}

fn digit_run(bytes: &[u8], start: usize) -> usize {
    let mut index = start;
    while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
    }
    index
}

fn is_number(text: &str) -> bool {
    if let Some(digits) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return is_digit_run(digits, u8::is_ascii_hexdigit);
    }
    match text.split_once('.') {
        Some((whole, fraction)) => {
            is_digit_run(whole, u8::is_ascii_digit) && is_digit_run(fraction, u8::is_ascii_digit)
        }
        None => is_digit_run(text, u8::is_ascii_digit),
    }
}

fn is_digit_run(text: &str, admitted: fn(&u8) -> bool) -> bool {
    !text.is_empty()
        && !text.starts_with('_')
        && !text.ends_with('_')
        && !text.contains("__")
        && text.bytes().all(|byte| admitted(&byte) || byte == b'_')
}

fn is_joiner(byte: u8) -> bool {
    matches!(byte, b'+' | b'-' | b'*' | b'/' | b'%' | b'<' | b'>')
}

fn indentation_len(line: &str) -> usize {
    line.bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count()
}

fn expression_start(tokens: &[Token]) -> bool {
    tokens.last().is_none_or(|token| {
        matches!(
            token.kind,
            TokenKind::Newline
                | TokenKind::Indent
                | TokenKind::Dedent
                | TokenKind::Assign
                | TokenKind::Semicolon
                | TokenKind::Comma
                | TokenKind::OpenParen
                | TokenKind::OpenBracket
                | TokenKind::OpenBrace
                | TokenKind::Operator
        )
    })
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

fn extend_token(source: &SourceFile, token: &mut Token, end: usize) {
    token.span = Span::new(source.id(), token.span.start, end);
    source.text()[token.span.start..end].clone_into(&mut token.text);
}
