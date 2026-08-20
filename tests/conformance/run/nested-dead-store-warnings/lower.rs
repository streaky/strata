// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: nested-dead-store-warnings
fn main() {
    if 1 == 1 {
        let mut nested: i8 = 1;
        let _ = &mut nested;
        nested = 2;
        println!("{}", terrane_scalar_support::scalar_text(&(nested)));
    }
    let mut top: i8 = 3;
    let _ = &mut top;
    top = 4;
    println!("{}", terrane_scalar_support::scalar_text(&(top)));
}
