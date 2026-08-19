// Generated deterministically by Terrane <version>.
#[derive(Clone, Debug)]
struct TerraneError {
kind: &'static str,
message: String,
cause: Option<Box<TerraneError>>,
context: Vec<&'static str>,
}
impl TerraneError {
fn new(kind: &'static str, message: impl Into<String>) -> Self {
Self { kind, message: message.into(), cause: None, context: Vec::new() }
}
fn at(mut self, frame: &'static str) -> Self {
self.context.push(frame);
self
}
fn render(&self) -> String {
let mut rendered = format!("{}: {}", self.kind, self.message);
if let Some(cause) = &self.cause {
rendered.push_str("\ncaused by: ");
rendered.push_str(&cause.render());
}
for frame in &self.context {
rendered.push_str("\nat ");
rendered.push_str(frame);
}
rendered
}
}
impl std::fmt::Display for TerraneError {
fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
formatter.write_str(&self.render())
}
}
impl From<terrane_int_support::ArithmeticError> for TerraneError {
fn from(error: terrane_int_support::ArithmeticError) -> Self {
Self::new(error.source_name(), error.to_string())
}
}
fn __terrane_uncaught(error: TerraneError) -> ! {
let _ = TerraneError::at;
eprintln!("{}", error.render());
std::process::exit(1);
}
// Source: case.trn
// Namespace: catch-arithmetic-overflow
fn main() {
    let __terrane_try_0: Result<Option<()>, TerraneError> = (|| {
        let mut value: i8 = 127;
        value = (terrane_int_support::fixed_addition(value, 1)).map_err(TerraneError::from)?;
        println!("{}", terrane_scalar_support::scalar_text(&(value)));
        Ok(None)
    })();
    let mut __terrane_return_0: Option<()> = None;
    match __terrane_try_0 {
        Ok(value) => __terrane_return_0 = value,
        Err(__terrane_error_0) => {
            let mut __terrane_handled_0 = false;
            if !__terrane_handled_0 && __terrane_error_0.kind == ".arithmetic-overflow" {
                __terrane_handled_0 = true;
                println!("{}", terrane_scalar_support::scalar_text(&(String::from("caught"))));
            }
            if !__terrane_handled_0 {
                __terrane_uncaught(__terrane_error_0);
            }
        }
    }
    println!("{}", terrane_scalar_support::scalar_text(&(String::from("finally"))));
    if let Some(value) = __terrane_return_0 {
        return value;
    }
}
