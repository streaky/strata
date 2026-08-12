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
    let program = parse(&source)?;
    let rust = lower(&program, &source);
    Ok(Compilation {
        source,
        program,
        rust,
    })
}

#[allow(clippy::too_many_lines)]
fn parse(source: &SourceFile) -> Result<Program, Vec<Diagnostic>> {
    let mut lines = source.text().split_inclusive('\n').scan(0, |offset, raw| {
        let start = *offset;
        *offset += raw.len();
        Some((start, raw.trim_end_matches(['\n', '\r'])))
    });
    let mut errors = Vec::new();
    let mut namespace = None;
    let mut output_path = None;
    let mut print_binding = None;
    let mut in_main = false;
    let mut message = None;
    let mut indent_style: Option<u8> = None;

    for (offset, line) in &mut lines {
        let indent_len = line
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count();
        let indent = &line.as_bytes()[..indent_len];
        if indent.contains(&b' ') && indent.contains(&b'\t') {
            errors.push(Diagnostic::error(
                "S0001",
                "mixed tabs and spaces in indentation",
                Span::new(source.id(), offset, offset + indent_len),
            ));
            continue;
        }
        if let Some(first) = indent.first().copied() {
            match indent_style {
                Some(style) if style != first => errors.push(Diagnostic::error(
                    "S0001",
                    "indentation style changes within the file",
                    Span::new(source.id(), offset, offset + indent_len),
                )),
                None => indent_style = Some(first),
                _ => {}
            }
        }
        let content = line[indent_len..].trim_end();
        if content.is_empty() || content.starts_with('#') || content.starts_with("//") {
            continue;
        }
        if content.starts_with("namespace ") && indent_len == 0 {
            namespace = Some(content[10..].trim().to_owned());
        } else if content == "from /core output import .print" && indent_len == 0 {
            output_path = Some("/core output".to_owned());
        } else if content == "print = .print" && indent_len == 0 {
            print_binding = Some("print".to_owned());
        } else if content == "function main" && indent_len == 0 {
            in_main = true;
        } else if in_main && indent_len > 0 && content.starts_with("print; ") {
            let value = &content[7..];
            if value.starts_with('\'') {
                if value.len() >= 2 && value.ends_with('\'') {
                    message = Some(unescape(
                        &value[1..value.len() - 1],
                        source,
                        offset + indent_len + 8,
                        &mut errors,
                    ));
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
                    "print expects one quoted string argument",
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
        Ok(Program {
            namespace: namespace.unwrap(),
            output_path: output_path.unwrap(),
            print_binding: print_binding.unwrap(),
            message: message.unwrap(),
        })
    } else {
        Err(errors)
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

fn lower(program: &Program, source: &SourceFile) -> String {
    format!(
        "// Generated deterministically by Strata {}.\n// Source: {}\n// Namespace: {}\nfn main() {{\n    println!(\"{{}}\", {:?});\n}}\n",
        crate::VERSION,
        display_path(source.path()),
        program.namespace,
        program.message
    )
}

fn display_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<source>")
        .to_owned()
}
