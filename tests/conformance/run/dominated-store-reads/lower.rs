// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: dominated-store-reads
fn main() {
    let mut nested_value: i8 = 0;
    let _ = &mut nested_value;
    if 1 == 1 {
        nested_value = 1;
        if 1 == 1 {
            println!("{}", terrane_scalar_support::scalar_text(&(nested_value)));
        }
    }
    let mut escaping: i8 = 0;
    if 1 == 1 {
        escaping = 2;
    }
    println!("{}", terrane_scalar_support::scalar_text(&(escaping)));
}
