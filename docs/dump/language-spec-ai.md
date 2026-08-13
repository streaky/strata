# Terrane AI language and compiler reference

SOURCE_OF_TRUTH: `docs/language-spec-and-compiler-architecture-draft.md`
ROLE: lossy retrieval/index layer for AI agents; not an independent specification.
SYNC_RULE: any semantic/grammar/architecture change to SOURCE_OF_TRUTH MUST update this file in the same work unit. If they conflict, SOURCE_OF_TRUTH wins.
IMPLEMENTATION_TRUTH: executable conformance cases define implemented behavior; this file includes planned/unimplemented language.
SELF_HEAL_RULE: when this reference is missing or unclear and SOURCE_OF_TRUTH resolves the question, update this file with the smallest durable rule/index improvement that prevents recurrence. Prefer compression, replacement, or a retrieval pointer over added prose; preserve fast scanning and bounded size.

## Retrieval map

| Need | Read here | Full spec |
|---|---|---|
| write/parse source | `LEX`, `GRAMMAR`, `CALL`, `DECL`, `CONTROL` | §§6, 9, 13–14, 34 |
| names/imports | `NAMESPACE`, `IMPORT`, `PRELUDE` | §§7–8 |
| types/numbers | `TYPE`, `INTEGER`, `OPERATOR`, `COERCION` | §§11, 17 |
| display/printing | `TEXT DISPLAY` | §9.6 |
| globals/build selection | `GLOBAL / BUILD` | §§20, 26 |
| ownership | `VALUE`, `REF`, `MOVE`, `LIFETIME` | §12 |
| errors/effects | `ERROR`, `EFFECT` | §§15, 19 |
| collections/text | `COLLECTION`, `TEXT` | §16 |
| classes/protocols | `OBJECT_MODEL` | §§9, 18 |
| packages/interop | `PACKAGE`, `RUST`, `FOREIGN` | §§23–24 |
| async/targets | `ASYNC`, `TARGET` | §§21–22 |
| compiler work | `COMPILER` | §§26–33, 36, 38 |
| unsettled/deferred | `OPEN`, `DEFERRED` | §§40, 42 |
| constitutional rules | `INVARIANT` | §41 |

## STATUS

- Design specification, not claim of implementation.
- Rust is canonical lowering; no bespoke production VM required.
- Generated Rust is deterministic, readable, inspectable, source-mapped.
- Everything is semantically an object; representation may specialize when behavior is identical.
- Compiler owns source files/spans/tokens/syntax/semantic IR/diagnostics.
- Never silently repair unsupported/ambiguous source.

## LEX

```yaml
encoding: UTF-8
layout: indentation-delimited; NEWLINE/INDENT/DEDENT
empty_block: legal; no pass/no-op statement
comments: ['# line', '// line', '/* first terminator closes */']
identifier:
  version_1_characters: ASCII letters and digits only
  start: ASCII letter
  continuation: ASCII letters|digits|joiners
  joiners: punctuation admitted by normative grammar
  exact_identity: punctuation retained; no normalization
  examples_valid: [http2, sha256, ipv4/ipv6, foo+bar, sha3-256sum]
  permanent_identifier: compact letter-joiner-letter, e.g. total-count
  lexical_error: terminal joiner + digits-only unit, e.g. count-1, page/2, x+4
  fix: insert operator spaces, e.g. count - 1
operators:
  spaced_infix: 'a + b'
  compact_letter_form: 'a+b' is identifier
  left_attached: 'a+ b' requires declared postfix behavior, otherwise error
  right_attached: 'a +b' is infix because preceding whitespace starts operator
numeric_literal:
  forms: [decimal digits, one optional '.' fraction, '0x' hex run]
  separator: "'_' between digits only; never leading, trailing, doubled, or beside '.'"
  absent_in_v1: [exponent, radix prefixes other than 0x, type suffix]
  lexical_error: digit run followed by identifier characters, e.g. 1e9, 0b101, 123abc, 0x
  dot_rule: "'.' joins a literal only before a digit; 1.type stays member access"
member_dot: no whitespace: value.member
object_name: leading dot: .member
invalid_adjacency: 'value .member'
newline: normally ends statement; grammar-defined continuation only
```

