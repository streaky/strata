# Terrane object surface — version one

This document maps the proposed **version-one language contract**, not the compiler's present implementation. It reorganises the language draft around the object relationships that source authors and tooling should see. The surface the compiler implements today is recorded separately in `docs/surface-today.md`.

The map is deliberately opinionated in one important respect: a member may be both a callable object and a namespace of related callable modes. Selecting `value.coerce` produces a method object; invoking that object selects its default behaviour, while selecting `value.coerce.checked` selects a child method object.

## Reading the map

```text
object
+-- child                 member lookup
+-- child; arguments      default invocation of that member object
+-- child
    +-- mode; arguments   child operation on the selected method object
```

Labels:

- **v1**: required in the proposed first usable language.
- **profile**: v1 contract, available only when the selected target/package provides its capability.
- **adapter**: supplied by an imported native or foreign adapter, not implicitly by `/core`.
- **later**: intentionally outside v1.

A type attachment such as `integer -> coerce` means every value satisfying `integer` exposes that method-object family. A child is visible only when its receiver and arguments satisfy that child's contract. Unsupported children are absent from the receiver's type; they are not runtime no-ops.

## 1. The object-contract hierarchy

Terrane says that everything is an object semantically, but it should not force every value into one boxed runtime class. The following is the source-visible contract hierarchy; the compiler may lower any statically known leaf directly to native Rust.

```text
object
+-- semantic-descriptor                         identity-bearing
|   +-- type
|   |   +-- interface
|   |   +-- class
|   |   +-- type-constructor
|   |   +-- scalar descriptors
|   +-- namespace
|   +-- package
|   +-- declared-callable
|   +-- build-profile
|   +-- capability
+-- value                                       value-assigned by default
|   +-- scalar
|   |   +-- number
|   |   |   +-- integer                         shared integer contract
|   |   |   |   +-- int                         exact, adaptive, unbounded
|   |   |   |   +-- fixed-integer
|   |   |   |       +-- signed-fixed-integer
|   |   |   |       |   +-- int8/int16/int32/int64/int128
|   |   |   |       +-- unsigned-fixed-integer
|   |   |   |           +-- uint8/uint16/uint32/uint64/uint128
|   |   |   +-- floating
|   |   |       +-- float                       canonical binary64
|   |   |       +-- float32
|   |   |       +-- float64                     same value contract as float
|   |   +-- bool
|   |   +-- none
|   +-- sequence
|   |   +-- string                              Unicode text
|   |   +-- bytes                               arbitrary octets
|   |   +-- list of T
|   |   +-- tuple ...
|   |   +-- range of T
|   +-- associative
|       +-- map of K, V
|       +-- set of T
+-- callable
|   +-- function
|   +-- bound-method
|   +-- method-family                           callable default + child modes
|   +-- class constructor/default invocation
|   +-- closure
+-- instance
|   +-- ordinary class instance                 COW value by default
|   +-- linear/resource instance                identity-bearing
|   +-- foreign proxy                           identity-bearing, adapter-owned
+-- error                                       catchable object contract
+-- iterator                                    explicit end-of-stream state
+-- reference
    +-- ref
    +-- weak-ref
```

### Why fixed-width integers do not subclass `int`

The fixed-width types and adaptive `int` should share an `integer` interface and reusable method-family definitions. They should **not** use substitutable class inheritance from `int`: `int8 + int8` may throw and returns `int8`, while `int + int` is exact and returns `int`; their bounds, layout, ABI, and arithmetic contracts differ. Treating `int8` as an `int` subtype would either permit unsound substitution or hide coercion.

The intended reuse is therefore:

```text
integer protocol/interface
+-- exact integer value/equality/order/div-rem contract
+-- shared coercion method-family shape
+-- shared bitwise operation shape
+-- int implementation
+-- fixed-integer implementation
    +-- shared bounded arithmetic mode families
    +-- per-width/per-signedness descriptor data
```

This gives us real inheritance of contracts and implementation traits without claiming that distinct numeric source types are assignment-compatible.

## 2. Universal protocols and members

Every object supports only the protocols its descriptor declares.

```text
object
+-- type -> type descriptor                      v1
+-- is / is a                                    identity / membership protocols, v1
+-- reflection                                   profile; descriptor metadata
+-- drop                                         when the object owns resources

value where equality is defined
+-- == / !=                                      v1

value where ordering is defined
+-- < / <= / > / >=                              v1

value where hashing is defined
+-- hash protocol                                v1

value where truth is defined
+-- truth protocol                               v1

iterable
+-- iteration protocol                           v1

text-display
+-- canonical display -> string                  v1
```

`print` accepts only `text-display`. In v1 that includes `string`, all integer and floating types, `bool`, and `none`; `bytes` deliberately does not implement it.

Descriptor identity is canonical. Rebinding `.int8` under an ordinary name does not create another type, and `value.type` returns the same descriptor consulted by membership, compatibility, and coercion.

## 3. Callable method objects

### 3.1 General rule

A method family is an immutable, bound callable object:

```text
selected = value.coerce
selected; Destination             # invokes selected.default
selected.checked; Destination     # looks up child, then invokes it
```

It carries the original receiver and exposes:

```text
method-family
+-- default invocation
+-- child method objects
+-- type / callable descriptor
+-- reflection metadata
    +-- receiver type
    +-- parameter and return types
    +-- effects
    +-- available child names
```

