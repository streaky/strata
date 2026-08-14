mod compiler;
pub mod diagnostic;
pub mod display;
pub mod highlight;
pub mod lexer;
mod lowering;
pub mod package;
pub mod parser;
pub mod semantics;
pub mod source;
pub mod syntax;
pub mod tokens;
pub mod types;

pub use compiler::{Compilation, CompilationFailure, compile, compile_package};
pub use diagnostic::{Diagnostic, Severity};
pub use package::{IMPLICIT_PACKAGE_ID, MANIFEST_FILE_NAME, Package, PackageLoadError, SourceUnit};
pub use semantics::{
    BOOTSTRAP_VERSION, EvaluationKind, EvaluationStep, FunctionContract, Namespace,
    ParameterContract, SemanticFailure, SemanticPackage, SemanticUnit, Symbol, TypedBinding,
    ValueType, Visibility, analyze,
};
pub use source::{SourceFile, Span};
pub use types::ScalarType;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
