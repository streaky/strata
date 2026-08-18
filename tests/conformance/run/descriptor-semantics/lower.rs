// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: descriptor-semantics
fn accepts(item: terrane_int_support::Int) -> bool {
    println!("{}", terrane_scalar_support::scalar_text(&(item)));
    return { let _ = item.clone(); true };
}
fn same_type(left: terrane_int_support::Int, right: terrane_int_support::Int) -> bool {
    println!("{}", terrane_scalar_support::scalar_text(&(left)));
    println!("{}", terrane_scalar_support::scalar_text(&(right)));
    return { let _ = left; let _ = right; true };
}
fn different_type(left: terrane_int_support::Int, right: String) -> bool {
    println!("{}", terrane_scalar_support::scalar_text(&(left)));
    println!("{}", terrane_scalar_support::scalar_text(&(right)));
    return { let _ = left; let _ = right; false };
}
fn main() {
    println!("{}", terrane_scalar_support::scalar_text(&(accepts(terrane_int_support::Int::from(1_i128)))));
    println!("{}", terrane_scalar_support::scalar_text(&({  true })));
    println!("{}", terrane_scalar_support::scalar_text(&({  true })));
    let value: f64 = 1.0;
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = value; true })));
    println!("{}", terrane_scalar_support::scalar_text(&({ let _ = value; true })));
    println!("{}", terrane_scalar_support::scalar_text(&({  true })));
    println!("{}", terrane_scalar_support::scalar_text(&(same_type(terrane_int_support::Int::from(2_i128), terrane_int_support::Int::from(3_i128)))));
    println!("{}", terrane_scalar_support::scalar_text(&(different_type(terrane_int_support::Int::from(4_i128), String::from("five")))));
}
