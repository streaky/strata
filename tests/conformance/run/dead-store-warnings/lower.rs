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
    let mut __terrane_iterator_0 = terrane_collection_support::string_iterator(&(String::from("ab")));
    loop {
        let ignored = match __terrane_iterator_0.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        let _ = &ignored;
        println!("{}", terrane_scalar_support::scalar_text(&(String::from("tick"))));
    }
}
