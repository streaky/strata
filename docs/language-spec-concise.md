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
identifier_case:
  rule: all user-declared names are lowercase - namespaces, functions, classes, interfaces, traits, fields, bindings
  form: kebab-case; 'parse-json' not 'parseJSON', which removes the acronym-casing bikeshed permanently
  rationale: case carries no semantic load in Terrane - 'is a' expresses type membership and 'receiver.member' expresses access, so case is free to constrain
  enforcement: uppercase parses, then is rejected with a precise diagnostic and formatter fixit; never silently folded
  carve_out: type parameters are uppercase ('list of T', 'map of K, V', 'iteration-step of Item') - a different KIND of name, standing in for a thing rather than naming one; never user-declared in v1 and never part of a path
charset:
  v1: ASCII only, per the version-one identifier policy
  namespaces: ASCII PERMANENTLY - non-ASCII segments hit the filesystem, where macOS NFD and Linux NFC produce different bytes for one identifier
  post_v1_extension: non-ASCII permitted in non-namespace identifiers only, and only with UAX #31 for the character set, NFC for equality, UTS #39 mixed-script confusable linting, and bidi control characters rejected outright (CVE-2021-42574, Trojan Source)
  ordering: widening an identifier set is backward compatible, narrowing is not; ship ASCII and extend later
identifier:
  version_1_characters: ASCII letters and digits only
  start: ASCII letter
  continuation: ASCII letters|digits|joiners
  joiners: punctuation admitted by normative grammar
  exact_identity: punctuation retained; no normalization
  examples_valid: [http2, sha256, ipv4-ipv6, foo+bar, sha3-256sum]
  permanent_identifier: compact letter-joiner-letter, e.g. total-count
  lexical_error: terminal joiner + digits-only unit, e.g. count-1, x+4
  slash_excluded: '/' is the namespace separator, NOT an identifier joiner; a character cannot be both without making 'namespace foo/bar' ambiguous
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
member_dot: no whitespace: value.member; '.' has NO other role in the language
invalid_adjacency: 'value member'
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
separator: '/' - one delimiter for every boundary, including the root anchor
namespace_declaration: 'namespace my-output/formatters'
root_anchor: '/leading' - same character as the separator
relative_parent: '../tier', '../../tier'; repeated parents nest as ordinary path components
relative_current: unanchored path
segment_grammar: '[a-z]([a-z0-9]|-[a-z0-9])*' - lowercase ASCII letter, then letters, digits, internal hyphens
segment_vs_identifier: DISTINCT production and a strict subset; identifier admits joiners + * % < >, a segment admits only '-'; 'foo+bar' is a legal identifier and an illegal segment; never reuse the identifier production for segments
segment_allowlist_rationale: excludes / \ : * ? " < > | NUL, control chars, leading/trailing space, dot, '.', '..' BY CONSTRUCTION; no blocklist needed
segment_reserved: con prn aux nul com1..com9 lpt1..lpt9 (Windows devices, reserved with any extension); empty segment
segment_reserved_rationale: made of legal characters, so the allowlist cannot exclude them; reserved now because adding later breaks existing names
uppercase: parses, then REJECTED semantically with a fixit; never silently lowercased (principle 5, no silent repair)
identity: exact source spelling
package_sources: manifest declares namespace-root to directory-root mappings; discovery is bounded to declared roots
filename_mapping: namespace tree corresponds to directory tree; a declaration that disagrees with its location is an ERROR unless the manifest declares that mapping
root_mapping: 'foo/bar -> ./some/path' makes foo/bar/dave resolve under ./some/path/dave
overlap: longest matching namespace prefix wins; two roots mapped to one directory is a manifest-load error
unmapped_file: a .trn file no mapping reaches is not part of the package; it is neither compiled nor reported
third_party: a dependency's namespaces come from ITS manifest and are never discovered by scanning its tree
determinism: the resolved source set is recorded in build metadata; sorted expansion, ambiguity is an error
lookup: ONE view; a name resolves through lexical scope, then namespace, then program-global, then prelude
lookup_from_a_function_body: the namespace tiers yield constants, constructs, imports, functions, types - NEVER a namespace variable; mutable state crosses a function boundary only as 'global', a parameter, or a return
dot_rule: '.' appears only between a receiver and its member, never as a name prefix
dot_exception: NONE - the dot has no other role anywhere in the language
scope: lexical + namespace
namespace_variable_scope: the namespace tier ONLY - not its own function bodies, not descendant namespaces, not importers; its role is composition at the tier
leaving_the_tier: a value that must leave is a 'constant', a 'global', or a function result
collision: different object symbols under same object name in same scope => error
shadowing: nearer binding shadows farther binding
reimport_same_export: idempotent
reimport_different_same_name: collision; alias required
```

Top-level plain assignment is namespace-local, including root namespace. `global` explicitly creates/replaces program-global identity and does not erase lexical provenance/visibility. A namespace variable is readable and writable only by other namespace-level declarations in that namespace, so `public` on one is meaningless and rejected rather than accepted as documentation.

```terrane
namespace application/commands
from /core/collections import map
private cache = map;
global shared-limit int = 10
```

## IMPORT

```terrane
use (system) sqlite
from /image/codec import resize
from /core/collections import map as ordered-map
from ../shared/config import settings
import with custom-import
```

Rules:

- `use` declares a build dependency; it does not automatically bind supplied names.
- `from ... import x` binds ordinary `x` in the scope containing the import; `as` renames it. No declare-then-bind step.
- Prelude names and descriptor constructs need NO import: `print; value` and `value int8 = 42` are complete programs. Importing `print` or `int8` is redundant, not required, and should not appear in examples or fixtures unless the case is specifically about importing.

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

- These need no import. `print; value` is a complete statement in a program with no import lines at all.
- Prelude may be disabled.
- Explicit `/core` object imports still work and may shadow/replace defaults deliberately.
- Library facilities such as `map`, `list`, `range`, `file` are NOT implicit prelude bindings; import them.
- Fixed-width numeric descriptors are NOT prelude bindings and NOT reserved words. They are descriptor constructs available without import, a separate category from the seven ordinary bindings above.

```yaml
prelude_bindings: the seven ordinary program-globals listed above; unchanged
descriptor_constructs: int8.int128, uint8.uint128, float32, float64, and the abstract category descriptors
construct_availability: usable in construct position without import ('value int8 = 42')
construct_value_use: still rejected in value position; a construct is not a runtime value
explicit_import: remains available for rebinding, aliasing, and shadowing ('from /core/types import int64 as word')
```

## CALL

```terrane
thing;                         # explicit zero-arg default call
print; message                 # positional arg
connect; host, port, timeout = 10
buffer.clear;                  # zero-arg member call
print; render                 # a bare name passes the object
print; (render; report)       # nested call MUST be grouped
```

Rules:

```yaml
call_marker: semicolon
zero_arg: semicolon required
member: receiver.member (no whitespace before dot)
adjacency: 'receiver object' invalid; NEVER invocation
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
postfix member/index/call   (NOTE: '++'/'--' are NOT here - they are update STATEMENTS)
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

