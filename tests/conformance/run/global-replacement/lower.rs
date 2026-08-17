// Generated deterministically by Terrane <version>.
static __TERRANE_GLOBAL_COUNTER: std::sync::LazyLock<std::sync::Mutex<Option<terrane_int_support::Int>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(Some(terrane_int_support::Int::from(0_i128))));
// Source: case.trn
// Namespace: global-replacement
fn setup() {
    *__TERRANE_GLOBAL_COUNTER.lock().expect("program-global lock poisoned") = Some(terrane_int_support::Int::from(11_i128));
}
fn bump() {
    {
        let mut value = __TERRANE_GLOBAL_COUNTER.lock().expect("program-global lock poisoned");
        *value = Some(value.clone().expect("program-global binding initialized before use") + terrane_int_support::Int::from(1_i128));
    }
}
fn main() {
    setup();
    bump();
    println!("{}", terrane_scalar_support::scalar_text(&(__TERRANE_GLOBAL_COUNTER.lock().expect("program-global lock poisoned").clone().expect("program-global binding initialized before use"))));
}
