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
// Namespace: independent-control-region-reads
fn main() {
    let mut across_if: i8 = 0;
    if 1 == 1 {
        across_if = 1;
    }
    if 1 == 1 {
        println!("{}", terrane_scalar_support::scalar_text(&(across_if)));
    }
    let mut across_loop: i8 = 0;
    let mut __terrane_iterator_0 = terrane_collection_support::string_iterator(&(String::from("ab")));
    loop {
        let first = match __terrane_iterator_0.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        let _ = &first;
        across_loop = 1;
    }
    let mut __terrane_iterator_1 = terrane_collection_support::string_iterator(&(String::from("cd")));
    loop {
        let second = match __terrane_iterator_1.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        let _ = &second;
        println!("{}", terrane_scalar_support::scalar_text(&(across_loop)));
    }
    let mut across_catch: i8 = 0;
    let __terrane_completion_0: TerraneCompletion<()> = (|| {
        let __terrane_try_0: TerraneCompletion<()> = (|| {
            across_catch = 7;
            return TerraneCompletion::Error(TerraneError::new(TerraneErrorKind::ArithmeticOverflow, "fixed-width integer arithmetic overflow").at("/independent-control-region-reads::main (case.trn:18:5)"));
        })();
        match __terrane_try_0 {
            TerraneCompletion::Return(value) => return TerraneCompletion::Return(value),
            TerraneCompletion::Break => return TerraneCompletion::Break,
            TerraneCompletion::Continue => return TerraneCompletion::Continue,
            TerraneCompletion::Normal => {}
            TerraneCompletion::Error(__terrane_error_0) => {
                let mut __terrane_handled_0 = false;
                if !__terrane_handled_0 && __terrane_error_0.kind == TerraneErrorKind::ArithmeticOverflow {
                    __terrane_handled_0 = true;
                    println!("{}", terrane_scalar_support::scalar_text(&(across_catch)));
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
        TerraneCompletion::Break | TerraneCompletion::Continue => __terrane_generated_defect("loop control escaped a non-loop try"),
    }
    let mut exclusive: i8 = 0;
    if 1 == 1 {
        exclusive = 9;
        let _ = &mut exclusive;
    } else {
        println!("{}", terrane_scalar_support::scalar_text(&(exclusive)));
    }
}