```terrane
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
private cache = map;
global service = service;

function add; left int, right int; int
  return left + right

function connect; host string, port int, timeout int = 10; connection
  ...
```

- Type expression follows binding/parameter name.
- A typed binding may omit its initializer (`name string`); flow-sensitive definite assignment must prove a value before any read, reference, member access, argument pass, or capture.
- [binding-initialization-dependencies] An initializer resolves against the scope as it stands immediately BEFORE its declaration, so the declared name is not in scope from its own initializer. Where nothing else binds that name, reading it — directly or through a called function — is a compile-time error naming the absent binding. Namespace initializer dependencies, including later namespace-level assignments folded into initialization, must be statically acyclic and rejected before lowering when they form a cycle.
- [redeclaration] Where the name is already bound in the SAME LEXICAL scope, the initializer reads the earlier binding and the declaration REPLACES it: `a int8 = 12` then `a int = a`. One name means one thing at each point in a scope, read top to bottom. Lexical only — a namespace top-level declaration may not replace another, because namespace initialization is ordered by dependency, not source position.
- [redeclaration-same-type] identical type => an assignment with a redundant annotation; an outstanding ref observes the new value, since the storage is the same.
- [redeclaration-retype] type changes => the binding's type changes, and the replaced value is RELEASED at that point, after its initializer is evaluated. Deterministic and earlier than scope exit: nothing can name the old value again, so retaining it would hold a resource the program cannot reach.
- [redeclaration-v1-limit] a type-changing replacement is REJECTED while an outstanding ref observes the binding (and, once closures exist, a capture); retyping what another scope holds must not happen without something explicit there. See DEFERRED.
- [block-scope] Function bodies and every indented control-flow body create lexical scopes. A nested declaration is visible through that body and deeper scopes, never in sibling bodies or after exit; its value is released on each exit. A `for` target spans its loop body only. A nearer declaration shadows until exit, while untyped assignment to an enclosing name assigns that existing binding.
- Function return type follows parameter section.
- Default value makes parameter optional; required parameters precede optional ones; variadic captures remaining values.
- Named arguments require stable exposed parameter names.
- `constant`, not `const`.
- Default visibility public; strict visibility mode can require explicit qualifiers.
- Package-supplied declaration modifiers use a `with` clause; core structural words never do.

