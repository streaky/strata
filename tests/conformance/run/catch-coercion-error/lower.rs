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
eprintln!("{}", error.render());
std::process::exit(1);
}
#[allow(dead_code)]
enum TerraneCompletion<T> {
Normal,
Return(T),
Error(TerraneError),
Break,
Continue,
}
// Source: case.trn
// Namespace: catch-coercion-error
fn main() {
    let value: i64 = 300;
    let __terrane_completion_0: TerraneCompletion<()> = (|| {
        let __terrane_try_0: TerraneCompletion<()> = (|| {
            let narrow: i8 = match terrane_int_support::coerce::<i8>(&(value)) { Ok(value) => value, Err(error) => return TerraneCompletion::Error(error.into()) };
            println!("{}", terrane_scalar_support::scalar_text(&(narrow)));
            TerraneCompletion::Normal
        })();
        match __terrane_try_0 {
            TerraneCompletion::Return(value) => return TerraneCompletion::Return(value),
            TerraneCompletion::Break => return TerraneCompletion::Break,
            TerraneCompletion::Continue => return TerraneCompletion::Continue,
            TerraneCompletion::Normal => {}
            TerraneCompletion::Error(__terrane_error_0) => {
                let mut __terrane_handled_0 = false;
                if !__terrane_handled_0 && __terrane_error_0.kind == ".integer-conversion-overflow" {
                    __terrane_handled_0 = true;
                    println!("{}", terrane_scalar_support::scalar_text(&(String::from("caught"))));
                }
                if !__terrane_handled_0 {
                    return TerraneCompletion::Error(__terrane_error_0);
                }
            }
        }
        TerraneCompletion::Normal
    })();
    match __terrane_completion_0 {
        TerraneCompletion::Normal => {}
        TerraneCompletion::Return(value) => return value,
        TerraneCompletion::Error(error) => __terrane_uncaught(error),
        TerraneCompletion::Break | TerraneCompletion::Continue => unreachable!(),
    }
}
