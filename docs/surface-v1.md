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

Descriptor identity is canonical. Rebinding `int8` under an ordinary name does not create another type, and `value.type` returns the same descriptor consulted by membership, compatibility, and coercion.

## 3. Callable method objects

### 3.1 General rule

A method family is an immutable, bound callable object:

```terrane
selected = value.coerce
selected; Destination             # the bare invocation is the throwing default
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
|   |   +-- print                               text-display values... -> none
|   |   +-- panic                               message -> never/error policy
|   +-- types
|   |   +-- object / value                     abstract contracts
|   |   +-- number / integer / fixed-integer abstract contracts
|   |   +-- int float bool string bytes none
|   |   +-- int8 int16 int32 int64 int128
|   |   +-- uint8 uint16 uint32 uint64 uint128
|   |   +-- float32 float64
|   |   +-- function                            type constructor
|   |   +-- ref weak-ref                       type constructors
|   +-- core collections
|   |   +-- list map set tuple range entry type constructors/constructors
|   +-- errors
|   |   +-- error
|   |   +-- arithmetic-overflow
|   |   +-- division-by-zero
|   |   +-- integer-conversion-overflow
|   |   +-- negative-shift-count
|   |   +-- coercion-error
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

### 5.1 Conversion: `coerce`, `parse`, and `radix`

Three distinct operations, deliberately not overlapping.

```text
source.coerce
+-- invocation; Destination -> Destination        throws a typed conversion error
+-- checked; Destination    -> Destination|none   no representability throw
+-- wrap; Destination       -> Destination        modulo destination width
+-- saturate; Destination   -> Destination        clamp to destination bounds
```

The bare invocation *is* the throwing default. `.default` exists in compiler metadata so reflection can describe the family uniformly, but source lookup of `.default` is rejected: one operation, one spelling.

**`coerce` takes no argument beyond its destination, ever.** It must never acquire a radix or format option. That is an invariant of the design, not a description of the current surface — acquiring one would absorb the role of `parse` and collapse the separation below.

Attachment and gating:

| Receiver | invocation destinations | `checked` | `wrap` | `saturate` |
|---|---|---|---|---|
| `int` | every integer; floating | fixed integer; floating | fixed integer only | fixed integer only |
| fixed integer | every integer; floating | fixed integer; floating | fixed integer only | fixed integer only |
| floating | floating; integer under an explicitly named rounding operation | the same fallible destinations | absent | bounded numeric destinations where meaningful |
| `string` | numeric destinations, from the canonical base-ten text spelling | the same numeric destinations | absent | absent |
| `bool` | integer destinations: `false` is `0`, `true` is `1`, total and lossless | not applicable; the conversion cannot fail | absent | absent |
| `bytes` | absent — text and bytes convert only through an explicit encoding object | absent | absent | absent |
| `none` | absent | absent | absent | absent |
| collection | declared sequence/map/set contracts with a statically known item conversion | the same declared destinations | absent | absent |

Integer to floating rounds to nearest, ties to even, and throws when the magnitude falls outside the destination's finite range rather than yielding an infinity. Integer to `bool` is *not* a conversion: it is a predicate choice and must be written as a comparison. Number to `string` uses the canonical text/display operation that `print` consumes, not `coerce`. No conversion substitutes a default value for a failure; a total substitute-on-failure conversion is permitted only as a separately named child.

Conversions are declared per source/destination pair rather than universal, so an undeclared pair is absent from the type rather than a runtime failure.

```text
source.parse
+-- invocation; callback -> the callback's declared return
+-- checked; callback    -> that return, plus absence when the callback throws
```

`parse` is the user-supplied interpretation path and **always requires a callback**. There is no built-in destination-owned `parse`; without a callback there would be no operation to perform. It is the only member typed by an argument's signature rather than by a destination descriptor. A union return is checked at the destination by ordinary union rules, reported statically from the callback's declaration. In version one the callback must be a statically resolvable function name, so it resolves and inlines like a coercion destination.

```text
text.radix; base    -> int        interpret base-N text
value.radix; base   -> string     render a number in base N
```

`radix` is a third operation attached by receiver, belonging to neither family. Narrowing after interpretation is ordinary coercion and follows the call-extent rule: `(text.radix; 16).coerce; int8`.

Flat spellings such as `checked-coerce` are not valid syntax; a policy is always a child of the family.

### 5.2 Arithmetic families

Operators remain familiar syntax and select each family's default child. The named surface uses the family shape rather than prefixed names:

```text
fixed.add
+-- invocation; rhs    -> T                      throws overflow
+-- checked; rhs       -> T|none
+-- wrap; rhs          -> T
+-- saturate; rhs      -> T
+-- overflowing; rhs   -> overflow-result of T   with value T and overflowed bool

