// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: optional-value-narrowing
fn helper() {
    let found: String = String::from("shadow");
    println!("{}", terrane_scalar_support::scalar_text(&(found)));
}
fn show(value: Option<i8>) {
    if value != None {
        println!("{}", terrane_scalar_support::scalar_text(&(*value.as_ref().expect("semantic optional narrowing"))));
    }
}
fn maybe() -> Option<i8> {
    return Some(4);
}
fn main() {
    let value: Option<i8> = Some(7);
    if value != None {
        println!("{}", terrane_scalar_support::scalar_text(&(*value.as_ref().expect("semantic optional narrowing"))));
    }
    let other: Option<i8> = Some(8);
    if other != None {
        println!("{}", terrane_scalar_support::scalar_text(&(*other.as_ref().expect("semantic optional narrowing"))));
    }
    show(Some(9));
    let returned: Option<i8> = maybe();
    if returned != None {
        println!("{}", terrane_scalar_support::scalar_text(&(*returned.as_ref().expect("semantic optional narrowing"))));
    }
    helper();
    let found: Option<terrane_string_support::TextRange> = terrane_string_support::find(&(String::from("banana")), &(String::from("ana")));
    if found != None {
        if found != None {
            println!("{}", terrane_scalar_support::scalar_text(&((found.as_ref().expect("semantic optional narrowing")).text().to_owned())));
        }
    }
}
