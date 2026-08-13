mod compiler;
pub mod diagnostic;
pub mod lexer;
pub mod package;
pub mod parser;
pub mod source;
pub mod syntax;
pub mod tokens;

pub use compiler::{Compilation, CompilationFailure, Program, compile};
pub use diagnostic::{Diagnostic, Severity};
pub use package::{IMPLICIT_PACKAGE_ID, MANIFEST_FILE_NAME, Package, PackageLoadError, SourceUnit};
pub use source::{SourceFile, Span};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
