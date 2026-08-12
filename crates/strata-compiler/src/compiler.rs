use std::path::{Path, PathBuf};

use crate::{Diagnostic, SourceFile, Span};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Program {
    pub namespace: String,
    pub output_path: String,
    pub print_binding: String,
    pub message: String,
}

#[derive(Clone, Debug)]
struct SyntaxProgram {
    namespace: String,
    output_path: String,
    print_binding: String,
    message: String,
}

struct LexedSource<'source> {
    logical_lines: Vec<(usize, &'source str)>,
}

struct RustIr<'program> {
    namespace: &'program str,
    message: &'program str,
}

#[derive(Clone, Debug)]
pub struct Compilation {
    pub source: SourceFile,
    pub program: Program,
    pub rust: String,
}

/// Compiles one Strata source file through parsing, resolution, and Rust lowering.
///
/// # Errors
///
/// Returns every source-oriented diagnostic produced by the shared frontend.
pub fn compile(path: impl Into<PathBuf>, text: String) -> Result<Compilation, Vec<Diagnostic>> {
    let source = SourceFile::new(0, path.into(), text);
    let tokens = lex(&source);
    let syntax = parse(&source, &tokens)?;
    let program = resolve(syntax);
    let ir = lower(&program);
    let rust = emit_rust(&ir, &source);
    Ok(Compilation {
        source,
        program,
        rust,
    })
}

fn lex(source: &SourceFile) -> LexedSource<'_> {
    let logical_lines = source
        .text()
        .split_inclusive('\n')
        .scan(0, |offset, raw| {
            let start = *offset;
            *offset += raw.len();
            Some((start, raw.trim_end_matches(['\n', '\r'])))
        })
        .collect();
    LexedSource { logical_lines }
}

#[allow(clippy::too_many_lines)]
fn parse(source: &SourceFile, tokens: &LexedSource<'_>) -> Result<SyntaxProgram, Vec<Diagnostic>> {
    let mut lines = tokens.logical_lines.iter().copied().peekable();
    let mut errors = Vec::new();
    let mut namespace = None;
    let mut output_path = None;
    let mut print_binding = None;
    let mut in_main = false;
    let mut message = None;
    let mut indent_style: Option<u8> = None;

    while let Some((offset, line)) = lines.next() {
        let indent_len = indentation_len(line);
        let raw_content = &line[indent_len..];
        let content = raw_content.trim_end();
        if content.is_empty() || content.starts_with('#') || content.starts_with("//") {
            continue;
        }
        check_indentation(
            source,
            offset,
            &line.as_bytes()[..indent_len],
            &mut indent_style,
            &mut errors,
        );
        if content.starts_with("namespace ") && indent_len == 0 {
            if namespace.is_some() {
                errors.push(duplicate(source, offset, line, "namespace declaration"));
            } else {
                namespace = Some(content[10..].trim().to_owned());
            }
        } else if content == "from /core output import .print" && indent_len == 0 {
            if output_path.is_some() {
                errors.push(duplicate(source, offset, line, "output import"));
            } else {
                output_path = Some("/core output".to_owned());
            }
        } else if content == "print = .print" && indent_len == 0 {
            if print_binding.is_some() {
                errors.push(duplicate(source, offset, line, "print binding"));
            } else {
                print_binding = Some("print".to_owned());
            }
        } else if content == "function main" && indent_len == 0 {
            if in_main {
                errors.push(duplicate(source, offset, line, "`main` function"));
            } else {
                in_main = true;
            }
        } else if in_main && indent_len > 0 && raw_content.starts_with("print; ") {
            if message.is_some() {
                errors.push(Diagnostic::error(
                    "S0005",
                    "only one statement is supported in `main` by this compiler version",
                    Span::new(source.id(), offset + indent_len, offset + line.len()),
                ));
                continue;
            }
            let value = &raw_content[7..];
            if value == ">>" {
                message = parse_block_string(
                    source,
                    &mut lines,
                    indent_len,
                    &mut indent_style,
                    &mut errors,
                );
            } else if value.starts_with(">>") {
                errors.push(Diagnostic::error(
                    "S0004",
                    "block string marker `>>` must be the final content on its line",
                    Span::new(source.id(), offset + indent_len + 7, offset + line.len()),
                ));
            } else if let Some(value) = value.strip_prefix('>') {
                message = Some(value.to_owned());
            } else if let Some(quoted) = value.strip_prefix('\'') {
                if let Some(closing) = closing_quote(quoted) {
                    if closing + 1 == quoted.len() {
                        message = Some(unescape(
                            &quoted[..closing],
                            source,
                            offset + indent_len + 8,
                            &mut errors,
                        ));
                    } else {
                        errors.push(Diagnostic::error(
                            "S0004",
                            "content after closing string quote is not supported",
                            Span::new(
                                source.id(),
                                offset + indent_len + 9 + closing,
                                offset + line.len(),
                            ),
                        ));
                    }
                } else {
                    errors.push(Diagnostic::error(
                        "S0002",
                        "unterminated string literal",
                        Span::new(source.id(), offset + indent_len + 7, offset + line.len()),
                    ));
                }
            } else {
                errors.push(Diagnostic::error(
                    "S0004",
                    "print expects one text argument",
                    Span::new(source.id(), offset + indent_len, offset + line.len()),
                ));
            }
        } else if content.contains("= .") {
            let object = content
                .split_once("= .")
                .map_or("", |(_, object)| object.trim());
            errors.push(Diagnostic::error(
                "S0003",
                format!("unresolved object `.{object}`"),
                Span::new(source.id(), offset + indent_len, offset + line.len()),
            ));
        } else {
            errors.push(Diagnostic::error(
                "S0005",
                "unsupported syntax in the first compiler milestone",
                Span::new(source.id(), offset + indent_len, offset + line.len()),
            ));
        }
    }

    let end = source.text().len();
    if namespace.is_none() {
        errors.push(Diagnostic::error(
            "S0005",
            "missing namespace declaration",
            Span::new(source.id(), 0, 0),
        ));
    }
    if output_path.is_none() {
        errors.push(Diagnostic::error(
            "S0003",
            "missing import for `.print`",
            Span::new(source.id(), end, end),
        ));
    }
    if print_binding.is_none() {
        errors.push(Diagnostic::error(
            "S0003",
            "missing ordinary `print` binding",
            Span::new(source.id(), end, end),
        ));
    }
    if !in_main {
        errors.push(Diagnostic::error(
            "S0005",
            "missing `function main`",
            Span::new(source.id(), end, end),
        ));
    }
    if message.is_none()
        && !errors
            .iter()
            .any(|error| error.code == "S0002" || error.code == "S0004")
    {
        errors.push(Diagnostic::error(
            "S0005",
            "main must invoke `print`",
            Span::new(source.id(), end, end),
        ));
    }

    if errors.is_empty() {
        Ok(SyntaxProgram {
            namespace: namespace.unwrap(),
            output_path: output_path.unwrap(),
            print_binding: print_binding.unwrap(),
            message: message.unwrap(),
        })
    } else {
        Err(errors)
    }
}
fn duplicate(source: &SourceFile, offset: usize, line: &str, description: &str) -> Diagnostic {
    Diagnostic::error(
        "S0005",
        format!("duplicate {description}"),
        Span::new(source.id(), offset, offset + line.len()),
    )
}

