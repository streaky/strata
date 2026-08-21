// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: collections-value-semantics
fn main() {
    let original: terrane_collection_support::List<terrane_int_support::Int> = terrane_collection_support::List::<terrane_int_support::Int>::new(vec![terrane_int_support::Int::from(1_i128), terrane_int_support::Int::from(2_i128)]);
    let mut independent: terrane_collection_support::List<terrane_int_support::Int> = (original).clone();
    (independent).append(terrane_int_support::Int::from(3_i128));
    println!("{}{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((original).length()))), terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((independent).length()))), terrane_scalar_support::scalar_text(&((independent).get((terrane_int_support::Int::from(2_i128)).as_usize().expect("index-error: index is outside usize")).cloned().expect("index-error: index is out of range"))));
    let mut ordered: terrane_collection_support::Map<String, terrane_int_support::Int> = terrane_collection_support::Map::<String, terrane_int_support::Int>::new(vec![terrane_collection_support::Entry::new(String::from("first"), terrane_int_support::Int::from(1_i128)), terrane_collection_support::Entry::new(String::from("second"), terrane_int_support::Int::from(2_i128))]);
    (ordered).set(String::from("third"), terrane_int_support::Int::from(3_i128));
    let mut __terrane_iterator_0 = terrane_collection_support::Iterable::terrane_iterator(&(ordered));
    loop {
        let pair = match __terrane_iterator_0.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}{}", terrane_scalar_support::scalar_text(&(pair.key)), terrane_scalar_support::scalar_text(&(pair.value)));
    }
    println!("{}", terrane_scalar_support::scalar_text(&((ordered).get(&(String::from("second"))).cloned().expect("missing-key: key is absent"))));
    let mut unique: terrane_collection_support::Set<String> = terrane_collection_support::Set::<String>::new(vec![String::from("b"), String::from("a"), String::from("b")]);
    (unique).add(String::from("c"));
    let mut __terrane_iterator_1 = terrane_collection_support::Iterable::terrane_iterator(&(unique));
    loop {
        let value = match __terrane_iterator_1.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(value)));
    }
    let pair: terrane_collection_support::Tuple<String> = terrane_collection_support::Tuple::<String>::new(vec![String::from("left"), String::from("right")]);
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((pair).length()))), terrane_scalar_support::scalar_text(&((pair).get((terrane_int_support::Int::from(1_i128)).as_usize().expect("index-error: index is outside usize")).cloned().expect("index-error: index is out of range"))));
    let explicit: terrane_collection_support::Entry<String, terrane_int_support::Int> = terrane_collection_support::Entry::<String, terrane_int_support::Int>::new(String::from("key"), terrane_int_support::Int::from(7_i128));
    println!("{}{}", terrane_scalar_support::scalar_text(&((explicit).key.clone())), terrane_scalar_support::scalar_text(&((explicit).value.clone())));
    let numbers: terrane_collection_support::Range = terrane_collection_support::Range::new(terrane_int_support::Int::from(0_i128), terrane_int_support::Int::from(3_i128), terrane_int_support::Int::from(1_i64)).expect("range step must be non-zero");
    let mut __terrane_iterator_2 = terrane_collection_support::Iterable::terrane_iterator(&(numbers));
    loop {
        let number = match __terrane_iterator_2.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(number)));
    }
    let inclusive: terrane_collection_support::Range = terrane_collection_support::Range::through(terrane_int_support::Int::from(2_i128), terrane_int_support::Int::from(0_i128), terrane_int_support::Int::from(-1_i128)).expect("range step must be non-zero");
    let mut __terrane_iterator_3 = terrane_collection_support::Iterable::terrane_iterator(&(inclusive));
    loop {
        let number = match __terrane_iterator_3.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(number)));
    }
    let deterministic_map: terrane_collection_support::UnorderedMap<String, terrane_int_support::Int> = terrane_collection_support::UnorderedMap::<String, terrane_int_support::Int>::new(vec![terrane_collection_support::Entry::new(String::from("first"), terrane_int_support::Int::from(1_i128)), terrane_collection_support::Entry::new(String::from("second"), terrane_int_support::Int::from(2_i128))]);
    let mut __terrane_iterator_4 = terrane_collection_support::Iterable::terrane_iterator(&(deterministic_map));
    loop {
        let pair = match __terrane_iterator_4.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(pair.key)));
    }
    let deterministic_set: terrane_collection_support::UnorderedSet<String> = terrane_collection_support::UnorderedSet::<String>::new(vec![String::from("x"), String::from("y")]);
    let mut __terrane_iterator_5 = terrane_collection_support::Iterable::terrane_iterator(&(deterministic_set));
    loop {
        let value = match __terrane_iterator_5.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(value)));
    }
}