Text literals:

```terrane
'single quoted default'
>rest of physical line is literal text
>>
  indentation-delimited multiline text
  common structural indentation removed
```

`>`/`>>` text is valid only in expression-start position. Tail/block text cannot be a non-final ungrouped subexpression. Preserve content exactly per full spec §6.7.

## NAMESPACE

```yaml
package_sources: manifest enumerates complete source-unit set; parse all before namespace resolution
filename_mapping: none
namespace_declaration: whitespace-separated tiers
example: 'namespace my-output formatters'
root_anchor: '/'; anchor only, NEVER separator
relative_parent: '.. tier'
relative_current: unanchored tier path
identity: exact source spelling
lookup_views:
  ordinary: foo
  object_form: .foo
  relation: two views over objects, distinct lookup rules
scope: lexical + namespace
collision: different object symbols under same object name in same scope => error
shadowing: nearer object-form symbol shadows farther symbol
reimport_same_export: idempotent
reimport_different_same_name: collision; alias required
```

Top-level plain assignment is namespace-local, including root namespace. `global` explicitly creates/replaces program-global identity and does not erase lexical provenance/visibility.

```terrane
namespace application commands
print = .print
private cache = .map;
global shared-limit int = 10
```

## IMPORT

```terrane
use (system) sqlite
from /core output import .print
from /collections import .map as .ordered-map
import with .custom-import
```

Rules:

- `use` declares a build dependency; it does not automatically bind supplied names.
- `from ... import .x` adds object-form `.x`, not ordinary `x`.
- Bind ordinary name explicitly: `print = .print`.
- Imports are structural compile-time slots, never ordinary calls/bindings.
- Importer selection is scoped; `global import with` selects program fallback.
- Ordinary binding named `import` cannot affect importer selection.
- Version-one execution: only declared precompiled/versioned host extensions run as importers/modifiers; never recursively execute arbitrary Terrane source.
- Structural stage order: manifest+lockfile -> host extensions -> imports in source order -> namespaces -> build selection -> resolve/type/modifiers.
- Import plans/inputs enter deterministic cache keys.

## PRELUDE

Version-one default ordinary program-global bindings EXACTLY:

```text
print int float bool string bytes none
```

- Prelude may be disabled.
- Explicit `/core` object imports still work and may shadow/replace defaults deliberately.
- Object-form facilities such as `.map`, `.list`, `.range`, `.file` are NOT implicitly prelude imports.
- Fixed-width numeric descriptors are exported as dot objects from `/core types`; import and bind them explicitly (for example `from /core types import .int64`, then `int64 = .int64`). They are not reserved words or prelude bindings.

## CALL

```terrane
thing;                         # explicit zero-arg default call
print; message                 # positional arg
connect; host, port, timeout = 10
buffer.clear;                  # zero-arg member call
print; .render                 # dot-object is ordinary argument
print; (.render; report)       # nested call MUST be grouped
```

Rules:

```yaml
call_marker: semicolon
zero_arg: semicolon required
member: receiver.member (no whitespace before dot)
adjacency: 'receiver .object' invalid; NEVER invocation
call_extent: call owns remainder of containing logical expression
arguments: one list, comma-separated
named_arg: identifier '=' call-free-expression
argument_calls: ungrouped calls forbidden; parenthesize nested calls
three_clause_for: its semicolons belong to for; calls in clauses parenthesized
evaluation: left-to-right
receiver: evaluated before selection
and_or: short-circuit
other_binary: both operands evaluated
default_args: call site, after supplied args, parameter order
```

## GRAMMAR

Compact precedence, high -> low:

```text
postfix/member/index/update/call
prefix: not - ~ ; ref move await consume postfix operand
* / %
+ -
<< >>
&
^
|
comparisons (non-associative)
is / is a
and
or
```