```yaml
form: "with per-cpu, aligned global counts unsigned-long = 0"
clause: 'with' + COMMA-separated modifiers, applied left to right, resolved in ordinary lexical scope
delimiter: the comma means another modifier follows; the list ends at the first element NOT followed by one, and the declaration begins there
no_wrapping_parens: the clause needs none; comma delimitation is sufficient
args: a modifier taking arguments is parenthesised - 'with per-cpu, (aligned; 64) global x int = 0'
args_rule: needs no special rule; a declaration always follows, so the call is never trailing and ordinary grouping applies
trailing_comma: an error - 'with per-cpu, global x = 0' reads 'global' as the next element and fails on a reserved word
scope: any declaration INCLUDING an untyped local binding; no typed-binding requirement
with_applies_to: first- and third-party package modifiers (open set)
with_never_applies_to: global, constant, public/private/protected, static/async/mutating/throws (closed set, compiler-owned)
test: can the compiler's model be described without it? if it changes name resolution, visibility, mutability, or a callable's type contract it is STRUCTURAL (keyword); if it changes only how a known declaration is realised - storage, layout, linkage, ABI, section, alignment - it is DECORATIVE (with)
ordering: with-modifiers precede structural keywords; package layer is outer
rationale: the protocol already forbids modifiers from touching visibility/ownership/effects/capability, so the split reports a real boundary; 'with global' would falsely imply global is extensible
why_exist: a declaration answers two separable questions - WHAT is declared (Terrane owns this) and HOW it is realised on a target or in a domain (modifiers own this)
why_not_macros: objects abstract what things do; modifiers abstract how declarations exist - which is why an extensibility system survives in a language that deliberately avoids macros
governing_rule: the modifier protocol is CLOSED IN ITS GUARANTEES, NOT closed in its intended vocabulary
open_ended: do not define modifiers by a list of purposes; a new domain property must not require a grammar change or a textual macro
inspectability: an unfamiliar modifier must remain answerable - what supplied it, what contract it accepts, what it adds, how it composed, what lowering resulted
origin: per-cpu is the motivating example; in C it is attributes plus linker behaviour plus accessors plus convention, in Terrane an ordinary declaration whose realisation has one instance per CPU
```
- Source-declared type parameters/generics are unsupported and MUST be rejected. Use concrete types, unions, interfaces, or generated concrete declarations.

## TYPE

Core:

```yaml
int: exact arbitrary-precision signed semantic value; adaptive representation
float: a SPELLING of float64, not a separate type; one canonical descriptor, so '.type', 'is a', reflection, and diagnostics all report float64
float_meaning: 'float' denotes THE DEFAULT PRECISION whatever that currently is; 'float64' denotes binary64 PINNED. Same descriptor this version, different meaning over time - which is what makes the default repointable
float_intent: code in 'float' moves with the language; code in 'float64' stays pinned because it must (wire format, foreign ABI, binary layout, beside float32)
float_default_reason: failure modes are asymmetric - wanting float32 and getting float64 wastes memory (found by profiling, fixed locally); wanting float64 and getting float32 computes wrong answers (integers stop round-tripping above 2^24: timestamps, byte counts, money in minor units)
float_future: may be repointed at a VERSION boundary, never by target or profile - the same source computing different results per build is what 'int' avoids by being semantically fixed
bool: true|false
string: Unicode text, UTF-8 standard representation
bytes: arbitrary binary
none: singleton absence value
void: no produced value; not storage/type erasure
opaque: hidden representation type; not void
fixed_signed: int8,int16,int32,int64,int128
fixed_unsigned: uint8,uint16,uint32,uint64,uint128
fixed_float: float32,float64
abstract: number, integer, fixed-integer, signed-fixed-integer, unsigned-fixed-integer, floating
abstract_roots: value, object (identity/ownership categories; carry no numeric members)
union: 'T|U'; none is ordinary union member
constructor: 'list of string'; arguments classified semantically as type or compile-time value
function_type: 'function from A, B to R'; associates right
```

- Values always have types; an unconstrained binding may be dynamic without weakening values. Numeric constant expressions are the exception before context: their spelling denotes a mathematical constant but a destination or typed operand selects its numeric type and arithmetic.
- Numeric values cross a single declared destination exactly or throw; mixed integer values promote exactly. Arithmetic across integer/floating values or unrelated categories remains rejected without an explicit policy conversion.
- Written coercion is object-driven and selects a different conversion policy; it is not permission merely to satisfy a numeric destination.
- Abstract descriptors are interface/category contracts exported from `/core/types`, never prelude names. `int` implements `integer` and `number`; fixed widths add `fixed-integer` plus their signedness contract; `float`/`float32`/`float64` implement `floating` and `number`. Conformance drives member attachment and finite-union reasoning; it creates no storage supertype.
- Type violations compile-time when provable.
- Conditions invoke truth protocol.
- `==` value equality; `is` source-visible identity; `is a` type membership, not numeric destination convertibility. A typed `int8` value is not an `int`; a numeric constant uses the queried type as context, so `42 is a int8` is true and an inadmissible constant answers false rather than failing. `===` invalid.
- `c is a` is identity against binding `a`; `c is a widget` is membership when complete type follows.
- Ordinary scalars/strings/collections are identity-less: `is` is false even for `x is x` and `42 is 42`. Only explicit refs, linear resources, and canonical descriptors carry identity. Exact-type-and-value comparison is `left == right and left.type is right.type`.
- Type descriptors are language constructs backed by canonical compiler-owned objects, not independently instantiated values.