fixed.subtract / multiply / negate / divide / remainder / shift-left / shift-right
+-- the same policy children where the operation supports them
```

`wrap`, `saturate`, and `overflowing` attach to `fixed-integer` only. Adaptive `int` has no bounds to wrap or clamp against, so those children are absent from its type rather than present as runtime no-ops. `int` exposes its throwing invocation always, and `checked` only where an operation is genuinely fallible: `divide`, `remainder`, and `div-rem` by zero.

```text
integer.div-rem; divisor -> div-rem-result of T
+-- quotient  -> T
+-- remainder -> T
```

`div-rem` exposes only its invocation and `checked`. `wrap` and `saturate` are absent even on fixed-width receivers, because a wrapped or clamped quotient no longer satisfies the quotient/remainder identity the result object exists to guarantee. Both operands evaluate once and one backend operation is performed.

`/` and `%` use Euclidean semantics. Division by zero throws under every policy and is never converted into a wrapped or saturated value. Fixed signed `MIN / -1` follows each policy's contract. Unsigned `negate` is absent. Postfix `++` and `--` are statements rather than expressions: they produce no value, and there is no form yielding the previous or updated result. They select the default `add`/`subtract` child only.

### 5.3 Bitwise families

```text
integer
+-- bit-and; rhs
+-- bit-or; rhs
+-- bit-xor; rhs
+-- bit-not;
+-- shift-left  (see 5.2; the policy children live with the arithmetic families)
+-- shift-right (see 5.2)
```

Shifts accept a non-negative count. On a fixed-width receiver the invocation and `checked` reject counts outside the width and `wrap` reduces the count modulo the width; `saturate` is absent, because saturating a shift *count* has no coherent value contract. Adaptive `int` uses infinite two's-complement semantics, `shift-left` is unbounded and total, `shift-right` is arithmetic, and no count-policy children exist. Host debug/release shift behaviour is never inherited.

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
+-- invocation;           -> string              trim both ends
+-- start;                -> string              trim the leading whitespace run
+-- start; literal        -> string              remove that literal when present, unchanged when absent
+-- end;                  -> string              trim the trailing whitespace run
+-- end; literal          -> string              remove that literal when present, unchanged when absent

string.upper
+-- invocation;           -> string              uppercase all cased characters
+-- first;                -> string              uppercase the first applicable cased character
+-- words;                -> string              uppercase each word's first applicable cased character

string.lower
+-- invocation;           -> string              lowercase all cased characters
+-- first;                -> string              lowercase the first applicable cased character

string.normalise                                  Unicode-data capability; profile/later
+-- nfc / nfd / nfkc / nfkd; -> string

string.case-fold                                  Unicode-data capability; profile/later
+-- invocation;           -> string              locale-independent Unicode case fold
```

`start` and `end` denote positions in logical scalar order — `start` is index 0 — for every string regardless of script. Writing direction is a display property, not an encoding property: a `string` stores none, so `left` and `right` belong to a directional/rendered text type that carries an explicit base direction, never to `string`. The same two children are reused unchanged by `contains` below, and any family constrainable to one end of the sequence uses this pair rather than coining a near-synonym. Removing a known prefix therefore needs no separate member: `trim.start; "foo"` covers it.

Any operation whose correct answer depends on how text is drawn — padding, alignment, ellipsis truncation, column layout — is a rendering operation and does not attach to `string` at all, under any name.

