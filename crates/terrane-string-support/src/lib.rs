use unicode_segmentation::UnicodeSegmentation;

/// Returns the number of user-perceived characters using Unicode extended
/// grapheme-cluster boundaries.
#[must_use]
pub fn length(value: &str) -> usize {
    value.graphemes(true).count()
}

/// Iterates over owned user-perceived characters.
pub fn graphemes(value: &str) -> impl Iterator<Item = String> + '_ {
    value.graphemes(true).map(String::from)
}

/// Returns the number of Unicode scalar values.
#[must_use]
pub fn scalar_length(value: &str) -> usize {
    value.chars().count()
}

/// Returns the number of bytes in the UTF-8 encoding.
#[must_use]
pub const fn byte_length(value: &str) -> usize {
    value.len()
}
