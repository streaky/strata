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
// Namespace: collections-value-semantics
fn main() {
    let original: terrane_collection_support::List<terrane_int_support::Int> = terrane_collection_support::List::<terrane_int_support::Int>::new(vec![terrane_int_support::Int::from(1_i128), terrane_int_support::Int::from(2_i128)]);
    let mut independent: terrane_collection_support::List<terrane_int_support::Int> = (original).clone();
    (independent).append(terrane_int_support::Int::from(3_i128));
    let _ = ((independent).set((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(1_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:8:3)"))), terrane_int_support::Int::from(4_i128))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:8:3)")));
    println!("{}{}{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((original).length()))), terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((independent).length()))), terrane_scalar_support::scalar_text(&(((independent).get_or_error((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(2_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:9:47)"))))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:9:47)"))))), terrane_scalar_support::scalar_text(&(((independent).get_or_error((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(1_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:9:63)"))))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:9:63)"))))));
    let mut ordered: terrane_collection_support::Map<String, terrane_int_support::Int> = terrane_collection_support::Map::<String, terrane_int_support::Int>::new(vec![terrane_collection_support::Entry::new(String::from("first"), terrane_int_support::Int::from(1_i128)), terrane_collection_support::Entry::new(String::from("second"), terrane_int_support::Int::from(2_i128))]);
    (ordered).set(String::from("third"), terrane_int_support::Int::from(3_i128));
    let _ = (ordered).set(String::from("second"), terrane_int_support::Int::from(4_i128));
    let mut __terrane_iterator_0 = terrane_collection_support::Iterable::terrane_iterator(&(ordered));
    loop {
        let pair = match __terrane_iterator_0.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}{}", terrane_scalar_support::scalar_text(&((pair).key)), terrane_scalar_support::scalar_text(&((pair).value)));
    }
    println!("{}", terrane_scalar_support::scalar_text(&(((ordered).get_or_error(&(String::from("second")))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:15:10)"))))));
    let mut unique: terrane_collection_support::Set<String> = terrane_collection_support::Set::<String>::new(vec![String::from("b"), String::from("a"), String::from("b")]);
    (unique).add(String::from("c"));
    let mut __terrane_iterator_1 = terrane_collection_support::Iterable::terrane_iterator(&(unique));
    loop {
        let value = match __terrane_iterator_1.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(value)));
    }
    let pair: terrane_collection_support::Tuple<String> = terrane_collection_support::Tuple::<String>::new(vec![String::from("left"), String::from("right")]);
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((pair).length()))), terrane_scalar_support::scalar_text(&(((pair).get_or_error((terrane_collection_support::index_from_int(&(terrane_int_support::Int::from(1_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:21:23)"))))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:21:23)"))))));
    let explicit: terrane_collection_support::Entry<String, terrane_int_support::Int> = terrane_collection_support::Entry::<String, terrane_int_support::Int>::new(String::from("key"), terrane_int_support::Int::from(7_i128));
    println!("{}{}", terrane_scalar_support::scalar_text(&((explicit).key)), terrane_scalar_support::scalar_text(&((explicit).value)));
    let numbers: terrane_collection_support::Range = (terrane_collection_support::Range::new(terrane_int_support::Int::from(0_i128), terrane_int_support::Int::from(3_i128), terrane_int_support::Int::from(1_i64))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:24:13)")));
    let mut __terrane_iterator_2 = terrane_collection_support::Iterable::terrane_iterator(&(numbers));
    loop {
        let number = match __terrane_iterator_2.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(number)));
    }
    let inclusive: terrane_collection_support::Range = (terrane_collection_support::Range::through(terrane_int_support::Int::from(2_i128), terrane_int_support::Int::from(0_i128), terrane_int_support::Int::from(-1_i128))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:27:15)")));
    let mut __terrane_iterator_3 = terrane_collection_support::Iterable::terrane_iterator(&(inclusive));
    loop {
        let number = match __terrane_iterator_3.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(number)));
    }
    let mut empty_count: terrane_int_support::Int = terrane_int_support::Int::from(0_i128);
    let empty: terrane_collection_support::Range = (terrane_collection_support::Range::new(terrane_int_support::Int::from(0_i128), terrane_int_support::Int::from(3_i128), terrane_int_support::Int::from(-1_i128))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:31:11)")));
    let mut __terrane_iterator_4 = terrane_collection_support::Iterable::terrane_iterator(&(empty));
    loop {
        let ignored = match __terrane_iterator_4.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        let _ = &ignored;
        empty_count = empty_count.clone() + terrane_int_support::Int::from(1_i128);
    }
    println!("{}", terrane_scalar_support::scalar_text(&(empty_count)));
    let __terrane_completion_0: TerraneCompletion<()> = (|| {
        let __terrane_try_0: TerraneCompletion<()> = (|| {
            match terrane_collection_support::Range::new(terrane_int_support::Int::from(0_i128), terrane_int_support::Int::from(3_i128), terrane_int_support::Int::from(0_i128)) { Ok(value) => value, Err(error) => return TerraneCompletion::Error(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:36:5)")) };
            TerraneCompletion::Normal
        })();
        match __terrane_try_0 {
            TerraneCompletion::Return(value) => return TerraneCompletion::Return(value),
            TerraneCompletion::Break => return TerraneCompletion::Break,
            TerraneCompletion::Continue => return TerraneCompletion::Continue,
            TerraneCompletion::Normal => {}
            TerraneCompletion::Error(__terrane_error_0) => {
                let mut __terrane_handled_0 = false;
                if !__terrane_handled_0 {
                    __terrane_handled_0 = true;
                    println!("{}", terrane_scalar_support::scalar_text(&(String::from("zero"))));
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
    let mut deterministic_map: terrane_collection_support::UnorderedMap<String, terrane_int_support::Int> = terrane_collection_support::UnorderedMap::<String, terrane_int_support::Int>::new(vec![terrane_collection_support::Entry::new(String::from("first"), terrane_int_support::Int::from(1_i128)), terrane_collection_support::Entry::new(String::from("second"), terrane_int_support::Int::from(2_i128))]);
    let _ = (deterministic_map).set(String::from("second"), terrane_int_support::Int::from(3_i128));
    println!("{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((deterministic_map).length()))), terrane_scalar_support::scalar_text(&(((deterministic_map).get_or_error(&(String::from("second")))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:41:36)"))))));
    let mut __terrane_iterator_5 = terrane_collection_support::Iterable::terrane_iterator(&(deterministic_map));
    loop {
        let pair = match __terrane_iterator_5.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&((pair).key)));
    }
    let mut deterministic_set: terrane_collection_support::UnorderedSet<String> = terrane_collection_support::UnorderedSet::<String>::new(vec![String::from("x"), String::from("y")]);
    (deterministic_set).add(String::from("z"));
    (deterministic_set).remove(&(String::from("x")));
    println!("{}", terrane_scalar_support::scalar_text(&((deterministic_set).contains(&(String::from("y"))))));
    println!("{}", terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((deterministic_set).length()))));
    let mut __terrane_iterator_6 = terrane_collection_support::Iterable::terrane_iterator(&(deterministic_set));
    loop {
        let value = match __terrane_iterator_6.next() {
            terrane_collection_support::IterationStep::Item(item) => item,
            terrane_collection_support::IterationStep::End => break,
        };
        println!("{}", terrane_scalar_support::scalar_text(&(value)));
    }
    let mut empty_list: terrane_collection_support::List<terrane_int_support::Int> = terrane_collection_support::List::<terrane_int_support::Int>::new(Vec::new());
    (empty_list).append(terrane_int_support::Int::from(5_i128));
    let mut empty_map: terrane_collection_support::Map<terrane_int_support::Int, String> = terrane_collection_support::Map::<terrane_int_support::Int, String>::new(Vec::new());
    (empty_map).set(terrane_int_support::Int::from(1_i128), String::from("one"));
    let nested: terrane_collection_support::List<terrane_collection_support::List<terrane_int_support::Int>> = terrane_collection_support::List::<terrane_collection_support::List<terrane_int_support::Int>>::new(vec![terrane_collection_support::List::<terrane_int_support::Int>::new(vec![terrane_int_support::Int::from(8_i128), terrane_int_support::Int::from(9_i128)])]);
    println!("{}{}{}", terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((empty_list).length()))), terrane_scalar_support::scalar_text(&(((empty_map).get_or_error(&(terrane_int_support::Int::from(1_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:56:29)"))))), terrane_scalar_support::scalar_text(&(terrane_int_support::Int::from((nested).length()))));
    let arbitrary: terrane_collection_support::Map<terrane_int_support::Int, String> = terrane_collection_support::Map::<terrane_int_support::Int, String>::new(vec![terrane_collection_support::Entry::<terrane_int_support::Int, String>::new(terrane_int_support::Int::from(2_i128), String::from("two")), terrane_collection_support::Entry::<terrane_int_support::Int, String>::new(terrane_int_support::Int::from(3_i128), String::from("three"))]);
    println!("{}{}", terrane_scalar_support::scalar_text(&(((arbitrary).get_or_error(&(terrane_int_support::Int::from(2_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:58:10)"))))), terrane_scalar_support::scalar_text(&(((arbitrary).get_or_error(&(terrane_int_support::Int::from(3_i128)))).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at("/collections-value-semantics::main (case.trn:58:24)"))))));
}
