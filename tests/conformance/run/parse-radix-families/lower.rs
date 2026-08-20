// Generated deterministically by Terrane <version>.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerraneErrorKind {
ArithmeticOverflow,
DivisionByZero,
IntegerConversionOverflow,
NegativeShiftCount,
CoercionError,
ResourceError,
SourceError,
}
impl TerraneErrorKind {
fn from_source_name(name: &str) -> Self {
match name {
".arithmetic-overflow" => Self::ArithmeticOverflow,
".division-by-zero" => Self::DivisionByZero,
".integer-conversion-overflow" => Self::IntegerConversionOverflow,
".negative-shift-count" => Self::NegativeShiftCount,
".coercion-error" => Self::CoercionError,
".resource-error" => Self::ResourceError,
_ => Self::SourceError,
}
}
fn source_name(self) -> &'static str {
match self {
Self::ArithmeticOverflow => ".arithmetic-overflow",
Self::DivisionByZero => ".division-by-zero",
Self::IntegerConversionOverflow => ".integer-conversion-overflow",
Self::NegativeShiftCount => ".negative-shift-count",
Self::CoercionError => ".coercion-error",
Self::ResourceError => ".resource-error",
Self::SourceError => ".error",
}
}
}
#[derive(Clone, Debug)]
struct TerraneError {
kind: TerraneErrorKind,
message: String,
cause: Option<Box<TerraneError>>,
context: Vec<&'static str>,
}
impl TerraneError {
fn new(kind: TerraneErrorKind, message: impl Into<String>) -> Self {
Self { kind, message: message.into(), cause: None, context: Vec::new() }
}
#[allow(dead_code)]
fn at(mut self, frame: &'static str) -> Self {
self.context.push(frame);
self
}
fn render(&self) -> String {
let mut rendered = format!("{}: {}", self.kind.source_name(), self.message);
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
Self::new(TerraneErrorKind::from_source_name(error.source_name()), error.to_string())
}
}
fn __terrane_uncaught(error: TerraneError) -> ! {
eprintln!("{}", error.render());
std::process::exit(1);
}
fn __terrane_generated_defect(message: &str) -> ! {
eprintln!("internal compiler defect: generated program reached an impossible completion: {message}");
std::process::exit(5);
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
// Namespace: parse-radix-families
fn from_hex(source: String) -> terrane_int_support::Int {
    return (terrane_int_support::parse_radix(&(source), &(16))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/parse-radix-families::from-hex (case.trn:5:10)")));
}
fn fail(source: String) -> Result<terrane_int_support::Int, TerraneError> {
    println!("{}", terrane_scalar_support::scalar_text(&(source)));
    return Err(TerraneError::new(TerraneErrorKind::CoercionError, "coercion has no compatible result").at("/parse-radix-families::fail (case.trn:8:3)"));
}
fn main() {
    let text: String = String::from("ff");
    let parsed: terrane_int_support::Int = from_hex(text);
    println!("{}", terrane_scalar_support::scalar_text(&(parsed)));
    let value: i64 = 255;
    println!("{}", terrane_scalar_support::scalar_text(&((terrane_int_support::format_radix(&(value), &(16))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/parse-radix-families::main (case.trn:14:11)"))))));
    let bad: String = String::from("x");
    (fail(bad)).ok();
}