```yaml
binding: REJECTED - 'd = int8' would store a type in a value slot; a construct is not a value to bind
rename: at the IMPORT only - 'from /core/types import int8 as byte'; one spelling per name in a scope, renamed where the name enters it
rename_use: the renamed construct is legal in annotation position, coercion destination, and 'is a' right side
type_in_a_value: holding a type to dispatch or instantiate through it is a DISTINCT capability belonging with reflection; it needs its own construct, not assignment syntax
value_use: REJECTED at the source span - no display or value protocol in v1 (print; d, arithmetic, value parameter)
lowering: a statically resolved descriptor needs NO runtime storage and lowers to nothing
materialisation: reflection or dynamic descriptor use may require the canonical descriptor object at runtime; 'not an ordinary value' does NOT mean 'never has a runtime representation'
defect: emitting a plain Rust binding for a descriptor, as if it were an ordinary value, is a compiler defect
backing_object: real - type returns it, 'is a' compares it, identity survives rebinding, reflection exposes it later
```

- Type descriptors are semantic objects with stable canonical identity, not ordinary values. Version-one type expressions/coercion destinations must resolve to finite compiler-known descriptor alternatives; lowering may erase the descriptor only when source behavior is unchanged.

Union destinations choose an exact type match first, otherwise the unique arm admitted by contextual constant typing or numeric destination conversion. Multiple admitted arms are a compile-time ambiguity; arm order never decides.
- `T|none` is a declared type anywhere a source type is accepted: bindings, parameters, and returns. A direct guard `value != none`, `none != value`, or `not (value is a none)` narrows that named binding to `T` in the guarded block; `and`/`or` combinations do not, and assignment invalidates the fact.

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
negative_shift: throws negative-shift-count
```

- Small multiplication computes exact `i128` intermediate; wider operations preserve exactness.
- Division by zero throws `division-by-zero`.
- Fixed widths require explicit `checked`/`wrap`/`saturate`/`overflowing` family children, never host build-mode behavior; fixed-width shift counts need their own source-language contract rather than inherited host behavior.
- A constant expression is a literal, unary-negated literal, parenthesised constant, or compile-time arithmetic combination. Its whole-number/decimal spelling does not fix a type. Typed binding initialization or assignment, a parameter default, a declared argument or return, a declared element or field, and a typed numeric operand supply context.
- Integer constant folding uses exact arithmetic with unbounded intermediates and checks only the final destination value. Floating folding performs each operation at destination precision, matching runtime arithmetic rather than rounding an exact result once; finite decimal/non-integral results may round normally, but an integral whole-number value must be exactly representable. An admitted constant materialises directly with no conversion/check. Outside context, whole-number constants are `int` and decimal constants are `float`.
- With one typed numeric operand, a constant takes that type; shift counts are exempt. Two differently typed integer values promote to the smallest integer type containing both source ranges, or `int`. Integer/floating value mixtures remain rejected.
- Numeric destinations admit every numeric source exactly or throw: range-contained widening has no representability check or conversion-error path; narrowing checks; integer-to-float requires exact representability; float-to-integer requires finite, integral, in-range input. Declared types and constant evaluation decide acceptance; range analysis may remove checks, never decide it.
- Named arithmetic families attach to `integer`: `add`, `subtract`, `multiply`, `divide`, `remainder`, `div-rem`, `negate`, `shift-left`, `shift-right`. Operators invoke each family's default child.

```yaml
policy_children: checked | wrap | saturate | overflowing
policy_receiver: wrap/saturate/overflowing attach to fixed-integer ONLY; int has no bounds to wrap or clamp
int_children: throwing default always; checked only where genuinely fallible (divide, remainder, div-rem by zero)
overflowing: returns 'overflow-result of T' with value T and overflowed bool
div_rem: returns 'div-rem-result of T' with quotient T and remainder T; default and checked only, never wrap/saturate
div_rem_reason: a wrapped quotient breaks the quotient/remainder identity the result object exists to guarantee
shift_fixed: default and checked reject counts outside the width; wrap reduces count modulo width; saturate absent
shift_int: shift-left unbounded and total; shift-right arithmetic; no count-policy children
postfix: '++' and '--' are STATEMENTS, never expressions; they produce no value
postfix_rationale: expression-valued increment is the source of C read-modify-write sequencing problems and buys nothing; write the two operations
postfix_policy: they select the default add/subtract child only; other policies need explicit assignment
```

## COERCION

```yaml
form: receiver family/policy; 'value.coerce; destination-type' | 'value.coerce.checked; destination-type'
family: invocation is the throwing default | coerce.checked | coerce.wrap | coerce.saturate
default_child: 'default' exists in compiler metadata for reflection only; source lookup of 'default' is rejected
implicit_numeric_destination: assignment/argument/return/element/field accepts exactly or throws; no written coerce required
exact_widening: source range contained by destination exact values; representation change, no representability check/conversion-error path
checked_narrowing: direct representability check; integer destination failure throws integer-conversion-overflow
float_to_integer: succeeds only for finite, integral, in-range values; otherwise integer-conversion-overflow
integer_to_float_implicit: succeeds only when this integer is exactly representable; otherwise throws
integer_to_float_written: 'value.coerce; float-type' requests IEEE round-to-nearest, ties-to-even; inexact result is ordinary
float_narrowing: exact finite values, signed zero, and signed infinity arrive with sign preserved; rounded finite values and every NaN throw integer-conversion-overflow
fixed_to_int: exact; int8..int64 and uint8..uint32 fit Small, uint64..int128 fit Wide, uint128 uses Wide below 2^127 or Big otherwise; Big may have an ordinary allocation failure but no conversion error
float_to_integer_written: NO declared coerce pair - choosing an integer for a fractional value needs a rounding mode and coerce never takes one; 'ratio.coerce; int' is absent while 'count int = ratio' is admitted
float_rounding_members: round (ties-to-even) | floor | ceiling | truncate; each yields an integer before destination conversion
float_out_of_range: written coerce throws coercion-error; never yields an infinity
string_parse: accepts exactly the destination's canonical text-display spelling
coerce_options: NONE - coerce takes only its destination; it must never grow radix or format arguments
parse_family: 'value.parse; callback' - the callback is REQUIRED; there is no built-in destination-owned parse
parse_result: result type comes from the callback's declared return, not from a destination descriptor
parse_checked: 'parse.checked; callback' catches a throwing callback and yields absence
parse_v1: callback must be a statically resolvable function name, not an arbitrary expression; resolved and inlined like a coercion destination
parse_union: a union return is checked at the destination by ordinary union rules; no parse-specific recheck
radix: third distinct operation - 'text.radix; 16' -> int (interpret), 'value.radix; 16' -> string (render)
radix_narrowing: ordinary coercion, grouped per call extent: '(text.radix; 16).coerce; int8'
declared: conversions are declared per source/destination pair; an undeclared pair is absent from the type, not a runtime failure
bool_to_int: declared, total, lossless (false 0, true 1)
int_to_bool: NOT a conversion; use an explicit comparison
failure_value: default child throws, checked returns none; neither substitutes a value
lenient_child: a total 'substitute on failure' conversion (PHP intval style, 0 for unparseable) is allowed ONLY as a separately named child, never as plain coerce; optional and unspecified in v1
callback: caller-supplied conversion callback admitted for undeclared pairs; requires function values, so later than version-one scalars
locale_parse: imported formatting facilities only, never coerce
universality: no guarantee any type coerces to any other
destination: version-one destinations resolve to finite compiler-known descriptors
lowering: contextual constants materialise directly; widening changes representation; checked conversion narrows/widens-back/compares; equivalent implicit and written integer narrowing emit equivalent checks
```

- The receiver evaluates once before policy selection and destination arguments. The whole call resolves policy availability statically; a selected family is not a storable bound-method value in version one.
- `checked` returns `T|none`; `wrap` and `saturate` exist only for supported fixed-width destinations. Flat `checked-coerce`, `wrapping-coerce`, and `saturating-coerce` spellings are invalid.

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
constant_scope: rejects reassignment regardless of lexical, namespace-local, or program-global identity tier
shadowing: a namespace-local binding may shadow a distinct program-global constant; writes resolve to the local identity
parameter_and_for_target_reassignment: allowed within lexical scope; value semantics preserve caller arguments and iterated collections
lowering_mutability: emit mutable target storage only when resolver-backed write analysis finds a reassignment
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
- `++`/`--` are statement/update operations on compatible mutable numeric bindings. They produce NO value and cannot appear in expression position; grammar places them in `update-statement`, not `postfix-expression`, and they are permitted in a three-clause `for` clause.
- Labels/goto function-local; cannot enter deeper scope or cross initialization/lifetime/cleanup unsafely.
- `match` reserved shape but outside minimum compiler milestone.

## ERROR / EFFECT

```terrane
throw error
try
  ...
