// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: build-report
static __TERRANE_F0_TITLE: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| String::from("Terrane"));
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
    println!("{}", terrane_scalar_support::scalar_text(&(format!("{}{}{}", terrane_scalar_support::scalar_text(&(&*__TERRANE_F0_TITLE)), terrane_scalar_support::scalar_text(&(String::from(" length: "))), terrane_scalar_support::scalar_text(&(terrane_string_support::length(&&*__TERRANE_F0_TITLE) as i128))))));
}