A selected family has no source-visible identity merely because it is boxed. Its receiver is evaluated exactly once, left to right, before child selection and arguments.

### 3.2 Availability and gating

Member availability is computed from all of:

1. receiver type or finite dynamic alternatives;
2. destination/argument descriptor alternatives;
3. target capabilities;
4. imported extension interfaces;
5. strictness/effect constraints.

For a finite dynamic receiver, a direct member access is valid only when every possible alternative supports a compatible family contract. A statically known destination narrows the available conversion modes. An arbitrary open runtime value is not accepted as a v1 type or coercion destination.

## 4. Core namespaces and descriptors

```text
/
+-- core
|   +-- output
|   |   +-- .print                               text-display values... -> none
|   |   +-- .panic                               message -> never/error policy
|   +-- types
|   |   +-- .object / .value                     abstract contracts
|   |   +-- .number / .integer / .fixed-integer abstract contracts
|   |   +-- .int .float .bool .string .bytes .none
|   |   +-- .int8 .int16 .int32 .int64 .int128
|   |   +-- .uint8 .uint16 .uint32 .uint64 .uint128
|   |   +-- .float32 .float64
|   |   +-- .function                            type constructor
|   |   +-- .ref .weak-ref                       type constructors
|   +-- collections
|   |   +-- .list .map .set .tuple .range .entry type constructors/constructors
|   +-- errors
|   |   +-- .error
|   |   +-- .arithmetic-overflow
|   |   +-- .division-by-zero
|   |   +-- .integer-conversion-overflow
|   |   +-- .negative-shift-count
|   |   +-- .coercion-error
|   +-- reflection                               profile
|   +-- build                                    immutable build-query objects
|   +-- concurrency                              profile
|       +-- task scopes, channels, locks, atomics, thread-local facilities
+-- source-declared package namespaces
+-- imported native package namespaces
+-- imported adapter namespaces                  e.g. python
```

The default prelude is intentionally small:

```text
print
int float bool string bytes none
```

Fixed-width numeric descriptors and collection constructors remain explicit imports. Abstract protocol descriptors should likewise not flood the prelude.

## 5. Scalar method attachment map

### 5.1 Coercion family

The canonical spelling is one callable family, attached where at least one destination is valid:

```text
source.coerce
+-- default; Destination   -> Destination         throws a typed conversion error
+-- checked; Destination   -> Destination|none    no representability throw
+-- wrap; Destination      -> Destination         modulo destination width
+-- saturate; Destination  -> Destination         clamp to destination bounds
```

Attachment and gating:

| Receiver | `default` destinations | `checked` destinations | `wrap` destinations | `saturate` destinations |
|---|---|---|---|---|
| `int` | every integer; floating; supported parsed/text forms | fixed integer and fallible parsed/text forms | fixed integer only | fixed integer only |
| fixed integer | every integer; floating; supported text forms | fixed integer and other fallible forms | fixed integer only | fixed integer only |
| floating | floating; protocol-declared integer/text forms | fallible integer/text forms | absent unless a destination protocol explicitly defines it | bounded numeric destinations where meaningful |
| `string` | numeric, bool, bytes/encoding, and descriptor-declared parse targets | every fallible parse target | absent | absent |
| `bytes` | string through an explicit encoding/decoder descriptor; adapter-declared targets | every fallible decode target | absent | absent |
| `bool` | string and explicitly declared numeric destinations only if the language settles those mappings | same fallible destinations | absent | absent |
| `none` | string and union/absence-aware destinations only | same fallible destinations | absent | absent |
| collection | explicitly declared collection/adapter destinations | fallible declared destinations | absent | absent |

The v1 numeric contracts already justify integer rows and numeric-to-float rounding. Bool-to-number, none-to-string, general collection conversion, and the exact distinction between `checked` and parse-specific result errors require conformance decisions before implementation; they must not be inferred silently.

This replaces the flat spellings `checked-coerce`, `wrapping-coerce`, and `saturating-coerce`.

### 5.2 Arithmetic families

Operators remain familiar syntax, but the named surface should use the same family shape rather than six unrelated prefixed names:

```text
fixed.add
+-- default; rhs       -> T                       checked; throws overflow
+-- checked; rhs       -> T|none
+-- wrap; rhs          -> T
+-- saturate; rhs      -> T
+-- overflowing; rhs   -> { value T, overflowed bool }

fixed.subtract / multiply / negate / divide / remainder
+-- the same mode children where the operation supports them
```

`int` attaches `add`, `subtract`, `multiply`, `negate`, `divide`, and `remainder`, but only their exact default operations; wrapping and saturation are absent because `int` has no width or bound. `/` and `%` use Euclidean semantics. All integer receivers also expose:

```text
integer.div-rem; divisor -> object
+-- quotient -> receiver integer type
+-- remainder -> receiver integer type
```

Division by zero remains an error in every mode. Fixed signed `MIN / -1` follows each mode's explicit contract. Unsigned `negate` is absent.

### 5.3 Bitwise families

```text
integer
+-- bit-and; rhs
+-- bit-or; rhs
+-- bit-xor; rhs
+-- bit-not;
+-- shift-left
|   +-- default; count
|   +-- checked/wrap/saturate only where fixed-width policy defines them
+-- shift-right
    +-- default; count
    +-- checked/wrap/saturate only where fixed-width policy defines them
```