- Arithmetic/shift/bitwise/`and`/`or` associate left.
- Prefix operators associate right.
- Comparisons do not chain: use `a < b and b < c`.
- Unary `+` absent.
- `ref ref value` and `move move value` rejected.
- Parentheses override precedence and re-enable nested calls.
- Assignment target: bare mutable binding or assignable member/index path only. Receiver/indices evaluate exactly once left-to-right before value.
- Bare `name = expr`: declare where permitted if unresolved; otherwise rebind mutable resolved name.
- Qualified/uninitialized declarations use explicit binding grammar.

Canonical statement inventory (some not version-one implementation scope):

```text
namespace, use, from/import, import with
binding/declaration, assignment, expression
function, class, protocol, interface, trait
if/else, while, for-in, three-clause for
return, break, continue
goto/label
try/catch/finally, throw
yield
when build
rust block, foreign-source block
```

Compound clauses align with owner. Empty bodies legal. `return` expression optional; `throw`/`yield` expression required; version-one `break`/`continue` have no value. `try` requires catch or finally.

## DECL

```terrane
name = value
name int = 42
name string
constant max-size int = 1024
private cache = .map;
global service = .service;

function add; left int, right int; int
  return left + right

function connect; host string, port int, timeout int = 10; connection
  ...
```

- Type expression follows binding/parameter name.
- A typed binding may omit its initializer (`name string`); flow-sensitive definite assignment must prove a value before any read, reference, member access, argument pass, or capture.
- Function return type follows parameter section.
- Default value makes parameter optional; required parameters precede optional ones; variadic captures remaining values.
- Named arguments require stable exposed parameter names.
- `constant`, not `const`.
- Default visibility public; strict visibility mode can require explicit qualifiers.
- Declaration modifiers are leading object-form names resolved in dedicated object scope; bare names never modifiers.
- Source-declared type parameters/generics are unsupported and MUST be rejected. Use concrete types, unions, interfaces, or generated concrete declarations.

## TYPE

Core:

```yaml
int: exact arbitrary-precision signed semantic value; adaptive representation
float: IEEE-754 binary64
bool: true|false
string: Unicode text, UTF-8 standard representation
bytes: arbitrary binary
none: singleton absence value
void: no produced value; not storage/type erasure
opaque: hidden representation type; not void
fixed_signed: int8,int16,int32,int64,int128
fixed_unsigned: uint8,uint16,uint32,uint64,uint128
fixed_float: float32,float64
union: 'T|U'; none is ordinary union member
constructor: 'list of string'; arguments classified semantically as type or compile-time value
function_type: 'function from A, B to R'; associates right
```

- Values always have types; unconstrained binding may be dynamic without weakening values.
- No implicit cross-type arithmetic/coercion.
- Explicit coercion is object-driven.
- Type violations compile-time when provable.
- Conditions invoke truth protocol.
- `==` value equality; `is` source-visible identity; `is a` type membership/assignability. `===` invalid.
- `c is a` is identity against binding `a`; `c is a widget` is membership when complete type follows.
- Ordinary scalars/strings/collections are identity-less: `is` is false even for `x is x` and `42 is 42`. Only explicit refs, linear resources, and canonical descriptors carry identity. Exact-type-and-value comparison is `left == right and left.type is right.type`.
- Type descriptors are canonical compiler-owned values with stable identity. Version-one type expressions/coercion destinations must resolve to finite compiler-known descriptor alternatives; lowering may erase the descriptor only when source behavior is unchanged.

## INTEGER

```yaml
int_semantics: mathematical exact signed integer
runtime_tiers: i64 -> i128 -> arbitrary precision limbs
overflow: representation promotion, NOT source throw
normalization: after every operation choose smallest exact tier
fixed_width: distinct types; retain width; ordinary arithmetic checked
signed_division: Euclidean quotient/remainder
host_lowering: direct Rust operators only if complete semantics match
capability: target without arbitrary promotion must prove bounds or reject; never silently bound int
bitwise_int: infinite two's-complement
right_shift: arithmetic/flooring
left_shift: exact
negative_shift: throws .negative-shift-count
```

