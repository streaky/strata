// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: for-loops
fn main() {
    let text: String = String::from("e\u{301}x");
    for character in terrane_string_support::graphemes(&text) {
        println!("{}", terrane_scalar_support::scalar_text(&(character)));
    }
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_string_support::length(&text) as i128)));
    let mut index: i128 = 0;
    while index < 3 {
        '__terrane_continue_0: {
            if index == 1 {
                break '__terrane_continue_0;
            }
            println!("{}", terrane_scalar_support::scalar_text(&(index)));
        }
        index += 1;
    }
}