`trim`, `upper`, and `lower` illustrate the reusable method-family rule requested for v1. `upper.words` changes only the first applicable cased character in each word and preserves the remainder; it is not editorial title casing. There is deliberately no `lower.words` child without an independently useful contract, and title styling belongs in policy-driven third-party libraries. `normalise` and `case-fold` are explicit Unicode operations rather than ambient-locale behavior; ordinary equality, `contains`, and literal search compare the actual Unicode scalar content and do not silently normalize or fold case. Locale-sensitive casing and the exact definition of a “word” need named policy/locale objects; they must not silently consult process locale. Until those contracts are settled, only locale-independent Unicode default operations can be marked v1.

Other string operations form ordinary method objects unless they have genuine mode children:

```text
string
+-- concat; values... -> string                       append to receiver, no separator; 'a'.concat; 'b','c' = 'abc'
+-- join; values... -> string                        receiver is the SEPARATOR; ': '.join; 'a','b' = 'a: b'
|   zero args -> ''; one arg -> that arg, no separator; never leads or trails
+-- contains                                         literal substring predicates
|   +-- invocation; string -> bool                   occurs anywhere
|   +-- start; string -> bool                        occurs at the logical start
|   +-- end; string -> bool                          occurs at the logical end
+-- find                                             position-returning search
|   +-- invocation; string -> text-range|none
|   +-- all; string -> iterator of text-range
|   +-- count; string -> int
+-- split; separator -> list of string
+-- replace; old, new -> string
+-- encode; encoding descriptor -> bytes
+-- coerce / parse / radix                as above
```

`contains` and its children are boolean; `find` is a separate family because it returns a position rather than a predicate. A family is *modes of one operation*, not a bucket of related operations, so the two do not merge — they share a subject, and grouping by subject is what namespaces are for. Every child of `contains` accepts the empty pattern and returns true for it. There is no case-insensitive child: apply an explicit `case-fold` to both operands. There is no regex child either, so no member dispatches on whether its argument is a literal or a pattern object.

The version-one `contains` family is exactly `start` and `end`. `any` and `all` over several patterns await variadics or collections; `at`, taking an explicit position, is the true generalisation but forces a byte/scalar/grapheme index-unit choice and awaits the `text-range` contract.

Only `concat`, `length`, explicit views, encode/decode, and iteration are anchored by the current draft. The remaining everyday string API is a proposed v1 library surface and needs focused semantic cases.

### 6.3 Regular expressions

Regular expressions are proposed as typed pattern objects rather than specially interpreted strings. Their first attachment point is the `string` surface:

```text
regex
+-- invocation; pattern string, options... -> regex
+-- pattern -> string
+-- options -> regex option set

string
+-- match
|   +-- invocation; regex -> regex-match|none         first match
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
|   +-- invocation; encoding -> string               throws decoding error
|   +-- checked; encoding -> string|none           proposed
|   +-- replace; encoding, replacement -> string   proposed explicit policy
+-- slice/index through range/index protocols
+-- coerce                                          gated aliases of declared conversions
```

Encoding descriptors such as `utf8` are canonical objects, not magic strings. Arbitrary bytes never implement text display or silently become `string`.

## 7. Collections and iteration

```text
list / list of T
+-- default invocation; values... -> list
+-- length -> int
+-- get
|   +-- invocation; int -> T                     throws index-error
|   +-- checked; int -> T|none
+-- set/index assignment; int, T -> none
+-- append; T -> none
+-- iteration -> T stream
+-- slice; range -> list

map / map of K, V
+-- default invocation; entries/named entries -> map
+-- length -> int
+-- get
|   +-- invocation; K -> V                       throws missing-key
|   +-- checked; K -> V|none
+-- set; K, V -> none
+-- keys / values / entries -> iterable views
+-- iteration -> entry/tuple contract

set / set of T
+-- default invocation; values... -> set
+-- length -> int
+-- contains; T -> bool
+-- add/remove; T -> none/result
+-- iteration -> T stream

tuple / tuple ...
+-- default invocation; values... -> tuple
+-- fixed length
+-- indexing/destructuring
+-- iteration where element contract permits

range / range of T
+-- default invocation; start, end, optional step -> range
+-- start / end / step
+-- iteration

entry / entry of K, V
+-- default invocation; key, value -> entry
+-- key
+-- value
```