Adaptive `int` uses infinite two's-complement semantics and exact left shift. Fixed integers operate on exactly their declared width. Host debug/release shift behaviour is never inherited.

### 5.4 Numeric descriptors and properties

```text
number value
+-- type

fixed-integer value/type descriptor
+-- bits
+-- signed
+-- minimum
+-- maximum

floating value/type descriptor
+-- bits
+-- finite / infinite / nan classification
```

These descriptor/property names are proposals for exposing already-contractual facts; their exact reflection spelling must be settled before code depends on them.

## 6. String and bytes method attachment map

### 6.1 String views and length

```text
string
+-- length -> int                                grapheme count
+-- bytes -> bytes-view
|   +-- length -> int                            UTF-8 octet count
|   +-- iteration -> byte values
+-- scalars -> scalar-view
|   +-- length -> int                            Unicode scalar count
|   +-- iteration -> scalar values
+-- graphemes -> grapheme-view
|   +-- length -> int                            same as string.length
|   +-- iteration -> grapheme strings
+-- iteration                                    graphemes by default
```

The grapheme operations are gated by the Unicode segmentation-data capability. Missing capability is a compile-time diagnostic, never a silent fallback to bytes or scalars.

### 6.2 String transformation families

```text
string.trim
+-- default;              -> string              trim both ends
+-- left;                 -> string
+-- right;                -> string
+-- start/end;            -> string              aliases are not proposed; choose one pair
+-- matching; pattern     -> string              explicit removable pattern

string.upper
+-- default;              -> string              uppercase all cased characters
+-- first;                -> string              uppercase the first applicable cased character
+-- words;                -> string              uppercase each word's first applicable cased character

string.lower
+-- default;              -> string              lowercase all cased characters
+-- first;                -> string              lowercase the first applicable cased character

string.normalise                                  Unicode-data capability; profile/later
+-- nfc / nfd / nfkc / nfkd; -> string

string.case-fold                                  Unicode-data capability; profile/later
+-- default;              -> string              locale-independent Unicode case fold

`trim`, `upper`, and `lower` illustrate the reusable method-family rule requested for v1. `upper.words` changes only the first applicable cased character in each word and preserves the remainder; it is not editorial title casing. There is deliberately no `lower.words` child without an independently useful contract, and title styling belongs in policy-driven third-party libraries. `normalise` and `case-fold` are explicit Unicode operations rather than ambient-locale behavior; ordinary equality, `contains`, and literal search compare the actual Unicode scalar content and do not silently normalize or fold case. Locale-sensitive casing and the exact definition of a “word” need named policy/locale objects; they must not silently consult process locale. Until those contracts are settled, only locale-independent Unicode default operations can be marked v1.

Other string operations form ordinary method objects unless they have genuine mode children:

```text
string
+-- concat; values implementing text/string contract -> string
+-- contains; string -> bool
+-- starts-with; string -> bool
+-- ends-with; string -> bool
+-- split; separator -> list of string
+-- replace; old, new -> string
+-- encode; encoding descriptor -> bytes
+-- coerce                                as above
```

Only `concat`, `length`, explicit views, encode/decode, and iteration are anchored by the current draft. The remaining everyday string API is a proposed v1 library surface and needs focused semantic cases.

### 6.3 Regular expressions

Regular expressions are proposed as typed pattern objects rather than specially interpreted strings. Their first attachment point is the `string` surface:

```text
regex
+-- default; pattern string, options... -> regex
+-- pattern -> string
+-- options -> regex option set

string
+-- match
|   +-- default; regex -> regex-match|none         first match
|   +-- all; regex -> iterable of regex-match
+-- matches; regex -> bool                         whole-string match
+-- replace; regex, replacement -> string
+-- split; regex -> list of string

regex-match
+-- text -> string
+-- range -> text-range
+-- groups -> indexed capture collection
+-- named -> named capture map

text-range                                      opaque range within matched text
+-- graphemes -> range of int                    half-open grapheme offsets
+-- scalars -> range of int                      half-open Unicode-scalar offsets
+-- bytes -> range of int                        half-open UTF-8-octet offsets
```

The regex object owns compilation and exposes invalid patterns as a source-oriented typed error; string methods never reinterpret an ordinary string as a regex implicitly. `regex-match.range` is a `text-range`, not an unqualified index range: it preserves grapheme, Unicode-scalar, and UTF-8-byte coordinate views relative to the matched input. Literal syntax, engine guarantees, Unicode mode, option names, capture participation, replacement-template rules, empty-match advancement, resource limits, and the exact distinction between search and whole-string matching remain to be settled in the authoritative specification. The eventual contract must not expose engine-specific backtracking behaviour as portable Terrane semantics.

### 6.4 Bytes

```text
bytes
+-- length -> int
+-- iteration -> uint8/int byte value contract
+-- decode
|   +-- default; encoding -> string               throws decoding error
|   +-- checked; encoding -> string|none           proposed
|   +-- replace; encoding, replacement -> string   proposed explicit policy
+-- slice/index through range/index protocols
+-- coerce                                          gated aliases of declared conversions
```

Encoding descriptors such as `utf8` are canonical objects, not magic strings. Arbitrary bytes never implement text display or silently become `string`.

## 7. Collections and iteration

```text
.list / list of T
+-- default invocation; values... -> list
+-- length -> int
+-- get/index; int -> T
+-- set/index assignment; int, T -> none
+-- append; T -> none
+-- iteration -> T stream
+-- slice; range -> list