catch some-error as error
  ...
finally
  ...
```

- Recoverable source throws lower primarily via Rust `Result`-like flow, not panic.
- `/core/errors` defines the standard error protocol and EXACTLY these language-mandated error objects:

```text
arithmetic-overflow          checked fixed-width result outside receiver range, incl. signed MIN / -1
division-by-zero             zero divisor for / % div-rem, every integer type and mode
integer-conversion-overflow  exact-or-throw numeric arrival cannot preserve the source value: integer out of range, fractional/NaN/infinite float into an integer, or an integer not exactly representable by a float. Name is broader than 'integer'/'overflow'; see OPEN
negative-shift-count         negative shift count on unbounded int << >>
coercion-error               coercion has no compatible result outside the overflow case above
```

- All catchable failures implement a structural `error` interface: stable `kind`, human-readable `message`, optional `cause`, and a source-context chain.
- `catch` clauses are tried in source order; a clause made unreachable by an earlier one is a compile-time diagnostic, never a silent reorder.
- `finally` always runs and may replace a pending outcome only by explicitly returning or throwing.
- Uncaught rendering prints the deterministic cause/source chain and exits through the profile's failure policy.
- Each core error is catchable, carries `message` plus structured operation/type detail, and `int` representation promotion raises none of them. Any other dotted error name (`file-error`, `not-found`, `python-error`) is package- or adapter-defined, never an implicit core error.
- Panic is separate fatal mechanism.
- Exported may-throw functions expose `throws`; non-throwing callable contracts reject may-throw implementations.
- Effect vocabulary is a closed compiler-known set plus typed thrown-error alternatives:

```yaml
effects: throws E | io | blocks | awaits | mutates | unsafe | foreign
allocates: NOT a public effect; compiler-internal fact only, because nearly every exported function allocates
allocates_profile: a no-allocation profile may require it declared where the guarantee matters
blocks_reason: retained because a blocking callee inside async code is a defect worth diagnosing
purity: a caller may call only effect-subset callees
capability: capabilities authorize effects; they are not themselves effects
inference: permitted for private functions; exported functions declare their public effect contract
```
- Uncaught errors retain source-oriented stack traces; foreign errors preserve native traceback/details.

## COLLECTION / TEXT

Core environment should provide object protocols/facilities for list, map, set, tuple, range, entry; import them explicitly from standard namespaces unless prelude changes normatively.

- List construction uses ordinary invocation; maps use named construction arguments; sets/tuples likewise object facilities.
- Indexing: `value[index]`; slices/ranges are objects.
- `for x in y` invokes iteration protocol.
- `string` is Unicode text/UTF-8; default length is grapheme count and requires capability.
- Explicit scalar and byte views avoid ambiguity.
- `bytes` distinct from `string`; encode/decode explicit.

```yaml
lookup: default child THROWS (missing-key for map, index-error for sequence); checked returns V|none
lookup_rule: absence is always the checked spelling; no operation returns absence by default
mutators: return the resulting collection for value/COW collections; none for in-place resource mutators unless a removed/replaced value is meaningful
order: map and set preserve insertion order as an observable contract
unordered_variant: a separate unordered map/set type exists for layout cost; it is DETERMINISTIC (fixed hash seed), not merely unordered
unordered_rule: the performance option must never be the nondeterministic option; it is a distinct type, not a flag
range: half-open by default; explicit 'through' constructor for inclusive ends
range_step: defaults to 1, must be non-zero; direction inconsistent with endpoints yields an empty range
inference: homogeneous literals infer the narrowest common declared type; heterogeneous require explicit union or annotation
cow: separation at first mutation visible through a non-unique value handle
hash_keys: mutable values and identity-bearing resources cannot be hash keys
iteration_step: advancing returns 'iteration-step of Item' with 'item of Item' and 'end' alternatives
iteration_end: exhaustion is NOT none, because none may be a valid item; end is sticky; advancing after end returns end
```

String members follow the same callable-family shape:

```yaml
concat: 'a.concat; b, c' -> 'abc'; appends arguments to the receiver, NO separator
join: "': '.join; a, b, c" -> 'a: b: c'; the RECEIVER is the separator (Python str.join / PHP implode shape)
join_bounds: zero args -> ''; one arg -> that arg with no separator; separator never precedes the first or follows the last part
composition_display: every argument converts through canonical text display; no display protocol is a typed error, never a silent rendering
composition_purity: neither member mutates the receiver; both return a new string
concat_vs_join: distinct operations sharing a subject, not modes of one; two members, not a family
trim: 'text.trim;' both ends | trim.start | trim.end
trim_argument: 'trim.start; "foo"' removes that literal when present, returns unchanged when absent; no separate strip-prefix member
position_children: start means logical index 0 and end the logical last scalar, for every string regardless of script
position_reason: writing direction is a display property; a string stores none, so left/right belong to a directional text type
contains: 'text.contains;' anywhere (default) | contains.start | contains.end; all boolean
contains_v1: exactly start and end; any/all await variadics or collections; 'at' awaits an index-unit decision
find: separate family; default -> text-range|none, find.all -> list of text-range, find.count -> int
empty_search: contains empty -> true; find empty -> first zero-width grapheme boundary; find.all empty -> every grapheme boundary including both ends; count = graphemes + 1
literal_search_boundary: non-empty find/contains operate on scalar sequences, not only grapheme boundaries; a match may end inside a grapheme
trim_modes: default Unicode whitespace; literal argument removes exactly one matching selected prefix/suffix
case_mapping: upper/lower default, .first, and upper.words are locale-independent Unicode operations; case-fold is explicit and locale-independent
normalise: explicit nfc/nfd/nfkc/nfkd children
split_replace: literal, left-to-right, non-overlapping; empty split -> grapheme list without synthetic empties; empty replace -> insert at every grapheme boundary including ends
family_rule: a family is modes of ONE operation, not a bucket of related operations; group by subject uses a namespace instead
case_search: no case-insensitive child; apply explicit case-fold to both operands
regex: never a child of contains; regex stays match/matches; no member dispatches on whether its argument is a pattern object
string_views: length defaults to graphemes; bytes/scalars/graphemes explicit; text-range retains immutable source with checked byte/scalar/grapheme views
bytes: immutable octets, distinct from string; b'...' literals; only \\, \', \n, \r, \t, \0, \xHH escapes; iteration -> uint8
encoding: explicit utf8/utf16-le/utf16-be/utf32-le/utf32-be; encode total; decode validates all input and throws decode-error, never replacement text
```

## OBJECT_MODEL

- Objects expose protocols rather than compiler-special-cased runtime species.
- `name` resolves through one lookup view; `value.name` is member lookup; calls explicit with `;`.
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
- Runtime independent; the structured-concurrency scope is a version-one language-level object, not a library convenience.
- Scope: explicit creation, child spawn, join; a child inherits its parent scope's deadline and may shorten but never extend it.
- Cancellation and deadlines are explicit values plus scope-boundary propagation; never ambient task-local globals.
- Channels/mutexes/atomics remain library objects.
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
from_import: binds exported objects into the containing scope via namespace/importer
lockfile: reproducible exact graph
cargo: compiler owns generated Cargo manifest/source tree
build_scripts: declarative metadata preferred; arbitrary scripts capability-gated and reported
```
```toml
package = "example.tools" # required non-empty identity
prelude = true            # optional; defaults true

[namespaces]               # required non-empty mapping table
"example/tools" = "src"
"example/generated" = "generated"
```
- Authored manifest filename: `package.toml`; syntax is TOML; unknown fields rejected.
- `namespaces`: canonical namespace-root keys mapped to distinct, relative directory roots; no absolute/parent paths. Source discovery recursively includes `.trn` files only, resolves overlapping mappings by longest namespace prefix, and assigns stable file IDs in sorted package-relative path order.
- Every discovered declaration must equal the namespace derived from its mapping and relative parent directory. Duplicate mapped directories and mapped roots containing no `.trn` files are manifest-load errors.
- A direct `.trn` CLI input is implicit package `single-file`, one unit, default prelude, and is exempt from directory correspondence.
- Compiler-bundled support source is copied content-addressably into generated builds and referenced only by generated-project-relative Cargo paths; no registry, network, or installation absolute path enters reproducible output. Apply the same vendoring mechanism to admitted authored third-party dependencies.

