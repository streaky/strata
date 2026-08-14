// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: build-report
fn report(name: String, passed: bool, attempts: terrane_int_support::Int) {
    if passed {
        println!("{}", terrane_scalar_support::scalar_text(&(format!("{}{}{}{}", terrane_scalar_support::scalar_text(&(name)), terrane_scalar_support::scalar_text(&(String::from(": passed in "))), terrane_scalar_support::scalar_text(&(attempts)), terrane_scalar_support::scalar_text(&(String::from(" attempt(s)")))))));
    } else {
        println!("{}", terrane_scalar_support::scalar_text(&(format!("{}{}", terrane_scalar_support::scalar_text(&(name)), terrane_scalar_support::scalar_text(&(String::from(": failed")))))));
    }
}
fn main() {
    report(String::from("lexer"), true, terrane_int_support::Int::from(1_i128));
    report(String::from("parser"), true, terrane_int_support::Int::from(2_i128));
    let title: String = String::from("Terrane");
    println!("{}", terrane_scalar_support::scalar_text(&(format!("{}{}{}", terrane_scalar_support::scalar_text(&(title)), terrane_scalar_support::scalar_text(&(String::from(" length: "))), terrane_scalar_support::scalar_text(&(terrane_string_support::length(&title) as i128))))));
}