.map / map of K, V
+-- default invocation; entries/named entries -> map
+-- length -> int
+-- get/index; K -> V or declared missing-key result
+-- set; K, V -> none
+-- keys / values / entries -> iterable views
+-- iteration -> entry/tuple contract

.set / set of T
+-- default invocation; values... -> set
+-- length -> int
+-- contains; T -> bool
+-- add/remove; T -> none/result
+-- iteration -> T stream

.tuple / tuple ...
+-- default invocation; values... -> tuple
+-- fixed length
+-- indexing/destructuring
+-- iteration where element contract permits

.range / range of T
+-- default invocation; start, end, optional step -> range
+-- start / end / step
+-- iteration

.entry / entry of K, V
+-- default invocation; key, value -> entry
+-- key
+-- value
```

Lists, maps, and sets are COW value objects. Mutation triggers separation where aliases exist. Tuples are fixed-length values. Iterator exhaustion is distinct from `none`.

The draft fixes these constructors and broad protocols but not every everyday method above. Missing-key behaviour, index errors, mutator return values, ordering guarantees, range endpoint inclusion, and element type inference must be settled by accepted/rejected cases before their v1 implementation.

## 8. Functions, classes, interfaces, and traits

```text
callable
+-- default invocation; positional/named arguments
+-- parameter descriptor list
+-- return descriptor
+-- effects: throws/allocates/I-O/blocks/awaits/mutation/unsafe/FFI

class descriptor
+-- default invocation -> construct
+-- static fields and methods
+-- one optional base class
+-- implemented interfaces
+-- used traits
+-- instance descriptor

class instance
+-- public/protected/private fields by scope
+-- bound method objects
+-- this
+-- drop when declared
```

- Functions and selected methods are first-class callable objects.
- `construct` is the class object's default invocation.
- `drop` is deterministic.
- Interfaces are named structural contract/type objects.
- Traits reuse implementation and are not subtyping.
- Single class inheritance preserves complete subclass state; multiple class inheritance and implicit signature overloading are later/non-v1.
- Default/named/variadic parameters, typed returns, closures, recursion, and early return are v1.
- Source-declared type parameters are later; v1 uses concrete types, unions, interfaces, and compiler/package-supplied type constructors.

## 9. Errors

```text
error
+-- message -> string
+-- cause -> error|none                            where wrapped
+-- source location / structured fields

/core errors::.arithmetic-overflow
+-- operation
+-- fixed-width type

/core errors::.division-by-zero
+-- operation
+-- numeric type

/core errors::.integer-conversion-overflow
+-- source value/type
+-- destination type

/core errors::.negative-shift-count
+-- attempted count
+-- shift operation

/core errors::.coercion-error
+-- source value/type
+-- destination type
```

`throw`, `try`, `catch`, and `finally` are v1 control flow. Ordinary language errors lower through result-like control flow, not Rust panic. `panic` is separate and profile-selectable. Package/adapter errors such as `.file-error` and `.python-error` are not implicit `/core` children.

## 10. Ownership, identity, and lifetime objects

```text
ordinary value assignment
+-- independent semantic value
+-- shared physical storage permitted via COW

ref object
+-- explicit shared identity
+-- strong reference contract

weak-ref object
+-- non-owning observation
+-- upgrade/check operation

move
+-- ownership transfer for linear/resource values

linear/resource object
+-- inherent identity
+-- no implicit copying
+-- deterministic drop
```

Scalar, string, collection, ordinary class, closure, and bound-method values have no identity merely due to boxing. Type, namespace, package, declared-function descriptors and explicit/resource identity groups do.

## 11. Control-flow and structural language objects

These constructs are syntax in v1, not replaceable prelude functions:

```text
if / else if / else
while
for ... in ...
three-clause for
break / continue
return
yield reservation (generator implementation may follow v1)
labels / goto with lifetime and definite-assignment checks
try / catch / finally / throw
when build
function / class / interface / trait declarations
namespace / import / use declarations
rust / unsafe rust / foreign runtime blocks
```

Postfix `++` and `--` are statements, not expression values. Pattern matching and user-replaceable core constructs remain later.

### 11.1 Import aliases bind only the ordinary scope

`as` on a `from ... import` selection removes the otherwise necessary object-form import followed by an ordinary binding:

```terrane
namespace integer-coercions
from /core output import .print as print
from /core types import .int8 as int8, .uint8 as uint8
```

Each selection has the form `.imported-object as ordinary-name`. It resolves the same exported object that an unaliased `.imported-object` selection would resolve, but binds that object directly under `ordinary-name` in the current ordinary scope. The alias has no leading dot, does not additionally introduce the imported spelling into the local object-form scope, and preserves the imported object's identity and visibility checks.

This replaces the following two-step pattern when only the ordinary names are wanted:

```terrane
from /core output import .print
from /core types import .int8, .uint8
print = .print
int8 = .int8
uint8 = .uint8
```

It is not declaration-modifier syntax. In particular, an import alias can never create or replace a program-global:

```terrane
# rejected
from /core output import .print as global print
```

Global creation or replacement remains an explicit `global` declaration, visibly separate from import:

```terrane
namespace foo
from /core output import .print
global print = .print
```

```terrane
namespace foo
from /core output import .print as printfoo
global print = printfoo
```

The aliased ordinary binding follows the same collision, duplicate-name, visibility, and current-scope rules as any other ordinary binding. Import syntax cannot smuggle `global`, `constant`, visibility, or any other declaration qualifier onto that binding.

## 12. Async, concurrency, and system profiles

```text
async callable -> task object
+-- await result through control-flow syntax
+-- cancellation/lifetime metadata