- Package import does not imply runtime mutation.
- Dependency graph/order deterministic.
- Separate compilation honors published representation/ABI; downstream cannot silently respecialize upstream public layout.

## CORE LIBRARY PRINCIPLE

```yaml
rule: standard facilities are written in TERRANE over a deliberately minimal Rust core
why_decisive: a Rust support crate is permanently opaque to the compiler - implementing a facility in Rust forecloses inlining, specialisation, and whole-program analysis for it forever
why_also: exercises lowering against real code; builds a corpus before a public one exists; failures surface as readable Terrane frames, which a Rust crate can never give
boundary: PER LAYER, not per facility - Rust owns the irreducible or audited layer, Terrane owns object model, policy, diagnostics, integration
example_json: Rust byte scanner beneath Terrane document model, descriptor mapping, data-path diagnostics, canonical output
example_tls: audited protocol implementation beneath Terrane stream integration, trust store, ALPN, connector policy, capability gating; NEVER reimplement TLS
rust_justified_only_if:
  - syscall/ABI boundary (fds, sockets, clocks, process control)
  - a guarantee the optimiser would destroy (constant-time compare, memory ordering, zeroisation) - not a perf judgement
  - large externally-audited security-critical implementation
  - data rather than code (Unicode tables, tz database), generated
rust_layer_rule: a layer claiming to be Rust states WHICH of the four applies
dependency_path: core libraries use the ordinary §23 mechanism - declaration plus authored wrapper; no privileged path, so they double as worked examples
profiles: core libraries declare Rust dependencies explicitly so a profile may exclude them
consequence_build: package-level artifact caching becomes load-bearing, not an optimisation
consequence_profile: capabilities become which Terrane packages are present, not which support crates were compiled in
```

