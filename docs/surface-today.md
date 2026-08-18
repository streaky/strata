# Terrane object surface — implemented today

This map describes the language surface implemented by the compiler today. It is not a map of every object proposed by the language draft.

Status labels:

- **implemented** — checked and lowered by the current compiler pipeline.
- **name only** — reserved in the compiler-owned namespace, but has no implemented value semantics or operations yet.
- **source-declared** — supplied by a Terrane program rather than the prelude.

## Tree

```text
Terrane package
├── compiler-owned namespaces
│   ├── /core
│   │   ├── /core/output
│   │   │   └── .print                         function
│   │   ├── /core/types
│   │   │   ├── .bool                         type descriptor
│   │   │   ├── .int                          type descriptor
│   │   │   ├── signed fixed-width descriptors
│   │   │   │   ├── .int8
│   │   │   │   ├── .int16
│   │   │   │   ├── .int32
│   │   │   │   ├── .int64
│   │   │   │   └── .int128
│   │   │   ├── unsigned fixed-width descriptors
│   │   │   │   ├── .uint8
│   │   │   │   ├── .uint16
│   │   │   │   ├── .uint32
│   │   │   │   ├── .uint64
│   │   │   │   └── .uint128
│   │   │   ├── floating-point descriptors
│   │   │   │   ├── .float                    spelling of .float64
│   │   │   │   ├── .float32                  canonical descriptor
│   │   │   │   └── .float64                  canonical descriptor
│   │   │   ├── .string                       type descriptor
│   │   │   ├── .none                         type descriptor
│   │   │   └── .bytes                        descriptor name only
│   │   └── /core/errors
│   │       ├── .error                         structural interface name only
│   │       ├── .arithmetic-overflow           error object name only
│   │       ├── .division-by-zero              error object name only
│   │       ├── .integer-conversion-overflow   error object name only
│   │       ├── .negative-shift-count          error object name only
│   │       ├── .resource-error                error object name only
│   │       └── .coercion-error                error object name only
│   └── /core/collections                      empty namespace; name only
├── default prelude
│   ├── print                                  binding to /core/output::print
│   ├── bool                                   type name for /core/types::bool
│   ├── int                                    type name for /core/types::int
│   ├── float                                  type spelling for /core/types::float64
│   ├── string                                 type name for /core/types::string
│   ├── bytes                                  type name for /core/types::bytes
│   └── none                                   type name for /core/types::none
└── source-declared package surface
    ├── namespace                              hierarchical object container
    │   ├── variable                           namespace-local value
    │   ├── constant                           namespace-local or program-global value
    │   ├── function                           callable value
    │   ├── nested namespace                   hierarchical name
    │   └── import                             selected names or namespace binding
    ├── function
    │   ├── parameter                          positional or named
    │   ├── optional parameter                 has a default expression
    │   └── return type                        declared scalar type
    └── lexical block
        └── binding                            local typed value
```

## Implemented value types

### `bool`

```text
bool value
├── property
│   └── .type -> .bool
├── unary operation
│   └── not bool -> bool
├── logical operations
│   ├── bool and bool -> bool
│   └── bool or bool -> bool
├── equality operations
│   ├── bool == bool -> bool
│   └── bool != bool -> bool
└── descriptor relation
    └── value is a .bool -> bool
```

`and` and `or` short-circuit. A descriptor comparison through `.type` uses canonical descriptor identity.

### `int`

`int` is an adaptive, exact signed integer. Its representation may widen, but that representation is not part of the Terrane object surface.

```text
int value
├── property
│   └── .type -> .int
├── unary operations
│   ├── -int -> int
│   └── ~int -> int
├── arithmetic
│   ├── int + int -> int
│   ├── int - int -> int
│   ├── int * int -> int
│   ├── int / int -> int      Euclidean quotient
│   └── int % int -> int      Euclidean remainder
├── bitwise and shift operations
│   ├── int & int -> int
│   ├── int | int -> int
│   ├── int ^ int -> int
│   ├── int << integer -> int
│   └── int >> integer -> int
├── comparisons
│   ├── int == int -> bool
│   ├── int != int -> bool
│   ├── int < int -> bool
│   ├── int <= int -> bool
│   ├── int > int -> bool
│   └── int >= int -> bool
├── coercion family
│   ├── .coerce; Destination -> Destination
│   └── .coerce.checked; Destination -> Destination or none
└── descriptor relation
    └── value is a .int -> bool
```

For an `int` source, the destination may be `int` or any fixed-width integer descriptor. `.coerce.wrap` and `.coerce.saturate` require a fixed-width source and therefore are not available from `int`.

### Fixed-width integers

The members below exist uniformly on:

```text
int8, int16, int32, int64, int128,
uint8, uint16, uint32, uint64, uint128
```

