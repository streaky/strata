// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: bytes-views-encoding
fn main() {
    let text: String = String::from("e\u{301}");
    println!("{}{}{}{}", terrane_scalar_support::scalar_text(&(terrane_string_support::length(&text) as i128)), terrane_scalar_support::scalar_text(&(((text).as_bytes().to_vec()).len() as i128)), terrane_scalar_support::scalar_text(&(((text).chars().map(|value| value.to_string()).collect::<Vec<_>>()).len() as i128)), terrane_scalar_support::scalar_text(&((terrane_string_support::graphemes(&(text)).collect::<Vec<_>>()).len() as i128)));
    let encoded: Vec<u8> = terrane_string_support::encode(&(text), terrane_string_support::Encoding::Utf8);
    let decoded: String = terrane_string_support::decode_or_fail(&(encoded), terrane_string_support::Encoding::Utf8);
    println!("{}", terrane_scalar_support::scalar_text(&(decoded)));
    let raw: Vec<u8> = Vec::from([97, 98, 99]);
    for byte in (raw).iter().copied() {
        println!("{}", terrane_scalar_support::scalar_text(&(byte)));
    }
    println!("{}{}", terrane_scalar_support::scalar_text(&((raw).len() as i128)), terrane_scalar_support::scalar_text(&(terrane_string_support::decode_or_fail(&(raw), terrane_string_support::Encoding::Utf8))));
}