structured task scope
+-- child tasks
+-- cancellation propagation
+-- failure observation

profile library objects
+-- channel
+-- mutex
+-- read/write lock
+-- atomic by supported width
+-- task group
+-- thread-local facility
+-- shared collection variants
```

These are ordinary objects supplied by selected packages/profiles, not universal prelude names. Capabilities gate allocator, threads, filesystem, sockets, process spawning, dynamic loading, reflection, unwinding, clocks, entropy, floating point, Unicode data, exact-big-integer storage, and atomic widths. Unavailable semantics are rejected; profiles never quietly change a type's behaviour.

## 13. Version-one data, operating-system, and I/O objects

These are proposed v1 standard-library objects. They require explicit imports and the relevant target capability; none is a universal prelude binding. The names below map the object relationships, while exact namespace paths and detailed semantics remain specification work.

### 13.1 Date and time

```text
instant
+-- compare / subtract
+-- elapsed; later instant -> duration

duration
+-- exact seconds and subsecond component
+-- checked arithmetic

date
+-- year / month / day
+-- add; calendar duration -> date
+-- format; date-time format -> string

time-of-day
+-- hour / minute / second / subsecond
+-- format; date-time format -> string

date-time
+-- date / time / offset / zone
+-- to-instant; -> instant
+-- add; duration or calendar duration -> date-time
+-- format; date-time format -> string

time-zone
+-- canonical identifier
+-- offset-at; instant -> offset
+-- resolve; local date-time, ambiguity policy -> date-time

clock
+-- wall; -> instant
+-- monotonic; -> monotonic-instant
+-- sleep; duration -> none
+-- timeout; duration, callable, arguments... -> result
+-- deadline; duration -> deadline
+-- deadline
|   +-- at; monotonic-instant -> deadline
+-- interval; duration, options -> ticker

deadline
+-- expires-at -> monotonic-instant
+-- remaining; -> duration|none
+-- expired -> bool
+-- timeout; callable, arguments... -> result

ticker                                        linear resource
+-- next; -> tick
+-- close;

tick
+-- scheduled-at -> monotonic-instant
+-- observed-at -> monotonic-instant
+-- lateness -> duration
```

Wall time and monotonic time are distinct. Calendar arithmetic is distinct from elapsed-duration arithmetic. `sleep`, `timeout`, deadlines, and tickers use monotonic time: wall-clock changes cannot make a timeout fire early or late. A timeout returns the callable result or raises a typed timeout failure; it requires a callable/task with a defined cancellation boundary and must not pretend that arbitrary synchronous native work can be safely stopped. A deadline is an absolute monotonic bound that can be created from a duration or monotonic instant and reused across nested operations. Tickers have an explicit missed-tick/catch-up policy and deterministic close/cancellation behavior. Local-time gaps and overlaps, leap-second policy, timezone database/version, parsing, formatting, platform precision, scheduler behavior, and timer-resource limits require explicit contracts; the process locale and timezone are never silent inputs.

### 13.2 JSON and YAML

```text
document-value
+-- none / bool / document-integer / document-decimal / string / list / map variants
+-- type inspection and checked extraction

document-integer
+-- exact arbitrary-precision signed integer

document-decimal
+-- exact decimal coefficient and exponent

serializable
+-- to-document; -> document-value

deserializable
+-- from-document; document-value -> Self

document mapping                                 descriptor-driven
+-- field names and rename policy
+-- optional and default field policy
+-- unknown-field reject/retain/ignore policy

json
+-- parse; string|byte stream -> document-value
+-- write; document-value, text writer, options -> none
+-- stringify; document-value, options -> string
+-- decode; input, destination descriptor, mapping options -> Destination
+-- encode; serializable value, options -> document-value

yaml
+-- parse; string|byte stream, schema/options -> document-value
+-- parse
|   +-- all; input, schema/options -> list/iterator of document-value
+-- write; document-value, text writer, options -> none
+-- stringify; document-value, options -> string
+-- decode; input, destination descriptor, mapping options -> Destination
+-- encode; serializable value, options -> document-value
```

JSON numbers are represented as exact `document-integer` or `document-decimal` values; parsing never rounds them through Terrane `float`, and descriptor-driven decoding defines any explicit conversion to destination numeric types. YAML parsing defaults to a safe data schema: no implicit application object construction, executable tags, or unbounded aliases. `serializable`/`deserializable` and the mapping contract make typed JSON/YAML conversion visible rather than magical: descriptor-selected field names, optional/default fields, and unknown-field behavior are explicit and diagnostics identify the data path. Duplicate-key policy, map ordering, numeric/date inference, custom tags, comments/round-tripping, resource limits, and canonical output are explicit options rather than ambient behaviour.

### 13.3 URLs

```text
url
+-- default; string -> url                        parse and validate
+-- checked; string -> url|none
+-- scheme / username / password / host / port
+-- path segments
+-- query -> ordered query entries
+-- fragment
+-- origin
+-- resolve; relative reference -> url
+-- string; -> string                             canonical serialisation