- Small multiplication computes exact `i128` intermediate; wider operations preserve exactness.
- Division by zero throws `.division-by-zero`.
- Fixed widths require explicit checked/wrapping/saturating/overflowing contracts, never host build-mode behavior; fixed-width shift counts need their own source-language contract rather than inherited host behavior.
- Literal initializers are range-checked against the destination over the whole constant expression; a syntactically negated minimum is accepted (`-128` as `int8`, `-2^127` as `int128`) without first rejecting its positive magnitude.
- Conversions are explicit (`coerce`, `checked-coerce`, `wrapping-coerce`, `saturating-coerce`).

## COERCION

```yaml
form: receiver operation; 'value.coerce; destination-type'
families: coerce (throwing) | checked-coerce | wrapping-coerce | saturating-coerce
integer_to_integer: per full spec §17.7; fixed-width-to-int always exact but still explicit
fixed_to_int: exact, explicit, cannot overflow
to_float: IEEE-754 round-to-nearest, ties-to-even
inexact_float: ordinary result, NOT an error; precision loss shown by destination type
float_out_of_range: infinity only when source protocol declares it, else .coercion-error
string_parse: accepts exactly the destination's canonical text-display spelling
locale_parse: imported formatting facilities only, never coerce
universality: no guarantee any type coerces to any other
destination: version-one destinations resolve to finite compiler-known descriptors
```

## TEXT DISPLAY

- Core text display returns `string`; version one implements it for strings, all integer types, all float types, booleans, and `none`, but not arbitrary `bytes`.
- Integers render base ten without grouping; floats use shortest round-trippable decimal text and preserve negative zero; booleans/absence render `true`, `false`, `none`.
- Core `print` displays arguments left-to-right and appends a newline. Unsupported display is a typed error; locale, styling, width, and precision require imported formatting facilities. Float lowering must normalize Rust's `NaN` spelling to canonical `nan` while also pinning `inf`, `-inf`, negative zero, and shortest round-trippable finite output.
- Version-one dynamic alternatives are finite and compiler-known, so protocol availability and typed-boundary compatibility are checked across all alternatives statically. Runtime display type errors are reserved for later or foreign erased dynamic values.

## VALUE / REF / MOVE / LIFETIME

```yaml
ordinary_assignment: value semantics
implementation: COW/share storage allowed if mutation cannot leak
mutation: separates backing storage before observable change
ref: explicit shared source-visible identity
move: explicit ownership transfer; source unusable afterward
borrow: bounded reference with compiler provenance; may narrow, never widen lifetime
interior_ref: separates COW, pins path, cannot escape/replace/remove while live
linear: noncopyable exclusive resource; move transfers identity
constants: cannot rebind
cleanup: deterministic lexical destruction; acyclic final strong reference release
cycles: never promise deterministic collection; reject provable cycles or diagnose/document leak
```

Distinct contracts: `ref T`, `borrowed-ref of T`, `user-ref of T`, `raw-address of T`, `array-ref of T`, `c-pointer of T`, callable ABI addresses. Never silently convert/weaken.

## CONTROL

```terrane
if condition
  ...
else
  ...

while condition
  ...

for item in things
  ...

for i = 0; i < limit; i++
  ...
```

- Three-clause calls require grouping: `for i = (start-at; limit); ...`.
- `++`/`--` are statement/update operations on compatible mutable numeric bindings.
- Labels/goto function-local; cannot enter deeper scope or cross initialization/lifetime/cleanup unsafely.
- `match` reserved shape but outside minimum compiler milestone.

## ERROR / EFFECT

```terrane
throw error
try
  ...
catch .some-error as error
  ...
finally
  ...
```

- Recoverable source throws lower primarily via Rust `Result`-like flow, not panic.
- `/core errors` defines the standard error protocol and EXACTLY these language-mandated error objects:

