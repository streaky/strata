use std::fmt;

/// Canonical source-language text for a supported core scalar value.
pub trait ScalarDisplay {
    fn write_scalar(&self, output: &mut String);

    #[must_use]
    fn scalar_text(&self) -> String {
        let mut output = String::new();
        self.write_scalar(&mut output);
        output
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

macro_rules! integer_display {
    ($($type:ty),+ $(,)?) => {$(
        impl ScalarDisplay for $type {
            fn write_scalar(&self, output: &mut String) {
                use fmt::Write as _;
                write!(output, "{self}").expect("writing to a String cannot fail");
            }
        }
    )+};
}

integer_display!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

macro_rules! float_display {
    ($($type:ty),+ $(,)?) => {$(
        impl ScalarDisplay for $type {
            fn write_scalar(&self, output: &mut String) {
                if self.is_nan() {
                    output.push_str("nan");
                } else {
                    use fmt::Write as _;
                    write!(output, "{self}").expect("writing to a String cannot fail");
                }
            }
        }
    )+};
}

float_display!(f32, f64);
