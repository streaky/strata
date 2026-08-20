// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: decode-error-offset
fn main() {
    let invalid: Vec<u8> = Vec::from([97, 255]);
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_string_support::decode_or_fail(&(invalid), terrane_string_support::Encoding::Utf8))));
}