Lists, maps, and sets are COW value objects, separating at the first mutation visible through a non-unique handle. Tuples are fixed-length values.

Lookup and indexing follow the family convention used everywhere else: the invocation throws (`missing-key` for a map, `index-error` for a sequence) and `checked` returns absence. Absence is always the `checked` spelling — no operation returns absence by default, and there is no separately named `get-required`.

Maps and sets preserve insertion order as an observable contract. A separate unordered map and set type exists for cases where the index-map layout costs too much; it is deterministic under a fixed hash seed rather than merely unordered, because the performance option must never be the nondeterministic one. It is a distinct type rather than a flag, so the guarantee stays visible in signatures.

Ranges are half-open by default with an explicit `through` constructor for inclusive ends; the step defaults to `1`, must be non-zero, and a direction inconsistent with the endpoints yields an empty range. Homogeneous literals infer the narrowest common declared type; heterogeneous literals require an explicit union or annotation. Mutable values and identity-bearing resources cannot be hash keys.

Iteration advances through a dedicated finite result:

```text
iterator.next; -> iteration-step of Item
+-- item of Item
+-- end
```

Exhaustion is `end`, never `none`, because `none` may be a legitimate item. Iterators are stateful linear objects, `end` is sticky, and advancing after `end` returns `end` without consulting the source again. `for` desugars through this protocol and neither exposes nor synthesises a sentinel.

## 8. Functions, classes, interfaces, and traits

```text
callable
+-- default invocation; positional/named arguments
+-- parameter descriptor list
+-- return descriptor
+-- effects: throws E / io / blocks / awaits / mutates / unsafe / foreign
|   allocation is compiler-internal, NOT public vocabulary

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
error                                              structural interface; all catchable failures implement it
+-- kind -> stable matchable identity
+-- message -> string                              for humans; never a matching key
+-- cause -> error|none                            where wrapped
+-- source-context chain

/core/errors::arithmetic-overflow
+-- operation
+-- fixed-width type

/core/errors::division-by-zero
+-- operation
+-- numeric type

/core/errors::integer-conversion-overflow
+-- source value/type
+-- destination type

/core/errors::negative-shift-count
+-- attempted count
+-- shift operation

/core/errors::coercion-error
+-- source value/type
+-- destination type
```

`throw`, `try`, `catch`, and `finally` are v1 control flow. Ordinary language errors lower through a compiler-owned result propagation representation, not Rust panic or native unwinding. `catch` clauses are tried in source order, and a clause made unreachable by an earlier one is a compile-time diagnostic rather than a silent reorder; `finally` always runs and may replace a pending outcome only by explicitly returning or throwing. Uncaught rendering prints the deterministic cause and source chain, then exits through the profile's failure policy. `panic` is separate and profile-selectable. Package/adapter errors such as `file-error` and `python-error` are not implicit `/core` children.

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

```terrane
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

### 11.0 Namespace paths and the prelude

`/` is the only namespace boundary marker: it anchors the root and separates every segment.

```terrane
namespace my-app/http/handlers
from /image/codec import resize
from ../shared/config import settings
from ../../platform import clock
```

A segment is `[a-z][a-z0-9-]*` — lowercase ASCII letter, then letters, digits, and internal hyphens. The allowlist makes every filesystem-hazardous character unformable rather than rejected, and `/` is therefore not an identifier character: `ipv4-ipv6`, never `ipv4/ipv6`. Windows device names (`con`, `prn`, `aux`, `nul`, `com1`–`com9`, `lpt1`–`lpt9`) are reserved as whole segments, since they are made of legal characters and the allowlist cannot see them.

All user-declared names are lowercase kebab-case. Uppercase parses and is then rejected with a fixit rather than silently folded. Type parameters are the carve-out and stay uppercase: `list of T`, `map of K, V`.

The namespace tree corresponds to a directory tree; a declaration disagreeing with its location is an error unless the manifest declares that mapping. The manifest maps a namespace root to a directory root, longest prefix wins, and a dependency's namespaces come from its own manifest rather than from scanning its tree.

**Most programs need no imports at all.** The prelude supplies `print`, `int`, `float`, `bool`, `string`, `bytes`, and `none` as ordinary bindings, and every descriptor is a construct available without import. This is a complete program:

```terrane
namespace demo

