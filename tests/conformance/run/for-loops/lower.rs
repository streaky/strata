// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: for-loops
fn main() {
    let text: String = String::from("ab");
    for character in text.chars().map(String::from) {
        println!("{}", character);
    }
    let mut index: i128 = 0;
    while index < 3 {
        '__terrane_continue_0: {
            if index == 1 {
                break '__terrane_continue_0;
            }
            println!("{}", index);
        }
        index += 1;
    }
}
