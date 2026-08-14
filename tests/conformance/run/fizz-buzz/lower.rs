// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: fizz-buzz
fn main() {
    let mut number: i128 = 1;
    while number <= 15 {
        if (number % 15) == 0 {
            println!("{}", terrane_scalar_support::scalar_text(&(String::from("FizzBuzz"))));
        } else if (number % 3) == 0 {
            println!("{}", terrane_scalar_support::scalar_text(&(String::from("Fizz"))));
        } else if (number % 5) == 0 {
            println!("{}", terrane_scalar_support::scalar_text(&(String::from("Buzz"))));
        } else {
            println!("{}", terrane_scalar_support::scalar_text(&(number)));
        }
        number += 1;
    }
}