fn closing_quote(value: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if character == '\'' && !escaped {
            return Some(index);
        }
        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }
    None
}

fn indentation_len(line: &str) -> usize {
    line.bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count()
}

fn check_indentation(
    source: &SourceFile,
    offset: usize,
    indent: &[u8],
    indent_style: &mut Option<u8>,
    errors: &mut Vec<Diagnostic>,
) {
    if indent.contains(&b' ') && indent.contains(&b'\t') {
        errors.push(Diagnostic::error(
            "S0001",
            "mixed tabs and spaces in indentation",
            Span::new(source.id(), offset, offset + indent.len()),
        ));
        return;
    }
    if let Some(first) = indent.first().copied() {
        match *indent_style {
            Some(style) if style != first => errors.push(Diagnostic::error(
                "S0001",
                "indentation style changes within the file",
                Span::new(source.id(), offset, offset + indent.len()),
            )),
            None => *indent_style = Some(first),
            _ => {}
        }
    }
}

fn parse_block_string<'source>(
    source: &SourceFile,
    lines: &mut std::iter::Peekable<impl Iterator<Item = (usize, &'source str)>>,
    statement_indent: usize,
    indent_style: &mut Option<u8>,
    errors: &mut Vec<Diagnostic>,
) -> Option<String> {
    let mut block_indent = None;
    let mut content_lines = Vec::new();
    while let Some((offset, line)) = lines.peek().copied() {
        let indent_len = indentation_len(line);
        if line.trim().is_empty() {
            lines.next();
            if block_indent.is_some() {
                content_lines.push(String::new());
            }
            continue;
        }
        let required = match block_indent {
            Some(required) => required,
            None if indent_len > statement_indent => {
                block_indent = Some(indent_len);
                indent_len
            }
            None => break,
        };
        if indent_len < required {
            break;
        }
        lines.next();
        check_indentation(
            source,
            offset,
            &line.as_bytes()[..indent_len],
            indent_style,
            errors,
        );
        content_lines.push(line[required..].to_owned());
    }
    if block_indent.is_none() {
        errors.push(Diagnostic::error(
            "S0004",
            "block string requires an indented nonblank line",
            Span::new(source.id(), source.text().len(), source.text().len()),
        ));
        None
    } else {
        Some(content_lines.join("\n"))
    }
}

fn unescape(
    value: &str,
    source: &SourceFile,
    start: usize,
    errors: &mut Vec<Diagnostic>,
) -> String {
    let mut result = String::new();
    let mut chars = value.char_indices();
    while let Some((index, character)) = chars.next() {
        if character != '\\' {
            result.push(character);
            continue;
        }
        match chars.next() {
            Some((_, 'n')) => result.push('\n'),
            Some((_, 'r')) => result.push('\r'),
            Some((_, 't')) => result.push('\t'),
            Some((_, '\\')) => result.push('\\'),
            Some((_, '\'')) => result.push('\''),
            Some((_, other)) => errors.push(Diagnostic::error(
                "S0002",
                format!("unsupported escape `\\{other}`"),
                Span::new(source.id(), start + index, start + index + 2),
            )),
            None => errors.push(Diagnostic::error(
                "S0002",
                "unterminated escape sequence",
                Span::new(source.id(), start + index, start + index + 1),
            )),
        }
    }
    result
}

fn resolve(syntax: SyntaxProgram) -> Program {
    Program {
        namespace: syntax.namespace,
        output_path: syntax.output_path,
        print_binding: syntax.print_binding,
        message: syntax.message,
    }
}

fn lower(program: &Program) -> RustIr<'_> {
    RustIr {
        namespace: &program.namespace,
        message: &program.message,
    }
}

fn emit_rust(ir: &RustIr<'_>, source: &SourceFile) -> String {
    format!(
        "// Generated deterministically by Strata {}.\n// Source: {}\n// Namespace: {}\nfn main() {{\n    println!(\"{{}}\", {:?});\n}}\n",
        crate::VERSION,
        display_path(source.path()),
        ir.namespace,
        ir.message
    )
}

fn display_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<source>")
        .to_owned()
}
