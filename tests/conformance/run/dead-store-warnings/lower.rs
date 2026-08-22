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
    let mut __terrane_iterator_1 = terrane_collection_support::string_iterator(&(String::from("ab")));
    loop {
        let mut replaced = match __terrane_iterator_1.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        let _ = &replaced;
        replaced = String::from("x");
        println!("{}", terrane_scalar_support::scalar_text(&(replaced)));
    }
    let mut __terrane_iterator_2 = terrane_collection_support::string_iterator(&(String::from("c")));
    loop {
        let mut preserved = match __terrane_iterator_2.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(preserved)));
        preserved = String::from("y");
        println!("{}", terrane_scalar_support::scalar_text(&(preserved)));
    }
    let mut __terrane_iterator_3 = terrane_collection_support::string_iterator(&(String::from("a")));
    loop {
        let outer = match __terrane_iterator_3.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        let mut __terrane_iterator_4 = terrane_collection_support::string_iterator(&(String::from("b")));
        loop {
            let mut inner = match __terrane_iterator_4.next() {
                terrane_collection_support::IterationStep::Item(item) => item,
                terrane_collection_support::IterationStep::End => break,
            };
            let _ = &inner;
            inner = String::from("z");
            println!("{}{}", terrane_scalar_support::scalar_text(&(outer)), terrane_scalar_support::scalar_text(&(inner)));
        }
    }
}