## DEPENDENCY PRINCIPLE

```yaml
rule: declarations name ECOSYSTEMS and PACKAGES, never APIs
truth: the resolved manifest/lock/features/target/toolchain define the interface; nothing in the language predefines it
bridging: the build generates boundary machinery ONLY for what Terrane source actually crosses; no wholesale projection
tooling: LSP projects an ADVISORY surface (cargo metadata, rustdoc, runtime introspection); never compiler-authoritative, never invents members, never alters output
authority: the ecosystem's own toolchain - cargo/rustc, C compiler/linker, the foreign runtime
no_execution: tooling must not execute arbitrary package code to inspect it
cache_identity: manifest contents + lock checksum + features + target triple + toolchain + source checksums
rust_specialisation: no generated adapter layer, no generic instantiation translation, no trait/lifetime/error mapping; those stay in Rust and are touched only inside native Rust bodies
rust_wrapper: a Terrane-visible wrapper is authored deliberately, never generated automatically
foreign_specialisation: 'from python/x import y' names a crossing point, not an API import; adapters define boundary behaviour, not a translation of the ecosystem
```

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
- Source warnings do not fail `check`/`rust`/`build`/`run`; generated/compiler Rust warnings remain denied. Warning conformance files match code, source-relative span, severity, message, order, and multiplicity exactly.
- Binding usage is indexed once by resolved declaration identity. `W4001`: initialized local value is never read. `W4002`: initial/later store cannot reach a read before definite replacement; conditional stores do not kill incoming values. Lowering explicitly consumes dead stores and unused loop targets.
- See `docs/compiler-plan.md` for milestone sequencing; do not infer implementation status from this design reference.