```text
.arithmetic-overflow          checked fixed-width result outside receiver range, incl. signed MIN / -1
.division-by-zero             zero divisor for / % div-rem, every integer type and mode
.integer-conversion-overflow  coerce to a fixed-width integer cannot represent the source value
.negative-shift-count         negative shift count on unbounded int << >>
.coercion-error               coercion has no compatible result outside the overflow case above
```

- Each is catchable, carries `message` plus structured operation/type detail, and `int` representation promotion raises none of them. Any other dotted error name (`.file-error`, `.not-found`, `.python-error`) is package- or adapter-defined, never an implicit core error.
- Panic is separate fatal mechanism.
- Exported may-throw functions expose `throws`; non-throwing callable contracts reject may-throw implementations.
- Effect metadata is inferred or declared and reflected.
- Uncaught errors retain source-oriented stack traces; foreign errors preserve native traceback/details.

## COLLECTION / TEXT

Core environment should provide object protocols/facilities for list, map, set, tuple, range, entry; import object forms explicitly from standard namespaces unless prelude changes normatively.

- List construction uses ordinary invocation; maps use named construction arguments; sets/tuples likewise object facilities.
- Indexing: `value[index]`; slices/ranges are objects.
- `for x in y` invokes iteration protocol.
- `string` is Unicode text/UTF-8; default length is grapheme count and requires capability.
- Explicit scalar and byte views avoid ambiguity.
- `bytes` distinct from `string`; encode/decode explicit.

## OBJECT_MODEL

- Objects expose protocols rather than compiler-special-cased runtime species.
- `.name` resolves object form; `value.name` member lookup; calls explicit with `;`.
- Function/class/namespace/type objects are reflectable semantic objects.
- `construct` is conventional constructor method selected by class default invocation.
- Protocol: structural capability.
- Interface: typed dispatch boundary.
- Trait: implementation reuse, not a type.
- Class: single inheritance initially; subclass-to-base assignment preserves dynamic value (no slicing).
- Overloading by implicit same-name signature dispatch is not initial behavior.
- Mutation visible by default; immutable behavior explicit via `constant`/contracts.

## GLOBAL / BUILD

- Program globals form explicit initialization graph; cycles diagnosed.
- Mutable globals used across threads must satisfy shared-thread-safe protocol.
- Prefer standard thread-local object over second global grammar.
- `when build` is deterministic compile-time selection over literals, immutable manifest/target/capability descriptors, boolean/comparison operators, compiler-provided pure queries.
- Inactive branches excluded from current build; all inputs enter cache key.

## ASYNC

- `async function` has distinct async callable type.
- `await` only inside async context.
- Sync/async callable types incompatible without explicit adapter.
- No borrow crosses suspension unless contract proves lifetime.
- Runtime independent; structured task scopes preferred.
- Channels/mutexes/atomics/task groups are library objects.
- Target capability may reject async statically.

## TARGET

- Build selects target profile/capabilities.
- Missing required capability => source diagnostic naming construct and requirement; never silently change semantics.
- Dynamic/static lowering choices may differ only with identical source behavior.
- `no_std` uses minimal support + target capabilities.
- Minimal support includes adaptive exact `int` and its normative failures when that feature lands; constrained targets prove supported bounds or reject by capability rather than changing semantics.
- Hosted convenience must not preclude allocator-free/embedded/kernel realization where capabilities permit.
- Low-level representation/ABI/pointer/volatile/atomic operations require explicit contracts and unsafe boundaries.

## PACKAGE

```yaml
origins: terrane packages | Rust crates | system/C libraries | foreign runtime packages
use: declares dependency
from_import: binds exported object forms via namespace/importer
lockfile: reproducible exact graph
cargo: compiler owns generated Cargo manifest/source tree
build_scripts: declarative metadata preferred; arbitrary scripts capability-gated and reported
```
```toml
package = "example.tools" # required non-empty identity
prelude = true            # optional; defaults true
sources = ["src/main.trn", "src/support.trn"] # required complete source set
```
- Authored manifest filename: `package.toml`; syntax is TOML; unknown fields rejected.
- `sources`: non-empty relative `.trn` paths only; no absolute/parent paths or duplicates; stable file IDs use sorted path order.
- A direct `.trn` CLI input is implicit package `single-file`, one unit, default prelude.
- Compiler-bundled support source is copied content-addressably into generated builds and referenced only by generated-project-relative Cargo paths; no registry, network, or installation absolute path enters reproducible output. Apply the same vendoring mechanism to admitted authored third-party dependencies.

