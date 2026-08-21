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
// Namespace: tuple-type-boundaries
fn echo(values: terrane_collection_support::Tuple<terrane_int_support::Int>) -> terrane_collection_support::Tuple<terrane_int_support::Int> {
    return (values).clone();
}
fn nested(values: terrane_collection_support::List<terrane_collection_support::Tuple<terrane_int_support::Int>>) -> terrane_collection_support::List<terrane_collection_support::Tuple<terrane_int_support::Int>> {
    return (values).clone();
}
fn main() {
    let pair: terrane_collection_support::Tuple<terrane_int_support::Int> = terrane_collection_support::Tuple::<terrane_int_support::Int>::new(vec![terrane_int_support::Int::from(1_i128), terrane_int_support::Int::from(2_i128)]);
    let returned: terrane_collection_support::Tuple<terrane_int_support::Int> = echo((pair).clone());
    println!("{}{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((returned).length()))), terrane_scalar_support::scalar_text(&(((returned).get_or_error((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(0_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:13:27)"))))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:13:27)"))))), terrane_scalar_support::scalar_text(&(((returned).get_or_error((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(1_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:13:40)"))))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:13:40)"))))));
    let empty: terrane_collection_support::Tuple<terrane_int_support::Int> = terrane_collection_support::Tuple::<terrane_int_support::Int>::new(Vec::new());
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((empty).length()))));
    let groups: terrane_collection_support::List<terrane_collection_support::Tuple<terrane_int_support::Int>> = terrane_collection_support::List::<terrane_collection_support::Tuple<terrane_int_support::Int>>::new(vec![terrane_collection_support::Tuple::<terrane_int_support::Int>::new(vec![terrane_int_support::Int::from(3_i128), terrane_int_support::Int::from(4_i128)]), terrane_collection_support::Tuple::<terrane_int_support::Int>::new(vec![terrane_int_support::Int::from(5_i128)])]);
    let echoed: terrane_collection_support::List<terrane_collection_support::Tuple<terrane_int_support::Int>> = nested((groups).clone());
    println!("{}{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((echoed).length()))), terrane_scalar_support::scalar_text(&(((((echoed).get_or_error((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(0_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:18:25)"))))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:18:25)")))).get_or_error((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(1_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:18:25)"))))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:18:25)"))))), terrane_scalar_support::scalar_text(&(((((echoed).get_or_error((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(1_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:18:39)"))))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:18:39)")))).get_or_error((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(0_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:18:39)"))))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/tuple-type-boundaries::main (case.trn:18:39)"))))));
}
