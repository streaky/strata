// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: string-join
fn separator() -> String {
    println!("{}", terrane_scalar_support::scalar_text(&(String::from("side effect"))));
    return String::from("--");
}
fn main() {
    let empty: String = { let _ = separator(); String::new() };
    let single: String = vec![terrane_scalar_support::scalar_text(&(String::from("one")))].join(&(String::from("--")));
    let many: String = vec![terrane_scalar_support::scalar_text(&(String::from("one"))), terrane_scalar_support::scalar_text(&(2)), terrane_scalar_support::scalar_text(&(true))].join(&(String::from("--")));
    println!("{}", terrane_scalar_support::scalar_text(&(empty)));
    println!("{}", terrane_scalar_support::scalar_text(&(single)));
    println!("{}", terrane_scalar_support::scalar_text(&(many)));
}
