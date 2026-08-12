mod compiler;
pub mod diagnostic;
pub mod lexer;
pub mod tokens;
pub mod source;

pub use compiler::{Compilation, CompilationFailure, Program, compile};
pub use diagnostic::{Diagnostic, Severity};
pub use source::{SourceFile, Span};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
