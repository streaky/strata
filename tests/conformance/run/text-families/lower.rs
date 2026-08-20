// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: text-families
fn main() {
    let text: String = String::from("  Straße  ");
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_string_support::trim(&(text), None))));
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_string_support::case_fold(&(text)))));
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_string_support::upper_first(&(String::from("straße"))))), terrane_scalar_support::scalar_text(&(terrane_string_support::upper_words(&(String::from("hello world"))))));
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_string_support::lower_first(&(String::from("Hello"))))));
    let decomposed: String = String::from("e\u{301}");
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_string_support::normalise(&(decomposed), "nfc"))));
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_string_support::replace(&(String::from("banana")), &(String::from("ana")), &(String::from("X"))))));
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from(terrane_string_support::find_all(&(String::from("banana")), &(String::from("ana"))).len() as i128))), terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from(terrane_string_support::find_all(&(String::from("banana")), &(String::from(""))).len() as i128))));
    let rtl: String = String::from("שלום");
    println!("{}{}", terrane_scalar_support::scalar_text(&((rtl).starts_with(&(String::from("ש"))))), terrane_scalar_support::scalar_text(&((rtl).ends_with(&(String::from("ם"))))));
}
