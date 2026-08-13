use std::path::{Path, PathBuf};

use crate::{
    Diagnostic, Package, SourceFile, Span,
    semantics::{self, SymbolKind},
    syntax::SyntaxTree,
};

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

#[derive(Clone, Debug)]
pub struct CompilationFailure {
    pub source: SourceFile,
    pub diagnostics: Vec<Diagnostic>,
}

impl std::ops::Deref for CompilationFailure {
    type Target = [Diagnostic];

    fn deref(&self) -> &Self::Target {
        &self.diagnostics
    }
}

/// Compiles one Strata source file as an implicit, stable-identity package.
///
/// # Errors
///
/// Returns every source-oriented diagnostic produced by the shared frontend.
pub fn compile(path: impl Into<PathBuf>, text: String) -> Result<Compilation, CompilationFailure> {
    compile_package(&Package::implicit(path, text))
}

/// Compiles every manifest-enumerated source unit through the shared frontend.
///
/// # Errors
///
/// Returns diagnostics from the first source unit that fails. All units are
/// parsed before semantic projection, in deterministic package order.
pub fn compile_package(package: &Package) -> Result<Compilation, CompilationFailure> {
    let semantic = semantics::analyze(package).map_err(|failure| CompilationFailure {
        source: failure.source,
        diagnostics: failure.diagnostics,
    })?;
    let entry_points = semantic
        .namespaces
        .values()
        .filter_map(|namespace| namespace.ordinary.get("main"))
        .filter(|symbol| symbol.kind == SymbolKind::Function)
        .collect::<Vec<_>>();
    let entry = match entry_points.as_slice() {
        [] => {
            let source = &semantic.units[0].source;
            return Err(CompilationFailure {
                source: source.clone(),
                diagnostics: vec![Diagnostic::error(
                    "S2015",
                    "package has no `main` function",
                    Span::new(source.id(), 0, 0),
                )],
            });
        }
        [entry] => *entry,
        [_, ambiguous, ..] => {
            let span = ambiguous
                .declaration_span
                .expect("source declarations carry their source span");
            let source = &semantic
                .units
                .iter()
                .find(|unit| unit.source.id() == span.file)
                .expect("entry declaration belongs to an analyzed source unit")
                .source;
            return Err(CompilationFailure {
                source: source.clone(),
                diagnostics: vec![Diagnostic::error(
                    "S2016",
                    "package has more than one `main` function",
                    span,
                )],
            });
        }
    };
    let entry_span = entry
        .declaration_span
        .expect("source declarations carry their source span");
    let unit = semantic
        .units
        .iter()
        .find(|unit| unit.source.id() == entry_span.file)
        .expect("entry declaration belongs to an analyzed source unit");
    let source = &unit.source;
    let program = project_bootstrap_program(source, &unit.tree).map_err(|diagnostics| {
        CompilationFailure {
            source: source.clone(),
            diagnostics,
        }
    })?;
    let program = resolve(program);
    let ir = lower(&program);
    let rust = emit_rust(&ir, source);
    Ok(Compilation {
        source: (*source).clone(),
        program,
        rust,
    })
}

fn project_bootstrap_program(
    source: &SourceFile,
    syntax: &SyntaxTree,
) -> Result<SyntaxProgram, Vec<Diagnostic>> {
    let mut lines = syntax
        .lexed
        .logical_lines
        .iter()
        .map(|(offset, line)| (*offset, line.as_str()))
        .peekable();
    let mut errors = Vec::new();
    let mut namespace = None;
    let mut output_path = None;
    let mut print_binding = None;
    let mut in_main = false;
    let mut message = None;
    let mut main_statement_seen = false;

    while let Some((offset, line)) = lines.next() {
        let indent_len = indentation_len(line);
        let raw_content = &line[indent_len..];
        let content = raw_content.trim_end();
        if content.is_empty() || content.starts_with('#') || content.starts_with("//") {
            continue;
        }
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
            main_statement_seen = true;
            if message.is_some() {
                errors.push(Diagnostic::error(
                    "S0005",
                    "only one statement is supported in `main` by this compiler version",
                    Span::new(source.id(), offset + indent_len, offset + line.len()),
                ));
                continue;
            }
            message = parse_print_value(
                source,
                &mut lines,
                &raw_content[7..],
                LineContext {
                    offset,
                    line,
                    indent_len,
                },
                &mut errors,
            );
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

    add_missing_diagnostics(
        source,
        namespace.as_deref(),
        output_path.as_deref(),
        print_binding.as_deref(),
        in_main,
        main_statement_seen,
        &mut errors,
    );
    finish_parse(namespace, output_path, print_binding, message, errors)
}

#[derive(Clone, Copy)]
struct LineContext<'source> {
    offset: usize,
    line: &'source str,
    indent_len: usize,
}

