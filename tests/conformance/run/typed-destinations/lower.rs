// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: typed-destinations
fn answer() -> terrane_int_support::Int {
    return terrane_int_support::Int::from(41_i128);
}
fn main() {
    let text: String = String::from("Terrane");
    let total: terrane_int_support::Int = terrane_int_support::Int::from(terrane_string_support::length(&text) as i128);
    let _ = &total;
    let total: terrane_int_support::Int = total.clone() + terrane_int_support::Int::from(1_i128);
    println!("{}", terrane_scalar_support::scalar_text(&(answer())));
    println!("{}", terrane_scalar_support::scalar_text(&(total)));
}
