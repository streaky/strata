// Generated deterministically by Terrane <version>.
// Source: case.trn
// Namespace: bounded-arithmetic-families
fn main() {
    let small: i8 = 120;
    let wrapped: i8 = terrane_int_support::fixed_addition_wrap(small, 10);
    println!("{}", terrane_scalar_support::scalar_text(&(wrapped)));
    let overflowed: terrane_int_support::OverflowResult<i8> = terrane_int_support::fixed_addition_overflowing(small, 10);
    println!("{}{}", terrane_scalar_support::scalar_text(&(overflowed.value)), terrane_scalar_support::scalar_text(&(overflowed.overflowed)));
    let pair: terrane_int_support::DivRemResult<terrane_int_support::Int> = terrane_int_support::unwrap_or_fail(((-terrane_int_support::Int::from(7_i128))).div_rem(&(terrane_int_support::Int::from(3_i128))));
    println!("{}{}", terrane_scalar_support::scalar_text(&(pair.quotient)), terrane_scalar_support::scalar_text(&(pair.remainder)));
    let exact: i64 = 5;
    println!("{}", terrane_scalar_support::scalar_text(&((terrane_int_support::Int::from((exact) as i128) * terrane_int_support::Int::from(9_i128)))));
    terrane_int_support::fixed_subtraction_checked(small, 20);
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::fixed_multiplication_saturate(small, 2))));
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_division(small, 3)))), terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_remainder(small, 7)))));
    let negated: terrane_int_support::OverflowResult<i8> = terrane_int_support::fixed_negation_overflowing(small);
    println!("{}{}", terrane_scalar_support::scalar_text(&(negated.value)), terrane_scalar_support::scalar_text(&(negated.overflowed)));
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_shift_left_wrap(small, &(1))))));
    terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_shift_right_checked(small, &(2)));
    let mut count: i8 = 1;
    count = terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_addition(count, 1));
    count = terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_subtraction(count, 1));
    println!("{}", terrane_scalar_support::scalar_text(&(count)));
    println!("{}{}", terrane_scalar_support::scalar_text(&((terrane_int_support::fixed_addition_checked(small, 10) == None))), terrane_scalar_support::scalar_text(&(terrane_int_support::fixed_addition_saturate(small, 10))));
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::fixed_subtraction_wrap(small, -20))), terrane_scalar_support::scalar_text(&(terrane_int_support::fixed_subtraction_saturate(small, -20))));
    let sub_overflow: terrane_int_support::OverflowResult<i8> = terrane_int_support::fixed_subtraction_overflowing(small, -20);
    println!("{}{}{}", terrane_scalar_support::scalar_text(&(sub_overflow.value)), terrane_scalar_support::scalar_text(&(sub_overflow.overflowed)), terrane_scalar_support::scalar_text(&((terrane_int_support::fixed_subtraction_checked(small, -20) == None))));
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::fixed_multiplication_wrap(small, 2))), terrane_scalar_support::scalar_text(&((terrane_int_support::fixed_multiplication_checked(small, 2) == None))));
    let mul_overflow: terrane_int_support::OverflowResult<i8> = terrane_int_support::fixed_multiplication_overflowing(small, 2);
    println!("{}{}", terrane_scalar_support::scalar_text(&(mul_overflow.value)), terrane_scalar_support::scalar_text(&(mul_overflow.overflowed)));
    let minimum: i8 = -128;
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_division_wrap(minimum, -1)))), terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_division_saturate(minimum, -1)))));
    let div_overflow: terrane_int_support::OverflowResult<i8> = terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_division_overflowing(minimum, -1));
    println!("{}{}{}", terrane_scalar_support::scalar_text(&(div_overflow.value)), terrane_scalar_support::scalar_text(&(div_overflow.overflowed)), terrane_scalar_support::scalar_text(&((terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_division_checked(minimum, -1)) == None))));
    let rem_overflow: terrane_int_support::OverflowResult<i8> = terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_remainder_overflowing(minimum, -1));
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_remainder_wrap(minimum, -1)))), terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_remainder_saturate(minimum, -1)))));
    println!("{}{}{}", terrane_scalar_support::scalar_text(&(rem_overflow.value)), terrane_scalar_support::scalar_text(&(rem_overflow.overflowed)), terrane_scalar_support::scalar_text(&((terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_remainder_checked(minimum, -1)) == None))));
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(((-terrane_int_support::Int::from(7_i128))).euclidean_div(&(terrane_int_support::Int::from(3_i128)))))), terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(((-terrane_int_support::Int::from(7_i128))).modulo(&(terrane_int_support::Int::from(3_i128)))))));
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(((terrane_int_support::Int::from(7_i128))).euclidean_div(&(-terrane_int_support::Int::from(3_i128)))))), terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(((terrane_int_support::Int::from(7_i128))).modulo(&(-terrane_int_support::Int::from(3_i128)))))));
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(((-terrane_int_support::Int::from(7_i128))).euclidean_div(&(-terrane_int_support::Int::from(3_i128)))))), terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(((-terrane_int_support::Int::from(7_i128))).modulo(&(-terrane_int_support::Int::from(3_i128)))))));
}