url query
+-- get / get-all
+-- append / set / remove
+-- iteration -> ordered key/value entries
```

URL parsing follows one named standard and version rather than platform helpers. Percent encoding is component-aware; decoded path/query data is never confused with filesystem paths or shell text. Internationalised hosts, default ports, relative references, opaque schemes, credential display, and normalisation require exact specification.

### 13.4 Paths, filesystem metadata, and permissions

```text
path
+-- default; string|components -> path
+-- name / parent / stem / extension
+-- components
+-- join; path -> path
+-- normalise; -> path                           lexical only
+-- absolute; base -> path                       lexical resolution

filesystem                                  capability-gated effect object
+-- exists; path -> bool
+-- metadata; path -> file-metadata             follows link by declared mode
+-- symlink-metadata; path -> file-metadata     inspects link itself
+-- canonical; path -> path                      realpath/canonical target
+-- read-link; path -> path                      immediate stored target

file-metadata
+-- kind -> regular-file|directory|symlink|other
+-- size
+-- permissions
+-- modified / accessed / created -> instant|none
+-- link target through explicit read-link operation
+-- stable platform identity where available

permissions
+-- owner/group/other mode bits where supported
+-- access-control detail through explicit profile objects
```

`path` is a lexical value: its constructor, component operations, joining, normalization, and absolute resolution against a supplied base never access a filesystem. `filesystem` is the capability-bearing effect object that supplies existence, metadata, canonicalisation, and link inspection; this keeps the same `path` usable with host, virtual, sandbox-handle-tree, or remote filesystem implementations. `extension` is the final component's syntactic extension, without claiming a content type. Permission mode bits and access probes describe filesystem state; neither proves that a later operation is authorised, because ACLs, identities, mounts, and races may intervene. Lexical normalisation does not access a filesystem and must not be named `realpath`. `filesystem.canonical` follows links and therefore is not by itself a sandbox boundary: authorization-sensitive traversal and open/create operations use directory/resource handles with beneath/no-follow/same-filesystem policies so validation and use are not separated by a TOCTOU race. Symlink following is always explicit at security boundaries.

### 13.5 Files and streams

```text
file
+-- open; path, open-options -> file-handle
+-- read; path, limits/options -> bytes
+-- read
|   +-- text; path, encoding/options -> string
+-- write; path, bytes, options -> none
+-- write
|   +-- text; path, string, encoding/options -> none
|   +-- atomic; path, bytes|string, options -> none
+-- metadata / remove / rename / copy             capability-gated

file-handle                              linear resource
+-- byte-reader
+-- byte-writer
+-- seek; offset/origin -> position               where supported
+-- metadata
+-- flush;
+-- close;

byte-reader
+-- read; buffer/count -> bytes or read-result
+-- read
|   +-- exact; count -> bytes
|   +-- all; limit -> bytes
+-- end-of-stream state distinct from none

byte-writer
+-- write; bytes -> count
+-- write
|   +-- all; bytes -> none
+-- flush;

text-reader
+-- encoding/decoder state
+-- read; count -> string
+-- lines -> iterable of string

text-writer
+-- encoding/encoder state
+-- write; string -> none
+-- line; string -> none
+-- flush;

process I/O
+-- stdin -> byte-reader/text-reader
+-- stdout -> byte-writer/text-writer
+-- stderr -> byte-writer/text-writer
```

Streams expose partial reads/writes, buffering, flushing, closure, decoding failures, and end-of-stream explicitly. Convenience whole-file operations require size/resource limits. Atomic replacement, durability (`flush` versus filesystem sync), append behavior, creation races, no-follow policy, and text newline handling are separate declared options.

### 13.6 Environment, CLI arguments, and process status

```text
environment
+-- get; name -> string|none
+-- require; name -> string                       throws missing-variable error
+-- entries; -> iterable of name/value entries
+-- set / remove                                  mutable-process capability only
+-- snapshot; -> immutable environment map

process arguments
+-- executable -> path|none
+-- values -> list of string
+-- raw values -> platform argument values        profile-specific

argument parser
+-- default; argument schema -> argument parser
+-- parse; process arguments|list of string -> parsed arguments
+-- usage/help rendering
+-- typed positional, option, flag, repeat, default, and remainder descriptors
+-- structured parse errors

exit-status
+-- default; int -> exit-status
+-- success -> bool
+-- code -> int|none
+-- signal/termination detail -> profile object|none

process
+-- exit; exit-status|int -> never
+-- success / failure canonical statuses
```

Environment access, argument decoding, and process termination are explicit effects. Environment snapshots are preferred over repeated ambient reads. CLI parsing is schema-driven and separate from raw argument acquisition. `exit` defines whether and how deterministic cleanup runs; it never masquerades as an ordinary returning function. Platform-invalid Unicode arguments and environment values must not be silently replaced.

### 13.7 Networking

```text
ip-address
+-- default; string -> ip-address                 parse IPv4 or IPv6
+-- checked; string -> ip-address|none
+-- version -> ipv4|ipv6
+-- string; -> string                             canonical presentation
+-- is-loopback / is-unspecified / is-multicast

socket-address
+-- default; ip-address, port -> socket-address
+-- ip-address / port
+-- string; -> string

tcp-listener                                  canonical type object
+-- bind; socket-address, options -> tcp-listener value

