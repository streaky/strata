use crate::source::{SourceFile, Span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub primary: Span,
}

impl Diagnostic {
    #[must_use]
    pub fn error(code: &'static str, message: impl Into<String>, primary: Span) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            primary,
        }
    }

    #[must_use]
    pub fn render(&self, source: &SourceFile) -> String {
        let (line, column) = source.line_column(self.primary.start);
        format!(
            "{}:{}:{}: error[{}]: {}\n",
            source.path().display(),
            line,
            column,
            self.code,
            self.message
        )
    }
}
