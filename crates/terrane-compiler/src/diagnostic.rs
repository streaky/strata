use std::fmt::Write as _;

use crate::source::{SourceFile, Span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub primary: Option<Span>,
    pub help: Option<String>,
}

impl Diagnostic {
    #[must_use]
    pub fn error(code: &'static str, message: impl Into<String>, primary: Span) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            primary: Some(primary),
            help: None,
        }
    }
    #[must_use]
    pub fn warning(code: &'static str, message: impl Into<String>, primary: Span) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
            primary: Some(primary),
            help: None,
        }
    }

    #[must_use]
    pub fn unlocated_error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            primary: None,
            help: None,
        }
    }

    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    #[must_use]
    pub fn render(&self, source: &SourceFile) -> String {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let mut rendered = if let Some(primary) = self.primary {
            let (line, column) = source.line_column(primary.start);
            format!(
                "{}:{}:{}: {severity}[{}]: {}\n",
                source.path().display(),
                line,
                column,
                self.code,
                self.message
            )
        } else {
            format!(
                "{}: {severity}[{}]: {}\n",
                source.path().display(),
                self.code,
                self.message
            )
        };
        if let Some(help) = &self.help {
            let _ = writeln!(rendered, "  help: {help}");
        }
        rendered
    }
}