tcp-listener value                            linear resource instance
+-- accept; -> tcp-stream value, peer socket-address
+-- local-address
+-- close;

tcp-stream                                    canonical type object
+-- connect; socket-address, options -> tcp-stream value

tcp-stream value                              linear resource instance
+-- byte-reader / byte-writer
+-- peer-address / local-address
+-- shutdown; read|write|both
+-- close;

udp-socket                                    canonical type object
+-- bind; socket-address, options -> udp-socket value

udp-socket value                              linear resource instance
+-- connect; socket-address
+-- send-to; bytes, socket-address -> count
+-- receive-from; limit -> bytes, peer socket-address
+-- byte-reader / byte-writer                    connected socket only
+-- local-address / close;

dns
+-- lookup; hostname, options -> list of ip-address
+-- reverse; ip-address, options -> list of hostname

tls
+-- client; tcp-stream, server-name, options -> tls-stream
+-- server; tcp-stream, server-identity, options -> tls-stream
+-- default certificate and hostname validation
+-- tls-stream -> byte-reader / byte-writer / close
```

Networking is capability-gated and uses parsed addresses rather than accepting endpoint strings at every operation. `tcp-listener`, `tcp-stream`, and `udp-socket` name canonical type objects when selected as constructors/factories (`tcp-listener.bind`, `tcp-stream.connect`, `udp-socket.bind`); their returned `… value` instances are distinct linear resources exposing lifecycle and I/O operations. DNS results are data, not proof of endpoint identity. Listener acceptance, connect, DNS, and TLS expose cancellation/timeouts through explicit operation options. TLS validates the server name and certificate chain by default; disabling verification is a separately named, capability-restricted operation, never a convenient boolean. Proxy, ALPN, trust-store, IP-literal, server certificate, UDP truncation, socket options, and platform capability semantics require exact contracts.

### 13.8 Randomness, encodings, cryptographic digests, and UUIDs

```text
random
+-- secure; -> secure-random                      operating-system entropy
+-- pseudo; seed -> pseudo-random                 reproducible, non-cryptographic

secure-random
+-- bytes; count -> bytes
+-- int; range -> int                             unbiased bounded selection
+-- uuid; -> uuid

pseudo-random
+-- bytes; count -> bytes
+-- int; range -> int
+-- split; -> pseudo-random                       deterministic child stream

hex
+-- encode; bytes -> string
+-- decode; string -> bytes
+-- checked; string -> bytes|none

base64
+-- encode; bytes, alphabet/padding options -> string
+-- decode; string, alphabet/padding options -> bytes
+-- checked; string, options -> bytes|none

hash algorithm
+-- digest; bytes|byte-reader -> digest
+-- digest
|   +-- keyed; key -> mac algorithm

digest
+-- algorithm / bytes
+-- constant-time-equals; digest -> bool
+-- hex / base64; -> string

mac algorithm
+-- sign; key, bytes|byte-reader -> mac
+-- verify; key, bytes|byte-reader, mac -> bool

uuid
+-- default; string -> uuid
+-- checked; string -> uuid|none
+-- random; secure-random -> uuid
+-- name; namespace uuid, name bytes|string, version -> uuid
+-- bytes / string
```

`secure-random` and reproducible `pseudo-random` are different types so deterministic tests cannot accidentally supply cryptographic entropy and security-sensitive code cannot quietly use a seeded generator. Hex and base64 are codecs, not string coercions. Digest and MAC algorithms are explicit descriptors, digest/MAC comparisons use their typed constant-time operation, and streaming inputs do not require buffering an entire message. Algorithm availability, output types, key handling, UUID versions, namespace constants, decoding strictness, and no-entropy profile failures require explicit specification; obsolete or weak algorithms do not become default conveniences.

### 13.9 Compression

```text
gzip
+-- compress; bytes|byte-reader, options -> bytes|byte-reader
+-- decompress; bytes|byte-reader, limits/options -> bytes|byte-reader

deflate
+-- compress; bytes|byte-reader, wrapper/options -> bytes|byte-reader
+-- decompress; bytes|byte-reader, wrapper/limits/options -> bytes|byte-reader

zstd
+-- compress; bytes|byte-reader, options -> bytes|byte-reader
+-- decompress; bytes|byte-reader, limits/options -> bytes|byte-reader
```

Compression operates on bytes and explicit byte streams, never text implicitly. Decompression requires output, nesting, and work/resource limits so a compressed input cannot silently consume unbounded memory, CPU, or disk. Wrapper/framing, concatenated members, dictionaries, checksums, trailing bytes, deterministic output, and streaming error propagation are explicit per-format contracts.

### 13.10 Structured logging

```text
logging                                       imported standard/profile package
+-- logger; name -> logger
+-- default -> logger

logger
+-- debug / info / warning / error; message, fields/options -> none
+-- with-fields; fields -> logger
+-- with-context; context -> logger
```

Logging is not a core-prelude replacement for `print`: it is a structured, capability/profile-gated application facility. Log fields retain their keys, values, source context, and severity for tracing and reflection rather than being eagerly formatted into an opaque string. Sink selection, level filtering, redaction, field-value serialization, buffering, failure behavior, and deterministic test capture require explicit profile contracts.

## 14. Packages, adapters, and foreign objects

```text
package descriptor
+-- identity/version/content
+-- namespace root
+-- dependencies and capabilities

