// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: build-report
fn report(name: String, passed: bool, attempts: i128) {
    if passed {
        println!("{}", format!("{}{}{}{}", name, String::from(": passed in "), attempts, String::from(" attempt(s)")));
    } else {
        println!("{}", format!("{}{}", name, String::from(": failed")));
    }
}
fn main() {
    report(String::from("lexer"), true, 1);
    report(String::from("parser"), true, 2);
    let title: String = String::from("Terrane");
    println!("{}", format!("{}{}{}", title, String::from(" length: "), title.chars().count()));
}
