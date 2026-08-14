// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: descriptor-semantics
fn accepts(item: terrane_int_support::Int) -> bool {
    println!("{}", terrane_scalar_support::scalar_text(&(item)));
    return true;
}
fn main() {
    println!("{}", terrane_scalar_support::scalar_text(&(accepts(terrane_int_support::Int::from(1_i128)))));
    println!("{}", terrane_scalar_support::scalar_text(&(true)));
    println!("{}", terrane_scalar_support::scalar_text(&(false)));
}
