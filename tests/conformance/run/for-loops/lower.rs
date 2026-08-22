// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: for-loops
fn show(mut value: String) {
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
    value = String::from("parameter");
    println!("{}", terrane_scalar_support::scalar_text(&(value)));
}
fn main() {
    show(String::from("original"));
    let text: String = String::from("e\u{301}x");
    let mut __terrane_iterator_0 = terrane_collection_support::string_iterator(&(text));
    loop {
        let mut character = match __terrane_iterator_0.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(character)));
        character = String::from("loop");
        println!("{}", terrane_scalar_support::scalar_text(&(character)));
    }
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_string_support::length(&text) as i128)));
    let mut index: terrane_int_support::Int = terrane_int_support::Int::from(0_i128);
    while index.clone() < terrane_int_support::Int::from(3_i128) {
        '__terrane_continue_1: {
            if index.clone() == terrane_int_support::Int::from(1_i128) {
                break '__terrane_continue_1;
            }
            println!("{}", terrane_scalar_support::scalar_text(&(index)));
        }
        index = index.clone() + terrane_int_support::Int::from(1_i128);
    }
}
