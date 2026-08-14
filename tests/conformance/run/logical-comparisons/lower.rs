// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: logical-comparisons
fn main() {
    let x: terrane_int_support::Int = terrane_int_support::Int::from(5_i128);
    let y: terrane_int_support::Int = terrane_int_support::Int::from(9_i128);
    println!("{}", terrane_scalar_support::scalar_text(&(((x.clone() > terrane_int_support::Int::from(1_i128)) && (y.clone() > terrane_int_support::Int::from(2_i128))))));
}