- Package import does not imply runtime mutation.
- Dependency graph/order deterministic.
- Separate compilation honors published representation/ABI; downstream cannot silently respecialize upstream public layout.

## RUST

- Rust is native lowering, not foreign runtime.
- Generated identifiers use exact deterministic injective encoding; punctuation never normalized away.
- Source name, generated Rust name, native/link symbol are independent reflected identities.
- Inline Rust block/expression and maintained `.rs` files are first-class escape hatches with explicit safety/source mapping.
- Generated/handwritten Rust may call each other within one Rust crate graph.
- Rust errors/diagnostics map back to Terrane spans without hiding originals.
- Ejection tooling can produce maintainable generated Rust/Cargo artifacts.

## FOREIGN

- System/C crosses explicit ABI boundary.
- Foreign runtime adapters (e.g. Python) are explicit semantic/performance/ownership/deployment boundaries.
- Each adapter declares conversions, effects, lifetime, thread, exception, deployment contracts.
- Foreign proxies require explicit `ref` or `move`; ordinary value assignment must not pretend value isolation.
- Embedded foreign source is opaque indentation-delimited body owned by adapter with nested source map.
- C++ initially through C-compatible shims/Rust bridges; arbitrary C++ ABI deferred.

## COMPILER

Pipeline:

```text
manifest/source set
-> UTF-8 source files + stable file IDs/spans
-> lossless tokens/trivia/layout
-> lossless CST
-> compact semantic AST
-> namespace assembly/import resolution
-> names/types/effects/ownership/control-flow
-> typed semantic IR
-> Rust-oriented lowering IR
-> deterministic Rust + Cargo
-> rustc/Cargo
-> source-mapped diagnostics/artifacts
```

Contracts:

- `check`, `rust`, `build`, `run` share pipeline.
- Parse recovery never promotes recovered invalid nodes to lowering.
- Diagnostic: stable code, primary source span, labels/notes/help; originating bytes including UTF-8.
- Generated output deterministic for compiler version, target, declared inputs.
- No universal boxed `Value` shortcut; finite dynamic alternatives use closed representations when sound.
- Direct native lowering only when Rust operation exactly matches complete Terrane semantics.
- Reflection exposes semantic descriptors, source/generated/native identities, compilation artifacts subject to profile.
- Development compilation explains lowering/cost/copies/COW/ref/move/foreign transitions.
- Cache keys include source set, compiler version, target, dependencies, import/modifier plans, build selections, relevant options.
- Conformance cases are implementation truth. Accepted compile cases compile generated crates; runtime changes execute; generated-Rust goldens reviewed.
- See `docs/compiler-plan.md` for milestone sequencing; do not infer implementation status from this design reference.

## DIAGNOSTIC HOTSPOTS

Must reject with source-oriented help:

```text
print .render             -> adjacency is not invocation; suggest member attachment or `print; .render`
count-1                   -> lexical attached digits-only suffix; suggest `count - 1`
a+ b                      -> undeclared left-attached/postfix operator
nested; other; value      -> nested call must be parenthesized
for x=(call; a);...       -> calls in for clauses grouped
.foo                      -> unresolved object-form lookup when not imported
foo                       -> ordinary name never satisfied by object-form symbol
list<string>              -> angle generic spelling invalid; canonical `list of string`
function f of T ...       -> source type parameters unsupported
===                       -> invalid; choose `==`, `is`, or `is a`
const                     -> invalid declaration word; use `constant`
```

## INVARIANT

Priority: these override examples/lowering sketches/plans. Condensed from full spec §41:

