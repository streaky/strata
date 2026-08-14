use std::fmt::Write as _;

use terrane_int_support::Int;

/// Produces canonical Terrane text for a core scalar value.
pub trait ScalarDisplay {
    fn write_scalar(&self, output: &mut String);

    #[must_use]
    fn scalar_text(&self) -> String {
        let mut output = String::new();
        self.write_scalar(&mut output);
        output
    }
}

/// Produces canonical Terrane text for a core scalar value.
#[must_use]
pub fn scalar_text(value: &impl ScalarDisplay) -> String {
    value.scalar_text()
}

impl<T: ScalarDisplay + ?Sized> ScalarDisplay for &T {
    fn write_scalar(&self, output: &mut String) {
        (*self).write_scalar(output);
    }
}

impl ScalarDisplay for bool {
    fn write_scalar(&self, output: &mut String) {
        output.push_str(if *self { "true" } else { "false" });
    }
}

impl ScalarDisplay for str {
    fn write_scalar(&self, output: &mut String) {
        output.push_str(self);
    }
}

impl ScalarDisplay for String {
    fn write_scalar(&self, output: &mut String) {
        output.push_str(self);
    }
}

impl ScalarDisplay for () {
    fn write_scalar(&self, output: &mut String) {
        output.push_str("none");
    }
}

macro_rules! integer_display {
    ($($type:ty),+ $(,)?) => {$(
        impl ScalarDisplay for $type {
            fn write_scalar(&self, output: &mut String) {
                write!(output, "{self}").expect("writing to a String cannot fail");
            }
        }
    )+};
}

integer_display!(Int, i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

macro_rules! float_display {
    ($($type:ty),+ $(,)?) => {$(
        impl ScalarDisplay for $type {
            fn write_scalar(&self, output: &mut String) {
                if self.is_nan() {
                    output.push_str("nan");
                } else {
                    write!(output, "{self}").expect("writing to a String cannot fail");
                }
            }
        }
    )+};
}

float_display!(f32, f64);
