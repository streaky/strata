// Generated deterministically by Terrane <version>.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerraneErrorKind {
ArithmeticOverflow,
DivisionByZero,
IntegerConversionOverflow,
NegativeShiftCount,
CoercionError,
DecodeError,
IndexError,
MissingKey,
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
".index-error" => Self::IndexError,
".missing-key" => Self::MissingKey,
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
Self::IndexError => ".index-error",
Self::MissingKey => ".missing-key",
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
impl From<terrane_collection_support::IndexError> for TerraneError {
fn from(error: terrane_collection_support::IndexError) -> Self {
Self::new(TerraneErrorKind::IndexError, error.to_string())
}
}
impl From<terrane_collection_support::MissingKey> for TerraneError {
fn from(error: terrane_collection_support::MissingKey) -> Self {
Self::new(TerraneErrorKind::MissingKey, error.to_string())
}
}
impl From<terrane_collection_support::RangeStepError> for TerraneError {
fn from(error: terrane_collection_support::RangeStepError) -> Self {
Self::new(TerraneErrorKind::SourceError, error.to_string())
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
// Namespace: parameter-dead-store-warnings
fn take(mut v: terrane_collection_support::List<terrane_int_support::Int>) {
    let _ = &v;
    v = terrane_collection_support::List::<terrane_int_support::Int>::new(vec![terrane_int_support::Int::from(7_i128), terrane_int_support::Int::from(8_i128)]);
    println!("{}", terrane_scalar_support::scalar_text(&(((v).get_or_error((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(1_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/parameter-dead-store-warnings::take (case.trn:6:10)"))))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/parameter-dead-store-warnings::take (case.trn:6:10)"))))));
}
fn scalar(mut v: terrane_int_support::Int) {
    let _ = &v;
    v = terrane_int_support::Int::from(4_i128);
    println!("{}", terrane_scalar_support::scalar_text(&(v)));
}
fn unused(v: terrane_collection_support::List<terrane_int_support::Int>) {
    let _ = &v;
    println!("{}", terrane_scalar_support::scalar_text(&(String::from("ignored"))));
}
fn optional(v: terrane_int_support::Int) {
    println!("{}", terrane_scalar_support::scalar_text(&(v)));
}
fn main() {
    take(terrane_collection_support::List::<terrane_int_support::Int>::new(vec![terrane_int_support::Int::from(1_i128), terrane_int_support::Int::from(2_i128)]));
    scalar(terrane_int_support::Int::from(3_i128));
    unused(terrane_collection_support::List::<terrane_int_support::Int>::new(vec![terrane_int_support::Int::from(1_i128), terrane_int_support::Int::from(2_i128)]));
    optional(terrane_int_support::Int::from(7_i128));
    optional(terrane_int_support::Int::from(5_i128));
}
