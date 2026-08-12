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
    pub primary: Option<Span>,
}

impl Diagnostic {
    #[must_use]
    pub fn error(code: &'static str, message: impl Into<String>, primary: Span) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            primary: Some(primary),
        }
    }

    #[must_use]
    pub fn unlocated_error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            primary: None,
        }
    }

    #[must_use]
    pub fn render(&self, source: &SourceFile) -> String {
        if let Some(primary) = self.primary {
            let (line, column) = source.line_column(primary.start);
            format!(
                "{}:{}:{}: error[{}]: {}\n",
                source.path().display(),
                line,
                column,
                self.code,
                self.message
            )
        } else {
            format!(
                "{}: error[{}]: {}\n",
                source.path().display(),
                self.code,
                self.message
            )
        }
    }
}
