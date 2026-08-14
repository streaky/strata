use terrane_string_support::{byte_length, length, scalar_length};

#[test]
fn default_length_counts_extended_grapheme_clusters() {
    assert_eq!(length("a\u{310}e\u{301}o\u{308}\u{332}"), 3);
    assert_eq!(length("👨‍👩‍👧‍👦🇷🇺"), 2);
    assert_eq!(length("a\r\nb"), 3);
}

#[test]
fn explicit_scalar_and_byte_lengths_preserve_their_units() {
    let text = "e\u{301}";
    assert_eq!(length(text), 1);
    assert_eq!(scalar_length(text), 2);
    assert_eq!(byte_length(text), 3);
}