native Terrane package
Rust crate dependency
+-- locked crate identity/version/checksum/features
+-- direct generated-Cargo dependency
+-- build-time native Rust interface from resolved package graph
+-- optional editor index/cache; not a compiler API projection
system/C adapter
foreign runtime adapter
+-- runtime/module loading
+-- proxy type descriptors
+-- explicit scalar/collection conversion
+-- errors and traceback translation
+-- ownership/thread/lifetime rules
+-- reflection/debug/profiling metadata
```

### 14.1 Rust crates and editor contracts

Rust is Terrane’s lowering language, so `use rust crate-name` adds a resolved Cargo dependency to the generated crate graph. The package selected by the manifest, Cargo resolution, features, target, and lock file is the native interface used by lowering at build time. Inline Rust and maintained Rust modules use that resolved Rust interface directly; Terrane does not predeclare, wrap, or project a high-level equivalent of the crate merely because it is a dependency.

The compiler’s reproducibility contract is the generated Cargo manifest and lock-resolved graph, not a checked-in catalogue of Rust APIs. Lowering emits deterministic Rust paths and calls against that graph, and Cargo/rustc type-checks the actual package versions and features selected for the build. A dependency change that alters an available Rust symbol is therefore a build-time interface change, diagnosed by the normal generated-Rust source mapping, rather than a stale compiler model.

Editor package knowledge is an optional, light-touch index over the same resolved graph, never an input that changes compilation. The language server obtains package/version/feature/target facts from Cargo metadata and lock data, then uses cached rustdoc JSON or Rust-analyzer for completion, signature help, hover, and documentation in inline or maintained Rust. It refreshes or invalidates that cache when the relevant manifest, lock, feature, target, or package source changes; it must not execute arbitrary package code merely to offer hints. Hints remain advisory: availability and correctness are settled by deterministic lowering and the build.

`reqwest` is the required v1 proving case. A Terrane package declares a locked `reqwest` dependency with `default-features = false` and explicit `blocking`, `rustls`, and optional `json` features; direct `reqwest::blocking` use in a native Rust body proves that the build-selected Rust interface flows through Cargo lowering without a Terrane wrapper. The language server may index that exact resolved package for Rust-native hints, but does not manufacture Terrane members or a request/result object model. The fixture uses a deterministic loopback server, compiles generated Rust with warnings denied, and runs it. Async `reqwest` awaits the general async model instead of imposing a one-off future abstraction.

## 15. Reflection and tooling-visible descriptors

When the profile retains reflection metadata, descriptors expose:

```text
type: identity, compatibility, protocols, members, ownership, capabilities
callable: parameters, return, effects, receiver, source identity
namespace/package: children, visibility, origin/version
value: source type, identity category, storage/copy facts where permitted
foreign proxy: runtime, foreign type, ownership, transition contracts
build: target, profile, capabilities, selected branches, adapter inputs
```

Debugging, tracing, profiling, and generated Rust all preserve stable source identities. Physical Rust representations are supplementary and never redefine source semantics.

## 16. Explicitly later than v1

```text
source-declared generics
general pattern matching
multiple class inheritance
implicit signature overloading/multimethods
replaceable core structural constructs
stateful hot-code replacement
time-travel/replay
arbitrary C++ ABI integration
additional foreign runtimes beyond the first Python contract
locale-policy-rich text API until deterministic policy objects are specified
```

## 17. Decisions this proposal makes

1. Related operation modes are children of one callable method object: `coerce.checked`, `coerce.wrap`, `coerce.saturate`; likewise bounded arithmetic modes.
2. Child names are concise because the parent supplies the semantic context.
3. Receiver and destination types gate the available child set statically.
4. Numeric reuse is based on an `integer` contract plus implementation traits, not unsound `int` subclassing.
5. String unit views (`bytes`, `scalars`, `graphemes`) remain objects with their own members.
6. Transform families such as `trim`, `upper`, and `lower` use default invocation plus meaningful child modes; `upper.words` is word-initial casing, not title styling.
7. Regular expressions are typed pattern objects accepted by string operations; ordinary strings are never treated as regex patterns implicitly.
8. Date/time, structured data, URLs, paths, files, streams, environment, CLI parsing, and process status are explicit imported v1 objects gated by capabilities.
9. Filesystem safety distinguishes lexical paths, canonical paths, symlink metadata, and race-resistant handle-relative operations.
10. Core namespaces remain small; profile and adapter objects do not leak into the prelude.
11. Networking, randomness, codecs, cryptographic digests/MACs, UUIDs, and compression are explicit imported v1 facilities gated by capabilities.
12. Secure entropy and reproducible pseudo-randomness are incompatible typed sources; no implicit conversion bridges them.
13. TLS certificate and hostname validation, bounded decompression, parsed network addresses, and constant-time typed digest/MAC comparison are secure defaults rather than optional afterthoughts.
14. `use rust` creates a direct locked Cargo dependency whose manifest/feature/target/lock-resolved interface is consumed by deterministic native Rust lowering, not projected into a static Terrane API.
15. Rust-aware editor support indexes that resolved graph through Cargo/rustdoc or Rust-analyzer for advisory native-Rust hints; it neither changes compilation nor manufactures high-level imported objects.
16. Every proposed convenience method that is not fixed by the draft needs a semantic conformance decision before being claimed as v1 implemented behaviour.