1. Everything semantic is object; representation can specialize invisibly.
2. Values always typed; dynamic != weak coercion; constraints optional/local/real; coercion explicit.
3. Assignment value-semantic; COW allowed; `ref` shared identity; `move` ownership transfer.
4. Ordinary/object-form lookup distinct; imports do not auto-bind ordinary names.
5. Namespace tiers whitespace-separated; `/` root anchor only.
6. Compact operator-bearing names differ lexically from spaced operators.
7. `foo.bar` member; `.bar` object; `foo; .bar` explicit argument; adjacency never call.
8. Compile-time structural slots never depend on same-spelled ordinary bindings.
9. Empty blocks legal; conventional control flow.
10. Public/dynamic permissive defaults; explicit private/protected/strict available.
11. Rust canonical; output deterministic/readable/source-mapped; name encoding injective.
12. Native/Rust/system/foreign dependencies share inspectable graph; foreign boundaries explicit.
13. Reflection/debugging/performance explanation compiler contracts.
14. Missing target capabilities diagnose; never silently weaken semantics.
15. Equality, identity, membership distinct.
16. Build selection deterministic over declared inputs.
17. Pointer/reference/borrow/address/ABI contracts distinct; never silently weaken.
18. Modifier lookup object-form and structural.
19. `void` no value; `opaque` hidden representation.
20. Derived borrow provenance never widens.
21. Source/generated/native names independent.
22. Destruction deterministic only for lexical ownership and acyclic final strong release.
23. `int` exact arbitrary precision with adaptive promotion/normalization; fixed widths expose bounds/overflow policy.

## OPEN

Validation/prototype points, not permission to invent semantics:

- zero-argument dot-object shorthand beyond required explicit `;` remains a possible future ergonomic study; current grammar requires `;`;
- map literal syntax;
- exact COW split policy;
- dynamic finite-union representation;
- reference implementation thresholds (`borrow`/Rc/Arc/custom);
- public-by-default API lint/strict policy;
- reflection artifact embedding policy;
- importer composition/evaluation ergonomics.

## DEFERRED

Not version-one; no private incompatible syntax:

- core constructs supplied/replaced as scoped objects (including `function`); version one keeps core constructs structural;
- source-declared generics;
- compact map literals;
- stateful hot-code replacement;
- arbitrary C++ ABI integration;
- multimethod/generic-function dispatch;
- additional foreign runtime adapters.

## AUTHORING CHECKLIST

Before writing Terrane:

1. Determine implemented subset from conformance cases, not this design.
2. Declare namespace tiers with spaces; never slash separators.
3. Import object forms explicitly; bind ordinary names explicitly.
4. Preserve compact punctuated identifiers; put spaces around infix operators.
5. Use `;` for every call, including zero args.
6. Parenthesize nested calls and calls in three-clause `for` clauses.
7. Use indentation; empty block is legal.
8. Write type after name; use `T|none`; canonical constructors use `of`.
9. Use explicit coercion; never assume integer or foreign conversion.
10. Choose value assignment vs `ref` vs `move` deliberately.
11. Use `constant`, not `const`; distinguish `void`/`opaque`.
12. Do not use source generics, `===`, adjacency calls, or implicit object imports.

## MAINTENANCE CHECKLIST

When full spec changes:

1. Update affected keyed section(s) here in same work unit.
2. Update `Retrieval map` if a topic/key moved or was added.
3. Keep `INVARIANT` synchronized with full spec §41.
4. Keep `OPEN` synchronized with §40 and `DEFERRED` with §42.
5. Keep grammar/call precedence synchronized with §34.
6. Keep diagnostic hotspots synchronized with normative diagnostics/acceptance tests.
7. Never promote planned behavior to implemented; conformance remains implementation truth.
8. Search this file for superseded terms/decisions after editing.
9. Treat a forced fallback to the full spec as a retrieval defect when the answer can be captured compactly: repair the smallest relevant key/rule in the same work unit.
10. Keep size bounded: prefer replacing vague text, deduplicating, or adding a precise pointer over accumulating explanatory prose.
