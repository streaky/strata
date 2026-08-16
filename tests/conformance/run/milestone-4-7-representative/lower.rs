// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: milestone-representative
fn main() {
    let heading: String = format!("{}{}{}{}{}", terrane_scalar_support::scalar_text(&(String::from("Terrane"))), terrane_scalar_support::scalar_text(&(String::from(" "))), terrane_scalar_support::scalar_text(&(4)), terrane_scalar_support::scalar_text(&(String::from("."))), terrane_scalar_support::scalar_text(&(7)));
    let joined: String = vec![terrane_scalar_support::scalar_text(&(heading)), terrane_scalar_support::scalar_text(&(String::from("namespaces"))), terrane_scalar_support::scalar_text(&(String::from("strings")))].join(&(String::from(" / ")));
    let count: terrane_int_support::Int = terrane_int_support::Int::from(terrane_string_support::length(&String::from("a🇺🇳")) as i128);
    let converted: u8 = terrane_int_support::saturating_coerce::<u8>(&(300));
    if count.clone() == terrane_int_support::Int::from(2_i128) {
        println!("{}", terrane_scalar_support::scalar_text(&(joined)));
    }
    println!("{}", terrane_scalar_support::scalar_text(&(converted)));
}
