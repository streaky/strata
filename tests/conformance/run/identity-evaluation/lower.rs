// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: identity-evaluation
fn observed(item: terrane_int_support::Int) -> terrane_int_support::Int {
    println!("{}", terrane_scalar_support::scalar_text(&(item)));
    return item.clone();
}
fn main() {
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = (observed(terrane_int_support::Int::from(1_i128))) + terrane_int_support::Int::from(1_i128); true })));
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = observed(terrane_int_support::Int::from(2_i128)); let _ = observed(terrane_int_support::Int::from(3_i128)); true })));
}