```text
fixed-width integer value T
├── property
│   └── .type -> descriptor T
├── unary operations
│   ├── -T -> T               signed types only
│   └── ~T -> T
├── arithmetic
│   ├── T + T -> T
│   ├── T - T -> T
│   ├── T * T -> T
│   ├── T / T -> T
│   └── T % T -> T
├── bitwise and shift operations
│   ├── T & T -> T
│   ├── T | T -> T
│   ├── T ^ T -> T
│   ├── T << integer -> T
│   └── T >> integer -> T
├── comparisons
│   ├── T == T -> bool
│   ├── T != T -> bool
│   ├── T < T -> bool
│   ├── T <= T -> bool
│   ├── T > T -> bool
│   └── T >= T -> bool
├── coercion family
│   ├── .coerce; Destination -> Destination
│   ├── .coerce.checked; Destination -> Destination or none
│   ├── .coerce.wrap; Destination -> Destination
│   └── .coerce.saturate; Destination -> Destination
└── descriptor relation
    └── value is a descriptor T -> bool
```

All integer descriptors, including `.int`, are valid destinations except that `.coerce.wrap` and `.coerce.saturate` do not accept `.int`. The family is compile-time only: a selection must be invoked in the same expression, so `family = value.coerce` is rejected, and the destination must resolve statically to a canonical descriptor. The flat `.checked-coerce`, `.wrapping-coerce`, and `.saturating-coerce` spellings are rejected with a migration diagnostic and no aliases remain. Default fixed-width arithmetic is checked. Overflow, division by zero, invalid shift counts, and failing exact coercions terminate with deterministic Terrane runtime failures.

Whole-number constant expressions may initialize a typed fixed-width binding directly when their mathematical value is in range; this includes signed minima such as `minimum int8 = -128`. This contextual treatment applies only to binding initializers. Whole-number literals passed to fixed-width parameters or returned through fixed-width contracts remain `int` and require explicit coercion.

### Floating-point values

Implemented types are `float32` and `float64`. `float` is the default-precision spelling of `float64` in this compiler version: both resolve to one canonical `.float64` descriptor and lower as binary64.

```text
floating-point value T
├── property
│   └── .type -> descriptor T
├── unary operation
│   └── -T -> T
├── arithmetic
│   ├── T + T -> T
│   ├── T - T -> T
│   ├── T * T -> T
│   ├── T / T -> T
│   └── T % T -> T
├── comparisons
│   ├── T == T -> bool
│   ├── T != T -> bool
│   ├── T < T -> bool
│   ├── T <= T -> bool
│   ├── T > T -> bool
│   └── T >= T -> bool
└── descriptor relation
    └── value is a descriptor T -> bool
```

No float conversion methods are implemented.

### `string`

```text
string value
├── properties
│   ├── .length -> int        Unicode extended grapheme-cluster count
│   └── .type -> .string
├── methods
│   ├── .concat; values... -> string
│   └── .join; values... -> string
├── iteration
│   └── for item in string    item is one owned grapheme string
├── comparisons
│   ├── string == string -> bool
│   ├── string != string -> bool
│   ├── string < string -> bool
│   ├── string <= string -> bool
│   ├── string > string -> bool
│   └── string >= string -> bool
└── descriptor relation
    └── value is a .string -> bool
```

`.concat` accepts zero or more values, converts each through Terrane's canonical scalar display, and appends them without a separator. `.join` accepts the same values but interleaves the receiver as the separator; an empty call yields the empty string and a singleton call adds no separator. The current `for` lowering is specifically string-grapheme iteration; there is no general iterable protocol yet.

### `none`

```text
none value
├── property
│   └── .type -> .none
└── descriptor relation
    └── value is a .none -> bool
```

`none` is also the absent arm of `.coerce.checked`. No other operations on `none` are implemented.

### `bytes`

`.bytes` is present in `/core/types` and the default prelude, but bytes literals, values, properties, methods, and operators are not implemented. It is therefore currently a reserved descriptor name rather than a usable implemented value type.

## Type descriptor objects

Every implemented scalar type has one canonical descriptor object:

```text
.bool
.int
.int8  .int16  .int32  .int64  .int128
.uint8 .uint16 .uint32 .uint64 .uint128
.float32 .float64 (`float` resolves to `.float64`)
.string
.none
```

Descriptor behavior:

```text
descriptor object D
├── identity
│   ├── D is D -> true
│   └── D is other-D -> false
└── use as a type
    ├── binding annotation
    ├── function parameter annotation
    ├── function return annotation
    ├── integer coercion destination
    └── right operand of `is a`
```

For a scalar value `value`:

```text
value.type is D
value is a D
```

both compare its resolved canonical Terrane type with `D`. Scalar values themselves are identity-less: `is` between ordinary scalar values is false even when their values and types are equal. Operand expressions are still evaluated for their effects.

Descriptor constructs are not runtime values and cannot be assigned to source bindings. An explicit import may bind a canonical descriptor under another name without creating a new descriptor.

