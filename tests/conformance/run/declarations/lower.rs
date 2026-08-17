// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: declarations
static __TERRANE_F0_NAMESPACE_VALUE: std::sync::LazyLock<terrane_int_support::Int> = std::sync::LazyLock::new(|| {
    let mut __terrane_namespace_value_value = terrane_int_support::Int::from(0_i128);
    __terrane_namespace_value_value = terrane_int_support::Int::from(11_i128);
    __terrane_namespace_value_value = __terrane_namespace_value_value.clone() + terrane_int_support::Int::from(1_i128);
    __terrane_namespace_value_value
});
fn main() {
    let local_value: i8;
    local_value = 16;
    println!("{}", terrane_scalar_support::scalar_text(&(&*__TERRANE_F0_NAMESPACE_VALUE)));
    println!("{}", terrane_scalar_support::scalar_text(&(local_value)));
    if true {
        let block_value: terrane_int_support::Int = terrane_int_support::Int::from(300_i128);
        println!("{}", terrane_scalar_support::scalar_text(&(block_value)));
    }
}
