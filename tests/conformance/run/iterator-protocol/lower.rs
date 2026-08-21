// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: iterator-protocol
fn main() {
    let mut values: terrane_collection_support::Iterator<()> = terrane_collection_support::Iterator::<()>::new(vec![(), ()]);
    let mut __terrane_iterator_0 = &mut (values);
    loop {
        let value = match __terrane_iterator_0.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(value)));
    }
    let mut __terrane_iterator_1 = &mut (values);
    loop {
        let revisited = match __terrane_iterator_1.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        let _ = &revisited;
        println!("{}", terrane_scalar_support::scalar_text(&(String::from("revisited"))));
    }
    let text: String = String::from("A👍🏽");
    let mut __terrane_iterator_2 = terrane_collection_support::string_iterator(&(text));
    loop {
        let grapheme = match __terrane_iterator_2.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(grapheme)));
    }
}