## Functions

### Built-in `print`

Canonical object and default-prelude spellings:

```text
/core/output::print
print
```

```text
print; values... -> none
```

- Accepts zero or more arguments.
- Converts each argument with canonical scalar display.
- Concatenates converted values without separators.
- Writes one trailing newline.
- Canonical display is implemented for all usable scalar types and `none`.

### Source-declared functions

```terrane
function name ReturnType; required Type, optional Type = default
```

Implemented callable contract:

```text
source function
├── parameters
│   ├── required positional or named parameters
│   └── trailing optional parameters with default expressions
├── arguments
│   ├── positional
│   ├── named
│   └── omitted optional arguments filled from defaults
├── return
│   ├── declared scalar return type
│   └── bare return / fallthrough for no-value functions
└── call result
    └── participates in type checking and later member/operator resolution
```

The compiler checks duplicate, unknown, missing, and excess arguments, and rejects positional arguments after named arguments. Variadic source-declared functions, overloads, function values, closures, and generic functions are not implemented.

## Source object and name model

Terrane resolves every bare name through one ordered view:

```text
lexical scope -> namespace -> program-global -> default prelude
```

The first matching name may denote a value, function, canonical descriptor, namespace, or imported entity. There is no leading-dot object form: `.` appears only between a receiver and a member, as in `value.name`. Namespace qualification uses `namespace::name`.

Namespaces form a package-wide tree assembled before reference resolution. Paths use `/` between canonical lowercase segments, with root `/` and parent `..` anchoring. Authored manifests bound sorted recursive source discovery through namespace-root-to-directory mappings, and every discovered declaration is checked against its longest-prefix directory correspondence. Generated Cargo projects live under the package root, and `terrane-build.toml` records the resolved package-relative source set. Direct `.trn` input remains an exempt implicit one-unit package. Selected imports, namespace bindings, visibility, lexical shadowing, program globals, and explicit `global`/`constant` binding rules are implemented.

A top-level plain assignment creates a namespace variable. Functions cannot read or write namespace variables across that boundary; mutable state must cross as an explicit `global`, parameter, or return value. Namespace variables cannot be `public`.

`constant` declarations are non-rebindable at every supported identity tier. In one lexical scope, an ordinary assignment to an already initialized local creates a replacement binding; its initializer sees the earlier binding, and its inferred type may change. Assignment to an uninitialized local, an enclosing-scope binding, a parameter, or a `for` target remains mutation. Generated Rust marks only genuinely mutated storage mutable.

## Properties and methods index

| Receiver | Member | Kind | Result / effect |
|---|---|---|---|
| any implemented scalar value | `.type` | property | canonical scalar descriptor |
| `string` | `.length` | property | adaptive `int` grapheme count |
| `string` | `.concat; values...` | method | concatenated `string` using canonical display |
| `string` | `.join; values...` | method | canonical displays interleaved with receiver separator |
| any integer | `.coerce; D` | family default | exact coercion or runtime failure |
| any integer | `.coerce.checked; D` | family child | destination value or `none` |
| fixed-width integer | `.coerce.wrap; D` | family child | destination value with wrapping policy |
| fixed-width integer | `.coerce.saturate; D` | family child | destination value with saturation policy |

The compiler represents these spellings as one canonical callable family with a
distinguished default and typed policy children. Semantic analysis resolves that family
before lowering; generated Rust then erases the family object to the matching support
operation. The support-crate helper names are backend details, not additional Terrane
members or direct source-level coercion operations.

No other value properties or methods are recognized by the current semantic/lowering pipeline.

## Compiler-owned names without implemented object behavior

These names exist so the namespace and resolution model has stable canonical identities. They must not be mistaken for completed runtime behavior. The `.error` name is classified as a structural interface and the remaining `/core/errors` names as error objects, but fields, inheritance, constructors, catchability, and runtime instances remain unimplemented:

```text
/core/types::bytes
/core/errors::error
/core/errors::arithmetic-overflow
/core/errors::division-by-zero
/core/errors::integer-conversion-overflow
/core/errors::negative-shift-count
/core/errors::resource-error
/core/errors::coercion-error
/core/collections
```

In particular, runtime arithmetic diagnostics currently use deterministic compiler support paths; they do not construct catchable instances of the `/core/errors` names.

## Major planned surface absent today

The authoritative language draft proposes a much larger ontology. None of the following should be inferred from compiler-owned names or Rust support internals as implemented Terrane API:

```text
collections: .list, .map, .set, .tuple, .range, .entry
protocols and interfaces
classes, structs, enums, traits, and constructors
reflection beyond canonical scalar `.type`
catchable error values and error hierarchies
bytes values and operations
collection properties and methods
general iteration protocols
user-declared type parameters and generic application
function/class/namespace/type reflection objects
```

This separation is intentional: executable conformance defines the current compiler contract, while the full specification describes the planned language.