fn add_missing_diagnostics(
    source: &SourceFile,
    namespace: Option<&str>,
    output_path: Option<&str>,
    print_binding: Option<&str>,
    main: bool,
    main_statement: bool,
    errors: &mut Vec<Diagnostic>,
) {
    let end = source.text().len();
    let mut missing = |condition: bool, code: &'static str, message: &'static str, span: Span| {
        if !condition {
            errors.push(Diagnostic::error(code, message, span));
        }
    };
    missing(
        namespace.is_some(),
        "S0005",
        "missing namespace declaration",
        Span::new(source.id(), 0, 0),
    );
    missing(
        output_path.is_some(),
        "S0003",
        "missing import for `.print`",
        Span::new(source.id(), end, end),
    );
    missing(
        print_binding.is_some(),
        "S0003",
        "missing ordinary `print` binding",
        Span::new(source.id(), end, end),
    );
    missing(
        main,
        "S0005",
        "missing `function main`",
        Span::new(source.id(), end, end),
    );
    missing(
        main_statement,
        "S0005",
        "main must invoke `print`",
        Span::new(source.id(), end, end),
    );
}

fn finish_parse(
    namespace: Option<String>,
    output_path: Option<String>,
    print_binding: Option<String>,
    message: Option<String>,
    errors: Vec<Diagnostic>,
) -> Result<SyntaxProgram, Vec<Diagnostic>> {
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

fn parse_print_value<'source>(
    source: &SourceFile,
    lines: &mut std::iter::Peekable<impl Iterator<Item = (usize, &'source str)>>,
    value: &str,
    context: LineContext<'_>,
    errors: &mut Vec<Diagnostic>,
) -> Option<String> {
    let LineContext {
        offset,
        line,
        indent_len,
    } = context;
    if value == ">>" {
        parse_block_string(source, lines, indent_len, errors)
    } else if value.starts_with(">>") {
        errors.push(Diagnostic::error(
            "S0004",
            "block string marker `>>` must be the final content on its line",
            Span::new(source.id(), offset + indent_len + 7, offset + line.len()),
        ));
        None
    } else if let Some(value) = value.strip_prefix('>') {
        Some(value.to_owned())
    } else if let Some(quoted) = value.strip_prefix('\'') {
        parse_quoted_value(source, quoted, offset, line, indent_len, errors)
    } else {
        errors.push(Diagnostic::error(
            "S0004",
            "print expects one text argument",
            Span::new(source.id(), offset + indent_len, offset + line.len()),
        ));
        None
    }
}

fn parse_quoted_value(
    source: &SourceFile,
    quoted: &str,
    offset: usize,
    line: &str,
    indent_len: usize,
    errors: &mut Vec<Diagnostic>,
) -> Option<String> {
    let Some(closing) = closing_quote(quoted) else {
        errors.push(Diagnostic::error(
            "S0002",
            "unterminated string literal",
            Span::new(source.id(), offset + indent_len + 7, offset + line.len()),
        ));
        return None;
    };
    if closing + 1 != quoted.len() {
        errors.push(Diagnostic::error(
            "S0004",
            "content after closing string quote is not supported",
            Span::new(
                source.id(),
                offset + indent_len + 9 + closing,
                offset + line.len(),
            ),
        ));
        return None;
    }
    Some(unescape(
        &quoted[..closing],
        source,
        offset + indent_len + 8,
        errors,
    ))
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
    }
    None
}

fn indentation_len(line: &str) -> usize {
    line.bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count()
}

fn parse_block_string<'source>(
    source: &SourceFile,
    lines: &mut std::iter::Peekable<impl Iterator<Item = (usize, &'source str)>>,
    statement_indent: usize,
    errors: &mut Vec<Diagnostic>,
) -> Option<String> {
    let mut block_indent = None;
    let mut content_lines = Vec::new();
    while let Some((_, line)) = lines.peek().copied() {
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
