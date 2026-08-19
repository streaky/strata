// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: mixed-width-comparisons
fn main() {
    let small: i8 = 5;
    let wide: i32 = 9;
    println!("{}", terrane_scalar_support::scalar_text(&((((small) as i32) == wide))));
    println!("{}", terrane_scalar_support::scalar_text(&((((small) as i32) != wide))));
    println!("{}", terrane_scalar_support::scalar_text(&((((small) as i32) < wide))));
    println!("{}", terrane_scalar_support::scalar_text(&((((small) as i32) <= wide))));
    println!("{}", terrane_scalar_support::scalar_text(&((((small) as i32) > wide))));
    println!("{}", terrane_scalar_support::scalar_text(&((((small) as i32) >= wide))));
    println!("{}", terrane_scalar_support::scalar_text(&((wide == ((small) as i32)))));
    println!("{}", terrane_scalar_support::scalar_text(&((wide != ((small) as i32)))));
    println!("{}", terrane_scalar_support::scalar_text(&((wide < ((small) as i32)))));
    println!("{}", terrane_scalar_support::scalar_text(&((wide <= ((small) as i32)))));
    println!("{}", terrane_scalar_support::scalar_text(&((wide > ((small) as i32)))));
    println!("{}", terrane_scalar_support::scalar_text(&((wide >= ((small) as i32)))));
}
