// Generated deterministically by Terrane <version>.
static __TERRANE_GLOBAL_COUNTER: std::sync::LazyLock<std::sync::Mutex<Option<terrane_int_support::Int>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(Some(terrane_int_support::Int::from(0_i128))));
fn __terrane_uninitialized_global(name: &str, path: &str, line: usize, column: usize) -> ! {
    eprintln!("{path}:{line}:{column}: error[T0007]: `{name}` may be read before it is assigned");
    std::process::exit(1);
}
// Source: case.trn
// Namespace: global-replacement
fn setup() {
    {
        let value = terrane_int_support::Int::from(11_i128);
        *__TERRANE_GLOBAL_COUNTER.lock().expect("program-global lock poisoned") = Some(value);
    }
}
fn bump() {
    {
        let value = __TERRANE_GLOBAL_COUNTER.lock().expect("program-global lock poisoned").clone().unwrap_or_else(|| __terrane_uninitialized_global("counter", "case.trn", 6, 20)).clone() + terrane_int_support::Int::from(1_i128);
        *__TERRANE_GLOBAL_COUNTER.lock().expect("program-global lock poisoned") = Some(value);
    }
}
fn current() -> terrane_int_support::Int {
    return __TERRANE_GLOBAL_COUNTER.lock().expect("program-global lock poisoned").clone().unwrap_or_else(|| __terrane_uninitialized_global("counter", "case.trn", 8, 10)).clone();
}
fn main() {
    setup();
    bump();
    if __TERRANE_GLOBAL_COUNTER.lock().expect("program-global lock poisoned").clone().unwrap_or_else(|| __terrane_uninitialized_global("counter", "case.trn", 12, 6)).clone() == terrane_int_support::Int::from(12_i128) {
        println!("{}", terrane_scalar_support::scalar_text(&(((current()) + terrane_int_support::Int::from(1_i128)))));
    }
}
