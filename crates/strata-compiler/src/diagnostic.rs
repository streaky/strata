use std::fmt::Write as _;

use crate::source::{SourceFile, Span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub primary: Span,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    #[must_use]
    pub fn error(code: &'static str, message: impl Into<String>, primary: Span) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            primary,
            labels: Vec::new(),
            notes: Vec::new(),
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
        let (line, column) = source.line_column(self.primary.start);
        let mut rendered = format!(
            "{}:{}:{}: error[{}]: {}\n",
            source.path().display(),
            line,
            column,
            self.code,
            self.message
        );
        for note in &self.notes {
            let _ = writeln!(rendered, "  note: {note}");
        }
        if let Some(help) = &self.help {
            let _ = writeln!(rendered, "  help: {help}");
        }
        rendered
    }
}
