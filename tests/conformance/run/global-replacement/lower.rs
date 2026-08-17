// Generated deterministically by Terrane <version>.
static __TERRANE_GLOBAL_COUNTER: std::sync::LazyLock<std::sync::Mutex<Option<terrane_int_support::Int>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(Some(terrane_int_support::Int::from(0_i128))));
fn __terrane_uninitialized_global(name: &str, path: &str, line: usize, column: usize) -> ! {
    eprintln!("{path}:{line}:{column}: error[T0007]: `{name}` may be read before it is assigned");
    std::process::exit(1);
}
// Source: case.trn
// Namespace: global-replacement
fn setup() {
    *__TERRANE_GLOBAL_COUNTER.lock().expect("program-global lock poisoned") = Some(terrane_int_support::Int::from(11_i128));
}
fn bump() {
    {
        let mut value = __TERRANE_GLOBAL_COUNTER.lock().expect("program-global lock poisoned");
        *value = Some(value.clone().unwrap_or_else(|| __terrane_uninitialized_global("counter", "case.trn", 6, 10)) + terrane_int_support::Int::from(1_i128));
    }
}
fn main() {
    setup();
    bump();
    println!("{}", terrane_scalar_support::scalar_text(&(__TERRANE_GLOBAL_COUNTER.lock().expect("program-global lock poisoned").clone().unwrap_or_else(|| __terrane_uninitialized_global("counter", "case.trn", 10, 10)))));
}
