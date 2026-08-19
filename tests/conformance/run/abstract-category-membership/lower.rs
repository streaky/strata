// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: abstract-category-membership
fn main() {
    let signed: i8 = -1;
    let unsigned: u8 = 1;
    let adaptive: i64 = 1;
    let decimal: f32 = 1.5_f32;
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = &signed; true })));
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = &signed; true })));
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = &signed; true })));
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = &signed; true })));
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = &signed; false })));
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = &unsigned; true })));
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = &adaptive; false })));
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = &decimal; true })));
}
