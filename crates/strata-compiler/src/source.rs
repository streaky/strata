use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub file: u32,
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(file: u32, start: usize, end: usize) -> Self {
        Self { file, start, end }
    }
}

#[derive(Clone, Debug)]
pub struct SourceFile {
    id: u32,
    path: PathBuf,
    text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    #[must_use]
    pub fn new(id: u32, path: PathBuf, text: String) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(offset, _)| offset + 1));
        Self {
            id,
            path,
            text,
            line_starts,
        }
    }

    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn line_column(&self, offset: usize) -> (usize, usize) {
        let line_index = self
            .line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line_index];
        (
            line_index + 1,
            self.text[line_start..offset].chars().count() + 1,
        )
    }
}
