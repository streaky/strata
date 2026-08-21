// Generated deterministically by Terrane <version>.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerraneErrorKind {
ArithmeticOverflow,
DivisionByZero,
IntegerConversionOverflow,
NegativeShiftCount,
CoercionError,
DecodeError,
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
".decode-error" => Self::DecodeError,
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
Self::DecodeError => ".decode-error",
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
impl From<terrane_string_support::DecodeError> for TerraneError {
fn from(error: terrane_string_support::DecodeError) -> Self {
Self::new(TerraneErrorKind::DecodeError, error.to_string().trim_start_matches(".decode-error: "))
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
// Namespace: decode-error-offset
fn main() {
    let invalid: Vec<u8> = Vec::from([97, 255]);
    println!("{}", terrane_scalar_support::scalar_text(&((terrane_string_support::decode(&(invalid), terrane_string_support::Encoding::Utf8)).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/decode-error-offset::main (case.trn:4:11)"))))));
}
