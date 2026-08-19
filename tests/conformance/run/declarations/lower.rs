// Generated deterministically by Terrane <version>.
static __TERRANE_GLOBAL_NAMESPACE_VALUE: std::sync::LazyLock<std::sync::Mutex<Option<terrane_int_support::Int>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(Some(terrane_int_support::Int::from(12_i128))));
fn __terrane_uninitialized_global(name: &str, path: &str, line: usize, column: usize) -> ! {
    eprintln!("{path}:{line}:{column}: error[T0007]: `{name}` may be read before it is assigned");
    std::process::exit(1);
}
// Source: case.trn
// Namespace: declarations
fn main() {
    let local_value: i8;
    local_value = 16;
    println!("{}", terrane_scalar_support::scalar_text(&(__TERRANE_GLOBAL_NAMESPACE_VALUE.lock().expect("program-global lock poisoned").clone().unwrap_or_else(|| __terrane_uninitialized_global("namespace-value", "case.trn", 7, 10)))));
    println!("{}", terrane_scalar_support::scalar_text(&(local_value)));
    if true {
        let block_value: i64 = 300;
        println!("{}", terrane_scalar_support::scalar_text(&(block_value)));
    }
    if false {
        println!("{}", terrane_scalar_support::scalar_text(&(String::from("no"))));
    } else {
        let else_value: i64 = 400;
        println!("{}", terrane_scalar_support::scalar_text(&(else_value)));
    }
}
