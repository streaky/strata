use std::sync::Arc;
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Utf32Le,
    Utf32Be,
}
impl Encoding {
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Utf8 => "utf8",
            Self::Utf16Le => "utf16-le",
            Self::Utf16Be => "utf16-be",
            Self::Utf32Le => "utf32-le",
            Self::Utf32Be => "utf32-be",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodeError {
    pub encoding: Encoding,
    pub byte_offset: usize,
}
impl std::fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            ".decode-error: invalid {} sequence at byte offset {}",
            self.encoding.source_name(),
            self.byte_offset
        )
    }
}

impl std::error::Error for DecodeError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextRange {
    source: Arc<str>,
    start: usize,
    end: usize,
}

impl TextRange {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.source[self.start..self.end]
    }

    #[must_use]
    pub fn byte_start(&self) -> usize {
        self.start
    }

    #[must_use]
    pub fn byte_end(&self) -> usize {
        self.end
    }

    #[must_use]
    pub fn scalar_start(&self) -> usize {
        self.source[..self.start].chars().count()
    }

    #[must_use]
    pub fn scalar_end(&self) -> usize {
        self.source[..self.end].chars().count()
    }

    #[must_use]
    pub fn grapheme_start(&self) -> usize {
        self.source[..self.start].graphemes(true).count()
    }

    #[must_use]
    pub fn grapheme_end(&self) -> usize {
        self.source[..self.end].graphemes(true).count()
    }
}

#[must_use]
pub fn encode(value: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => value.as_bytes().to_vec(),
        Encoding::Utf16Le | Encoding::Utf16Be => value
            .encode_utf16()
            .flat_map(|unit| match encoding {
                Encoding::Utf16Le => unit.to_le_bytes(),
                Encoding::Utf16Be => unit.to_be_bytes(),
                _ => unreachable!(),
            })
            .collect(),
        Encoding::Utf32Le | Encoding::Utf32Be => value
            .chars()
            .flat_map(|scalar| match encoding {
                Encoding::Utf32Le => u32::from(scalar).to_le_bytes(),
                Encoding::Utf32Be => u32::from(scalar).to_be_bytes(),
                _ => unreachable!(),
            })
            .collect(),
    }
}

/// Decodes bytes with the selected canonical encoding.
///
/// # Errors
///
/// Returns the encoding and byte offset of malformed input.
pub fn decode(value: &[u8], encoding: Encoding) -> Result<String, DecodeError> {
    match encoding {
        Encoding::Utf8 => std::str::from_utf8(value)
            .map(str::to_owned)
            .map_err(|error| DecodeError {
                encoding,
                byte_offset: error.valid_up_to(),
            }),
        Encoding::Utf16Le | Encoding::Utf16Be => {
            let chunks = value.chunks_exact(2);
            if !chunks.remainder().is_empty() {
                return Err(DecodeError {
                    encoding,
                    byte_offset: value.len() - 1,
                });
            }
            let units = chunks.map(|chunk| match encoding {
                Encoding::Utf16Le => u16::from_le_bytes([chunk[0], chunk[1]]),
                Encoding::Utf16Be => u16::from_be_bytes([chunk[0], chunk[1]]),
                _ => unreachable!(),
            });
            let mut result = String::new();
            for (index, scalar) in char::decode_utf16(units).enumerate() {
                match scalar {
                    Ok(scalar) => result.push(scalar),
                    Err(_) => {
                        return Err(DecodeError {
                            encoding,
                            byte_offset: index * 2,
                        });
                    }
                }
            }
            Ok(result)
        }
        Encoding::Utf32Le | Encoding::Utf32Be => {
            let chunks = value.chunks_exact(4);
            if !chunks.remainder().is_empty() {
                return Err(DecodeError {
                    encoding,
                    byte_offset: value.len() - chunks.remainder().len(),
                });
            }
            let mut result = String::new();
            for (index, chunk) in chunks.enumerate() {
                let scalar = match encoding {
                    Encoding::Utf32Le => {
                        u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                    }
                    Encoding::Utf32Be => {
                        u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                    }
                    _ => unreachable!(),
                };
                let Some(scalar) = char::from_u32(scalar) else {
                    return Err(DecodeError {
                        encoding,
                        byte_offset: index * 4,
                    });
                };
                result.push(scalar);
            }
            Ok(result)
        }
    }
}

