// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: dead-store-warnings
fn main() {
    let mut value: i8 = 1;
    let _ = &mut value;
    value = 2;
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
    let mut stale: i8 = 3;
    let _ = &mut stale;
    stale = 4;
    let _ = &mut stale;
    for ignored in terrane_string_support::graphemes(&String::from("ab")) {
        let _ = &ignored;
        println!("{}", terrane_scalar_support::scalar_text(&(String::from("tick"))));
    }
}
