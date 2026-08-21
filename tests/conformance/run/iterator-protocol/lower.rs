// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: iterator-protocol
fn main() {
    let values: terrane_collection_support::Iterator<()> = terrane_collection_support::Iterator::<()>::new(vec![(), ()]);
    let mut __terrane_iterator_0 = values;
    loop {
        let value = match __terrane_iterator_0.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(value)));
    }
    let text: String = String::from("A👍🏽");
    let mut __terrane_iterator_1 = terrane_collection_support::string_iterator(&(text));
    loop {
        let grapheme = match __terrane_iterator_1.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(grapheme)));
    }
}