## DIAGNOSTIC HOTSPOTS

Must reject with source-oriented help:

```text
print render             -> adjacency is not invocation; suggest member attachment or `print; render`
count-1                   -> lexical attached digits-only suffix; suggest `count - 1`
a+ b                      -> undeclared left-attached/postfix operator
nested; other; value      -> nested call must be parenthesized
for x=(call; a);...       -> calls in for clauses grouped
foo                       -> unresolved name when not imported or declared
value .member             -> invalid; whitespace before a member dot
list<string>              -> angle generic spelling invalid; canonical `list of string`
function f of T ...       -> source type parameters unsupported
===                       -> invalid; choose `==`, `is`, or `is a`
const                     -> invalid declaration word; use `constant`
```

## INVARIANT

Priority: these override examples/lowering sketches/plans. Condensed from full spec §41:

1. Everything semantic is object; representation can specialize invisibly.
2. Values are typed once context fixes them; dynamic != weak coercion; constraints optional/local/real. Numeric constants take destination/operand arithmetic, numeric destinations preserve the exact value or throw, and written coercion selects alternative object-driven policies.
3. Assignment value-semantic; COW allowed; `ref` shared identity; `move` ownership transfer.
4. One lookup view; imports bind ordinary names scoped to the containing block, function, or namespace.
5. Namespace segments `/`-separated, lowercase `[a-z]([a-z0-9]|-[a-z0-9])*`; `/` is both root anchor and separator, and is never an identifier character.
6. Compact operator-bearing names differ lexically from spaced operators.
7. `foo.bar` member; `.bar` object; `foo; bar` explicit argument; adjacency never call.
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
18. Package modifiers are `with`-introduced and resolved in ordinary scope; core structural words are bare keywords.
19. `void` no value; `opaque` hidden representation.
20. Derived borrow provenance never widens.
21. Source/generated/native names independent.
22. Destruction deterministic only for lexical ownership and acyclic final strong release.
23. `int` exact arbitrary precision with adaptive promotion/normalization; fixed widths expose arithmetic bounds/overflow policy; numeric destination conversion is exact-or-throw.

## OPEN

Validation/prototype points, not permission to invent semantics:

- zero-argument invocation shorthand beyond the required explicit `;` remains a possible future ergonomic study; current grammar requires `;`;
- map literal syntax;
- exact COW split policy;
- conversion-declaration coherence: conflicting declarations for one source/destination pair, and whether a declaration may be added for a type the author does not own;
- numeric arrival diagnostics: final spelling of the typed-value exact-arrival predicate (proposed `value.fits; Destination`), wording/severity of the typed false-`is a` and lossy constant-division lints, stable `T00xx` codes for contextual-constant rejections, and whether `integer-conversion-overflow` keeps its name now that it covers every exact-or-throw arrival failure;
- the version-one async surface: task identity/linearity, un-awaited task disposal, scope failure semantics for surviving siblings, defined cancellation points, and the executor boundary the language fixes versus the profile selects;
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
- additional foreign runtime adapters;
- type-changing replacement of a binding observed by a ref or capture; a later version may let the holder accept the new type explicitly, but never silently.

## AUTHORING CHECKLIST

Before writing Terrane:

1. Determine implemented subset from conformance cases, not this design.
2. Declare namespace segments with `/` separators, lowercase only; never whitespace tiers.
3. Import explicitly; `as` renames. Imports bind ordinary names directly.
4. Preserve compact punctuated identifiers; put spaces around infix operators.
5. Use `;` for every call, including zero args.
6. Prefer one call per statement. A single parenthesised call in an argument list is ordinary; two or more should be bound to named intermediates. Nesting is legal but obscures evaluation order, accumulates meaningless parentheses, and leaves diagnostics and traces pointing at an anonymous subexpression.
6. Parenthesize nested calls and calls in three-clause `for` clauses.
7. Use indentation; empty block is legal.
8. Write type after name; use `T|none`; canonical constructors use `of`.
9. Let a single numeric destination perform its exact-or-throw conversion; write `coerce` or a rounding member when selecting a different policy. Never assume foreign conversion.
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
