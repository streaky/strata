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
// Namespace: collection-contextual-typing
fn main() {
    let values: terrane_collection_support::List<terrane_int_support::Int> = terrane_collection_support::List::<terrane_int_support::Int>::new(vec![terrane_int_support::Int::from(1_i128)]);
    let found: Option<terrane_int_support::Int> = terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(0_i128))).ok().and_then(|index| (values).get(index).cloned());
    let narrow: terrane_collection_support::List<i8> = terrane_collection_support::List::<i8>::new(vec![5, 6]);
    println!("{}", terrane_scalar_support::scalar_text(&(((narrow).get_or_error((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(0_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collection-contextual-typing::main (case.trn:8:10)"))))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collection-contextual-typing::main (case.trn:8:10)"))))));
    if found != None {
        println!("{}", terrane_scalar_support::scalar_text(&(*found.as_ref().expect("semantic optional narrowing"))));
    }
}
