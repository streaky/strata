// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: name-aliasing
static __TERRANE_F0_ORIGINAL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| String::from("namespace"));
static __TERRANE_F0_ALIAS: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| (*__TERRANE_F0_ORIGINAL).clone());
fn main() {
    let local: String = String::from("function");
    let copy: String = local;
    println!("{}", terrane_scalar_support::scalar_text(&(&*__TERRANE_F0_ALIAS)));
    println!("{}", terrane_scalar_support::scalar_text(&(copy)));
}