function main
  value int8 = 120
  print; value
```

Importing `print` or `int8` is redundant. Explicit import remains available for rebinding, aliasing, and shadowing, and the examples below use it because they are specifically about importing.

### 11.1 Imports bind ordinary names; `as` renames

A `from ... import` binds each selected object under an ordinary name in the scope containing the import. There is no separate declare-then-bind step and no second spelling for the same object.

```terrane
namespace report-builder
from /image/codec import resize, encode
```

`as` renames a selection, which is how a colliding import is disambiguated and how a shorter or clearer local name is chosen:

```terrane
from /image/codec import resize as scale
from /core/types import int64 as word
```

The alias binds the exported object under the new name in the current scope, preserving the object's identity and visibility checks. Since imports now bind ordinary names, an import cannot shadow a name while leaving the original reachable under a second spelling; where both are wanted, alias one of them.

Import syntax is not declaration-modifier syntax. An alias can never create or replace a program-global:

```terrane
# rejected
from /core/output import print as global print
```

Global creation or replacement remains an explicit `global` declaration, visibly separate from import:

```terrane
namespace foo
from /core/output import print as core-print
global print = core-print
```

The bound name follows the same collision, duplicate-name, visibility, and scope rules as any other ordinary binding. Import syntax cannot smuggle `global`, `constant`, visibility, or any other declaration qualifier onto it.

## 12. Async, concurrency, and system profiles

```text
async callable -> task object
+-- await result through control-flow syntax
+-- cancellation/lifetime metadata

structured task scope                              v1 language-level object, not a library convenience
+-- child tasks; the scope joins them before completing
+-- cooperative cancellation with defined cancellation points
+-- deadline inheritance: a child may shorten but never extend its parent's
+-- failure observation for a child that throws while siblings run

profile library objects
+-- channel
+-- mutex
+-- read/write lock
+-- atomic by supported width
+-- thread-local facility
+-- shared collection variants
```

The async callable type, the task object, the structured scope, and cancellation with deadline propagation are all version one. Channels, locks, atomics, and executor selection are not: they are library objects over that core. Deadlines are explicit values that additionally propagate down scope boundaries — not ambient task-local state, because the boundary is written in the source. The language fixes the executor boundary but never hard-codes one executor.

These are ordinary objects supplied by selected packages/profiles, not universal prelude names. Capabilities gate allocator, threads, filesystem, sockets, process spawning, dynamic loading, reflection, unwinding, clocks, entropy, floating point, Unicode data, exact-big-integer storage, and atomic widths. Unavailable semantics are rejected; profiles never quietly change a type's behaviour.

## 13. Version-one data, operating-system, and I/O objects

These are proposed v1 standard-library objects. They require explicit imports and the relevant target capability; none is a universal prelude binding. The names below map the object relationships, while exact namespace paths and detailed semantics remain specification work.

**These facilities are written in Terrane, not as Rust support crates.** The Rust core stays deliberately minimal, and the boundary runs per layer rather than per facility: Rust owns the layer that is irreducible or externally audited, Terrane owns the object model, policy, diagnostics, and integration above it. So a JSON facility may have a Rust byte-level scanner beneath a Terrane document model and descriptor mapping, and a TLS facility uses an audited protocol implementation beneath Terrane stream integration, trust store, ALPN, and connector policy — TLS itself is never reimplemented.

A layer is Rust only when it is a syscall or ABI boundary, requires a guarantee the optimiser would destroy (constant-time comparison, memory ordering, zeroisation), is a large audited security-critical implementation, or is generated data rather than code. Everything else is Terrane, because a Rust support crate is permanently opaque to the compiler and forecloses optimisation of that facility forever. Core libraries reach Rust through the ordinary dependency mechanism in §14 — declaration plus an authored wrapper — so they receive no privileged path and double as worked examples of dependency use.

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
+-- invocation; string -> url                        parse and validate
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
+-- invocation; string|components -> path
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
+-- invocation; argument schema -> argument parser
+-- parse; process arguments|list of string -> parsed arguments
+-- usage/help rendering
+-- typed positional, option, flag, repeat, default, and remainder descriptors
+-- structured parse errors

exit-status
+-- invocation; int -> exit-status
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
+-- invocation; string -> ip-address                 parse IPv4 or IPv6
+-- checked; string -> ip-address|none
+-- version -> ipv4|ipv6
+-- string; -> string                             canonical presentation
+-- is-loopback / is-unspecified / is-multicast

socket-address
+-- invocation; ip-address, port -> socket-address
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
+-- invocation; string -> uuid
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

One principle governs every ecosystem below, and each entry is a specialisation of it:

> Dependency declarations name ecosystems and packages, not APIs. The build resolves the exact package and generates only the boundary machinery that Terrane source actually crosses. Tooling projects an advisory surface, which is never compiler-authoritative.

A declaration names `serde` or `numpy`; it does not describe what they contain. The resolved manifest, lock, features, target, and toolchain define the interface for a given build, because a predefined surface would be a second, weaker copy of the ecosystem's own type system and would drift with every release. Nothing is projected wholesale: boundary machinery exists for the specific calls and values a program crosses, so generated output stays proportional to use. Editor knowledge — Cargo metadata, rustdoc, runtime introspection — is advisory, never invents members, and never decides whether a program compiles. Tooling must not execute arbitrary package code to inspect it.

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
foreign runtime adapter                            boundary machinery, NOT a translation of the ecosystem
+-- runtime/module loading
+-- proxy type descriptors for crossings the program contains
+-- explicit scalar/collection conversion
+-- errors and traceback translation
+-- ownership/thread/lifetime rules
+-- reflection/debug/profiling metadata
+-- does NOT enumerate, mirror, or typecheck the foreign package's API
```

