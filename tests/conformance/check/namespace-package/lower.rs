// Generated deterministically by Terrane <version>.
// Source: main.trn
// Namespace: app
fn main() {
    println!("{}", terrane_scalar_support::scalar_text(&(String::from(" namespace package"))));
}
// Source: support.trn
// Namespace: app/support
static __TERRANE_F1_VALUE: std::sync::LazyLock<terrane_int_support::Int> = std::sync::LazyLock::new(|| terrane_int_support::Int::from(1_i128));
