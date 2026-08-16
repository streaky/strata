use std::path::PathBuf;

use crate::{
    Diagnostic, Package, SourceFile, Span,
    semantics::{self, SymbolKind},
};

#[derive(Clone, Debug)]
pub struct Compilation {
    pub source: SourceFile,
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

/// Compiles one Terrane source file as an implicit, stable-identity package.
///
/// # Errors
///
/// Returns every source-oriented diagnostic produced by the shared frontend.
pub fn compile(path: impl Into<PathBuf>, text: String) -> Result<Compilation, CompilationFailure> {
    compile_package(&Package::implicit(path, text))
}

/// Compiles every manifest-discovered source unit through the shared frontend.
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
                .unwrap_or_else(|| Span::new(semantic.units[0].source.id(), 0, 0));
            let source = semantic
                .units
                .iter()
                .find(|unit| unit.source.id() == span.file)
                .map_or(&semantic.units[0].source, |unit| &unit.source);
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
        .unwrap_or_else(|| Span::new(semantic.units[0].source.id(), 0, 0));
    let unit = semantic
        .units
        .iter()
        .find(|unit| unit.source.id() == entry_span.file)
        .unwrap_or(&semantic.units[0]);
    let source = &unit.source;
    let rust = crate::lowering::emit(&semantic);
    Ok(Compilation {
        source: (*source).clone(),
        rust,
    })
}
