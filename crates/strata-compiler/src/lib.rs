pub mod diagnostic;
pub mod source;

pub use diagnostic::{Diagnostic, Label, Severity};
pub use source::{SourceFile, Span};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
