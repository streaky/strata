// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: numeric-literals
fn main() {
    let adaptive: terrane_int_support::Int = terrane_int_support::Int::from(16_i128);
    let single: f32 = (1.5) as f32;
    let double: f64 = 2.25;
    let inferred: f64 = 3.5;
    println!("{}", terrane_scalar_support::scalar_text(&(adaptive)));
    println!("{}", terrane_scalar_support::scalar_text(&(single)));
    println!("{}", terrane_scalar_support::scalar_text(&(double)));
    println!("{}", terrane_scalar_support::scalar_text(&(inferred)));
}