### 14.1 Rust crates and editor contracts

Rust is Terrane’s lowering language, so `use rust crate-name` adds a resolved Cargo dependency to the generated crate graph. The package selected by the manifest, Cargo resolution, features, target, and lock file is the native interface used by lowering at build time. Inline Rust and maintained Rust modules use that resolved Rust interface directly; Terrane does not predeclare, wrap, or project a high-level equivalent of the crate merely because it is a dependency.

The compiler’s reproducibility contract is the generated Cargo manifest and lock-resolved graph, not a checked-in catalogue of Rust APIs. Lowering emits deterministic Rust paths and calls against that graph, and Cargo/rustc type-checks the actual package versions and features selected for the build. A dependency change that alters an available Rust symbol is therefore a build-time interface change, diagnosed by the normal generated-Rust source mapping, rather than a stale compiler model.

Editor package knowledge is an optional, light-touch index over the same resolved graph, never an input that changes compilation. The language server obtains package/version/feature/target facts from Cargo metadata and lock data, then uses cached rustdoc JSON or Rust-analyzer for completion, signature help, hover, and documentation in inline or maintained Rust. It refreshes or invalidates that cache when the relevant manifest, lock, feature, target, or package source changes; it must not execute arbitrary package code merely to offer hints. Hints remain advisory: availability and correctness are settled by deterministic lowering and the build.

`reqwest` is the required v1 proving case. A Terrane package declares a locked `reqwest` dependency with `default-features = false` and explicit `blocking` and `rustls-tls` features, an explicitly chosen roots variant, and optional `json`; direct `reqwest::blocking` use in a native Rust body proves that the build-selected Rust interface flows through Cargo lowering without a Terrane wrapper. The language server may index that exact resolved package for Rust-native hints, but does not manufacture Terrane members or a request/result object model. The fixture uses a deterministic loopback server, compiles generated Rust with warnings denied, and runs it. Async `reqwest` awaits the general async model instead of imposing a one-off future abstraction.

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

Descriptors are semantic objects with canonical identity, not ordinary values. A statically resolved descriptor needs no runtime storage and lowers to nothing; reflection is the case that requires the canonical descriptor object to be materialised at runtime. "Not an ordinary value" is therefore a statement about storage and assignment, not a claim that a descriptor can never exist at run time — and a profile that strips reflection metadata removes the materialisation, not the identity.

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
