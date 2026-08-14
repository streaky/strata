// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: fizz-buzz
fn main() {
    let mut number: i128 = 1;
    while number <= 15 {
        if (number % 15) == 0 {
            println!("{}", "FizzBuzz");
        } else if (number % 3) == 0 {
            println!("{}", "Fizz");
        } else if (number % 5) == 0 {
            println!("{}", "Buzz");
        } else {
            println!("{}", number);
        }
        number += 1;
    }
}
