// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: fizz-buzz
fn main() {
    let mut number: terrane_int_support::Int = terrane_int_support::Int::from(1_i128);
    while number.clone() <= terrane_int_support::Int::from(15_i128) {
        if terrane_int_support::unwrap_or_fail((number.clone()).modulo(&(terrane_int_support::Int::from(15_i128)))) == terrane_int_support::Int::from(0_i128) {
            println!("{}", terrane_scalar_support::scalar_text(&(String::from("FizzBuzz"))));
        } else if terrane_int_support::unwrap_or_fail((number.clone()).modulo(&(terrane_int_support::Int::from(3_i128)))) == terrane_int_support::Int::from(0_i128) {
            println!("{}", terrane_scalar_support::scalar_text(&(String::from("Fizz"))));
        } else if terrane_int_support::unwrap_or_fail((number.clone()).modulo(&(terrane_int_support::Int::from(5_i128)))) == terrane_int_support::Int::from(0_i128) {
            println!("{}", terrane_scalar_support::scalar_text(&(String::from("Buzz"))));
        } else {
            println!("{}", terrane_scalar_support::scalar_text(&(number)));
        }
        number = number.clone() + terrane_int_support::Int::from(1_i128);
    }
}