#[must_use]
pub fn trim(value: &str) -> String {
    value.trim().to_owned()
}

#[must_use]
pub fn trim_start(value: &str, pattern: Option<&str>) -> String {
    pattern.map_or_else(
        || value.trim_start().to_owned(),
        |pattern| value.strip_prefix(pattern).unwrap_or(value).to_owned(),
    )
}

#[must_use]
pub fn trim_end(value: &str, pattern: Option<&str>) -> String {
    pattern.map_or_else(
        || value.trim_end().to_owned(),
        |pattern| value.strip_suffix(pattern).unwrap_or(value).to_owned(),
    )
}

#[must_use]
pub fn find(value: &str, pattern: &str) -> Option<TextRange> {
    let source = Arc::<str>::from(value);
    source.find(pattern).map(|start| TextRange {
        source,
        start,
        end: start + pattern.len(),
    })
}

#[must_use]
pub fn find_all(value: &str, pattern: &str) -> Vec<TextRange> {
    let source = Arc::<str>::from(value);
    if pattern.is_empty() {
        return source
            .grapheme_indices(true)
            .map(|(start, _)| start)
            .chain(std::iter::once(source.len()))
            .map(|start| TextRange {
                source: Arc::clone(&source),
                start,
                end: start,
            })
            .collect();
    }
    source
        .match_indices(pattern)
        .map(|(start, matched)| TextRange {
            source: Arc::clone(&source),
            start,
            end: start + matched.len(),
        })
        .collect()
}

#[must_use]
pub fn upper(value: &str) -> String {
    value.to_uppercase()
}

#[must_use]
pub fn lower(value: &str) -> String {
    value.to_lowercase()
}
#[must_use]
pub fn upper_first(value: &str) -> String {
    map_first_cased(value, char::to_uppercase)
}

#[must_use]
pub fn upper_words(value: &str) -> String {
    value.split_word_bounds().map(upper_first).collect()
}

#[must_use]
pub fn lower_first(value: &str) -> String {
    map_first_cased(value, char::to_lowercase)
}

fn map_first_cased<I>(value: &str, map: impl Fn(char) -> I) -> String
where
    I: Iterator<Item = char>,
{
    let Some((index, character)) = value
        .char_indices()
        .find(|(_, character)| character.is_lowercase() || character.is_uppercase())
    else {
        return value.to_owned();
    };
    let mut result = String::with_capacity(value.len());
    result.push_str(&value[..index]);
    result.extend(map(character));
    result.push_str(&value[index + character.len_utf8()..]);
    result
}

#[must_use]
pub fn case_fold(value: &str) -> String {
    value.case_fold().collect()
}

#[must_use]
pub fn normalise(value: &str, form: &str) -> String {
    match form {
        "nfc" => value.nfc().collect(),
        "nfd" => value.nfd().collect(),
        "nfkc" => value.nfkc().collect(),
        "nfkd" => value.nfkd().collect(),
        _ => unreachable!("compiler validates normalization forms"),
    }
}

#[must_use]
pub fn split(value: &str, pattern: &str) -> Vec<String> {
    if pattern.is_empty() {
        return value.graphemes(true).map(str::to_owned).collect();
    }
    value.split(pattern).map(str::to_owned).collect()
}

#[must_use]
pub fn replace(value: &str, pattern: &str, replacement: &str) -> String {
    if pattern.is_empty() {
        let mut result = String::with_capacity(
            value.len() + replacement.len() * (value.graphemes(true).count() + 1),
        );
        result.push_str(replacement);
        for grapheme in value.graphemes(true) {
            result.push_str(grapheme);
            result.push_str(replacement);
        }
        return result;
    }
    value.replace(pattern, replacement)
}
