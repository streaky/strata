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
// Namespace: parse-radix-families
fn from_hex(source: String) -> terrane_int_support::Int {
    return terrane_int_support::unwrap_or_fail(terrane_int_support::parse_radix(&(source), &(16)));
}
fn fail(source: String) -> Result<terrane_int_support::Int, TerraneError> {
    println!("{}", terrane_scalar_support::scalar_text(&(source)));
    return Err(TerraneError::new(".coercion-error", "coercion has no compatible result").at("case.trn:8:3"));
}
fn main() {
    let text: String = String::from("ff");
    let parsed: terrane_int_support::Int = from_hex(text);
    println!("{}", terrane_scalar_support::scalar_text(&(parsed)));
    let value: i64 = 255;
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::unwrap_or_fail(terrane_int_support::format_radix(&(value), &(16))))));
    let bad: String = String::from("x");
    (fail(bad)).ok();
}
