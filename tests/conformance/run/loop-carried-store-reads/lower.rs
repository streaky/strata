// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: loop-carried-store-reads
fn main() {
    let mut while_value: i8 = 0;
    let mut limit: i8 = 0;
    while limit < 2 {
        println!("{}", terrane_scalar_support::scalar_text(&(while_value)));
        while_value = 5;
        limit = terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_addition(limit, 1));
    }
    let mut for_value: i8 = 0;
    for character in terrane_string_support::graphemes(&String::from("ab")) {
        let _ = &character;
        println!("{}", terrane_scalar_support::scalar_text(&(for_value)));
        for_value = 7;
    }
}
