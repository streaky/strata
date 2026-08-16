# Terrane — Working Language Specification and Compiler Architecture

**Draft 0.1 — a human-facing object language lowered transparently to Rust**

> This document is the current integrated design source: normative language semantics, compiler/lowering contracts, and rationale share one file while the design is still changing quickly. Normative requirements are identified by the terms below; implementation sequencing lives separately in `compiler-plan.md`. A future publication may split these views without changing their contract. The constitutional invariants in §41 govern every section and take precedence over illustrative architecture or rationale.
>
> The project, language, and command-line interface have the working name **Terrane**; the CLI command is `terrane`.

---

## 1. Status and terminology

This is a **design specification**, not a claim that an implementation exists.

The words **must**, **must not**, **should**, and **may** are used in their usual specification sense:

- **must / must not** define the proposed language contract;
- **should** describes a strong implementation or ecosystem recommendation;
- **may** describes permitted behaviour or an optional capability.

Three representations are distinguished throughout:

1. **source** — the human-facing language described here;
2. **compiler model** — transient lexer, parser, AST, resolution, and analysis structures;
3. **generated Rust** — the canonical lowered representation passed to Cargo and `rustc`.

The compiler model exists because writing a parser without one would be needless theatre. It is not intended to become a second public intermediate language. For users, tooling, debugging, auditing, and performance work, **generated Rust is the authoritative lowered form**.

---

## 2. Executive summary

The language is designed around a deliberately small set of ideas:

- Everything is an object **semantically**.
- Not everything must be boxed or dynamically represented at runtime.
- Values are typed; bindings are dynamic unless explicitly constrained.
- Ordinary assignment has value semantics.
- Value assignment is copy-on-write by default; explicit `ref` shares mutable identity and `move` transfers ownership.
- The default global namespace is extremely small and clean.
- Engineers may define or replace their own global and namespace-local bindings, including facilities such as `print`; compile-time constructs such as `import` use separate structural extension slots.
- Imports populate an object-facing namespace without automatically polluting the ordinary variable/function namespace.
- Namespaces are tiered using whitespace; `/` anchors resolution at the root but is not a separator.
- Ordinary syntax favours unshifted characters and readable words over punctuation gymnastics.
- Control flow is conventional where conventional syntax is already good.
- The language lowers to readable, deterministic Rust, then uses the normal Rust toolchain.
- Native Terrane packages, Rust crates, system/C libraries, full and inline Rust, and explicit foreign-runtime adapters are first-class.
- Compilation is transparent during development and explicit at deployment boundaries.
- Reflection, source mapping, diagnostics, debugging, tracing, allocation analysis, and performance explanation are designed in from the beginning.
- A VM or JIT is not required. Fast incremental Rust compilation is the default development model.
- `no_std`, embedded, firmware, and kernel targets are possible when the program uses only capabilities available on those targets.

A representative program is:

```text
namespace my-app

from /core output import .print

print = .print

function main

  project-name = >Terrane
  build-target = >native executable
  build-status = >ready to build

  message = ': '.concat; project-name, build-target, build-status
  print; message
```

Conceptually:

1. `/` anchors the import at the root namespace.
2. `core output` is a tiered namespace path.
3. The import makes `.print` available as an object symbol.
4. `print = .print` creates an ordinary namespace binding to the imported print object.
5. `': '.concat` looks up the `concat` member on the `': '` text object.
6. Invoking that member joins the arguments using its receiver as the separator.
7. `print; message` invokes `print`’s default behaviour with `message` as its argument.

The output is:

```text
Terrane: native executable: ready to build
```

---

## 3. Goals

### 3.1 One human language over mature machinery

The language should justify its existence by **removing the need to care about several lower-level languages for ordinary work**, not by creating another isolated runtime and library island.

The intended stack is:

```text
human source
  -> parse, resolve, analyse
  -> readable generated Rust
  -> Cargo and rustc
  -> native binary, library, firmware image, wasm module, or kernel artefact
```

The language borrows Rust’s implementation ecosystem rather than rebuilding:

- native code generation;
- optimisation;
- ownership machinery;
- platform support;
- linking;
- C ABI integration;
- async and concurrency libraries;
- debuggers and native debug formats;
- package compilation;
- cross-compilation;
- `no_std`.

### 3.2 Progressive strictness

The default experience should get out of the engineer’s way:

```text
x = 42
```

When a contract matters, it can be added locally:

```text
x int = 42
```

When conversion is intended, it is explicit:

```text
x = x.coerce; float
```

Strictness should be additive and selectable at binding, member, function, class, namespace, package, and build-profile boundaries.

### 3.3 Clean names by default, real control when desired

The language should not begin by pouring hundreds of functions, variables, classes, helpers, and framework artefacts into global scope.

At the same time, the engineer should be able to define an actual project-global binding without fighting the language:

```text
global print = .print
global log = .logger
global database = .database;
```

If a runtime cannot tolerate a name being replaced, that facility should not masquerade as an ordinary replaceable binding.

### 3.4 Inspectable abstraction

The language should hide machinery when it is irrelevant and expose it unusually well when it matters.

A developer or coding agent should be able to ask:

- what Rust was generated for this function or class?
- what source expression caused this allocation?
- why was this value physically copied?
- was a value assignment satisfied through shared storage or a copy-on-write split?
- which generated Rust span caused this `rustc` diagnostic?
- what source-level object is represented by this native stack frame?
- what capability prevents this code compiling for `no_std`?

### 3.5 Pleasant ordinary typing

The common path should avoid braces, parentheses, colons, underscores, and shifted punctuation where they are not buying clarity.

This is an ergonomic target, not a religious prohibition. Shifted punctuation remains available where it is genuinely the cleanest answer.

---

## 4. Non-goals

The initial language is not intended to be:

- Rust with different punctuation;
- a compatibility implementation of Python, PHP, JavaScript, or another dynamic language;
- a new garbage-collected VM;
- a JIT research project;
- a macro language whose grammar can be rewritten by arbitrary packages;
- an attempt to expose every C++ ABI directly;
- a promise that every dynamic feature works without cost on every target;
- a promise that server processes dynamically recompile source in production;
- an excuse to hide generated code or compiler consequences;
- a second opaque IR layered between source and Rust;
- a language in which weak typing, implicit string/number coercion, and dynamic typing are treated as the same thing.

---

## 5. Design principles

### 5.1 Everything is an object semantically

Scalars, strings, functions, methods, classes, namespaces, importers, errors, collections, tasks, type descriptors, and reflection descriptors are all objects in the source-language model.

This does **not** require a universal heap allocation or a runtime vtable for every value.

```text
x = 42
```

creates an `int` object semantically. The compiler may realise it as:

```rust
let x: i64 = 42;
```

when no observable source behaviour requires boxing.

### 5.2 Values are typed; bindings may be dynamic

`42` is an `int`. It is not an “untyped scalar”.

```text
x = 42
```

means that `x` currently contains an `int` object. A later assignment may bind `x` to a different type:

```text
x = forty two
```

A type annotation constrains the binding:

```text
x int = 42
```

### 5.3 Dynamic does not mean weak

The language does not silently turn `'42'` into `42` merely because an operation would otherwise fail.

```text
x int = '42'
```

is a type error.

```text
x int = '42'.coerce; int
```

is an explicit conversion.

Equality is not permitted to smuggle in unrelated coercion rules:

```text
1 == '1'
```

is false, not true.

### 5.4 Easy by default; guarantees on demand

The absence of a qualifier normally means **minimal restriction**, not an invisible inferred restriction.

Examples:

```text
function render
```

is public and dynamically typed by default.

```text
private function render
```

narrows visibility.

```text
function add int; a int, b int
```

adds a type contract.

```text
strict types
```

may require contracts throughout a selected scope.

### 5.5 Dangerous behaviour should look deliberate

Ordinary assignment should not unexpectedly create shared mutable identity.

```text
b = a
```

means value assignment.

Shared identity is explicit:

```text
b = ref a
```

The implementation should satisfy ordinary value assignment through copy-on-write sharing until either logical value is modified.

A transfer of ownership for a linear value is explicit:

```text
b = move a
```

### 5.6 Rust is visible, not sacred

Generated Rust is a build artefact, debugging surface, performance receipt, and escape hatch.

It should be:

- readable;
- stable enough to diff;
- deterministic for the same source/compiler/profile;
- source-mapped;
- accessible through tooling;
- optionally accessible through runtime reflection;
- never the only place a source-language error is reported.

Generated Rust should normally not be edited in place. A module may instead be deliberately “ejected” into a maintained native Rust source file.

---

## 6. Lexical structure

### 6.1 Encoding

Source files are UTF-8.

The version-one compiler restricts identifier characters to ASCII letters and digits while the grammar stabilises. A later language version may admit Unicode identifier characters deliberately; non-ASCII characters are not silently normalized or accepted by version one.

### 6.2 Indentation and blocks

Blocks are indentation-delimited.

```text
class widget

  function render
    print; 'rendered'
```

A block begins when the next logical line is more deeply indented and ends on dedent.

Tabs and spaces are both valid indentation styles, but a source file must use exactly one of them for leading block indentation. The first indented logical code line selects the file's style. After that:

- a tabs-style file uses one tab per indentation level and rejects spaces in every leading indentation prefix;
- a spaces-style file uses only spaces in leading indentation prefixes and rejects tabs there;
- blank and comment-only lines do not select or alter the style.

Any mixed leading indentation, whether within one prefix or across different code lines in the same file, is a compile-time error at the offending whitespace. The lexer must not silently convert or repair indentation. Tabs remain valid as string content through escapes such as `\t`.

The formatter emits two spaces per level by default, although it may preserve or be configured to emit a consistently tab-indented file. For spaces-style source, indentation width is not fixed semantically; indentation depth is determined by increases and returns to previously established indentation columns.

### 6.3 Empty blocks

Empty declarations are legal.

```text
function not-yet

class placeholder
```

No `pass`, empty statement, or dummy expression is required.

A declaration with no following deeper-indented line has an empty body.

### 6.4 Comments

Terrane supports both shell-style and C-style comments:

```text
# comment

// comment

/*
comment
*/

/***
 * also a legal comment
 */
```

`#` and `//` begin line comments outside strings, raw blocks, foreign-source blocks, and block comments. They consume through the end of the physical line.

`/*` begins a block comment and the next `*/` ends it. Block comments may span lines. They do not nest: another `/*` inside one is comment text, and the first following `*/` closes the comment. An unterminated block comment is a compile-time error reported at its opening delimiter.

All `/* ... */` forms are ordinary comments, including forms beginning with `/**`, `/***`, or lines conventionally prefixed by `*`. A documentation-comment convention may later assign meaning to one of those forms, but it must remain lexically valid as a comment regardless.

Comment contents do not participate in indentation. Comment-only lines are ignored when producing indentation tokens, and a multiline comment must not create or close a block. Outside comments, `//` and `/*` are recognised only as those exact two-character delimiters, so `/` remains available for root-anchored namespace paths.

Python-style triple-string “comments” are deliberately not supported. A string is an expression, never a comment, and unused strings must not acquire comment semantics. An embedded foreign-source block retains the foreign language’s own lexical rules; Terrane does not reinterpret Python contents.

### 6.5 Identifiers

An identifier begins with a letter and ends with a letter or digit. Between those ends it may contain:

- letters and digits;
- runs of the identifier-joiner glyphs `+`, `-`, `*`, `/`, `%`, `<`, and `>`.

Identifiers may end in digits: `http2`, `sha256`, and `vector4` are valid. The restriction applies only when a terminal digits-only unit is introduced by an identifier joiner. Compact forms such as `count-1`, `page/2`, and `x+4` are lexical errors rather than identifiers or arithmetic. Names such as `http2-client`, `ipv4/ipv6`, and `sha3-256sum` remain valid because each unit after a joiner contains a letter.

A compact letter-to-letter joiner sequence is always an identifier, permanently: `total-count`, `page-size`, and `width-height` never mean subtraction without surrounding operator whitespace, even if a same-spelled binding exists. Arithmetic must be written `total - count`. This asymmetry is intentional: kebab-case names require a stable lexical interpretation, while a terminal joiner-plus-digits form is reserved as an error because it is not needed for that naming convention.

Examples:

```text
print
my-class
http2-client
foo+bar
ipv4/ipv6
input>output
```

The rule is lexical and universal for those glyphs: a maximal joiner run directly surrounded on both sides by identifier characters belongs to the identifier only when the following identifier unit contains a letter. A symbolic run cannot begin an identifier. When it begins a token after whitespace, a delimiter, or the start of a line and is immediately followed by an identifier character, it has behavioural/operator meaning rather than becoming part of the following name.

```text
a+b      # one identifier token
a + b    # detached operator expression
a +b     # the same operator, right-attached to its operand
a+ b     # postfix/left-attached form; an error unless `+` declares that behaviour
count-1  # lexical error: attached joiner followed by a digits-only suffix
-einval  # prefix negation, never an identifier named `-einval`
```

Consequently `x=foo+bar` binds `x` to the exact identifier `foo+bar`, while `x=count-1` is rejected with a diagnostic suggesting `x = count - 1`. `=` and other structural delimiters are not identifier joiners, so assignment remains recognisable without surrounding spaces. Nevertheless, canonical Terrane style requires whitespace around these delimiters: compact forms such as `x=foo+bar` visually obscure the boundary between assignment syntax and operator-bearing identifiers. Formatters insert the spaces, and linters should warn when they are omitted. The warning targets the compact structural delimiter, not the operator-bearing identifier; `result = foo+bar` remains ordinary canonical source. Ordinary numeric suffixes remain valid when no joiner introduces them, as in `sha256`.

Prefix, right-attached, and postfix forms are grammar-specific. `-1` and `-einval` apply declared prefix negation; `a +b` is the same infix addition as `a + b` because the preceding whitespace starts an operator token; and `i++` retains its declared postfix meaning. A left-attached form such as `a+ b` is reserved for declared postfix behaviour and is otherwise an error. `foo++bar`, by contrast, is an identifier because its post-joiner unit contains letters. Comment openers take lexical priority, so `//` and `/*` begin comments rather than forming identifier content.

Comparison tokens containing `=`, such as `==`, `!=`, `<=`, and `>=`, cannot occur inside identifiers because `=` is structural. They may be detached or right-attached (`a == b` or `a ==b`); a left-attached spelling is invalid unless that token acquires an explicit postfix meaning. Future symbolic operators must explicitly declare whether each glyph is an identifier joiner and which prefix, infix, or postfix behaviours it supports; adding an operator must not silently change how existing source tokenises.

This design deliberately makes whitespace and attachment two of the language's small number of semantic signals. Rejecting a terminal joiner-plus-number suffix prevents a likely misspelling from silently changing between a name and arithmetic. The diagnostic must identify the attached suffix and offer the corresponding spaced expression as a fix.

### 6.6 Contextual words

Most structural words are grammar tokens in their structural positions:

```text
namespace
from
class
function
if
else
for
while
try
catch
finally
throw
return
break
continue
public
private
protected
global
ref
move
rust
unsafe
```

`import` is special: it participates structurally in both `from ... import ...` and `import with ...`. The latter selects a compile-time importer slot; neither form resolves an ordinary binding named `import`.

The language should use contextual rather than gratuitously reserved keywords where doing so remains unambiguous.

### 6.7 Text literals

Quoted strings use single quotes by default:

```text
name = 'alice'
separator = ' '
empty = ''
exact = '  \tmany like it'
```

At minimum, the following escapes are supported:

```text
\\
\'
\n
\r
\t
```

An attached `>` in an expression-start position begins a **tail string**. Every source character after the marker through the physical end of that line is literal content; the line terminator is excluded:

```text
project-kind = >native executable
message = >Hello! From, "Terrane"! >>
send; recipient, >Error: file not found!
```

The second value is exactly `Hello! From, "Terrane"! >>`. Quotes, commas, operators, comment markers, and further `>` characters have no grammatical meaning after the opening marker. Whitespace is preserved exactly, including whitespace immediately after `>` and trailing horizontal whitespace. An attached `>` with no following content is the empty string.

The marker must begin an expression and must be lexically attached to the expression position; its content begins with the very next character, which may be whitespace. This keeps it distinct from infix comparison:

```text
is-larger = left > right
message = >left > right
```

A tail string consumes the remainder of its line, so it is necessarily the final syntactic element on that line. It may nevertheless be the final argument of a call, as in `send; recipient, >Error: file not found!`. Use a quoted string when member access, another argument, an operator, or any other syntax must follow the literal.

An exact `>>` in an expression-start position opens a **block string** whose content is the following indented block:

```text
message = >>
  Hello! From, "Terrane"!

  Everything in this block is text.
  # This is content, not a comment.
```

If `>>` is followed by any same-line content, including horizontal whitespace, the construct is invalid; it is not reinterpreted as a tail string beginning with `>`.

The first nonblank line selects the block's structural indentation prefix. That exact prefix is removed from each nonblank content line; any indentation beyond it is preserved as content. Blank lines are preserved and do not end the block. The first nonblank line lacking that prefix ends the block and is parsed normally. This follows the source file's selected tab-or-space indentation style without expanding tabs or normalising content whitespace.

Lines are joined with `\n`. Source layout does not add a final newline to the value. An empty block is invalid rather than silently producing an empty string; use `>` or `''` for that value.

Both tail and block strings are literal and non-interpolating. Once either form begins, comments, escapes, substitutions, and ordinary Terrane tokens are not recognised within its content. Interpolation, if added, requires a separate explicit form.

A bare identifier always performs binding lookup:

```text
x = hello
```

To create text, use one of the three explicit forms:

```text
inline = 'hello'
tail = >Hello, from Terrane!
multiline = >>
  Hello,
  from Terrane!
```

### 6.8 Numeric literals

A numeric literal is a run of decimal digits with an optional single `.` fraction, or a `0x` hexadecimal run:

```text
count = 42
ratio = 3.14
mask = 0xff
population = 1_000_000
```

`_` may separate digits within a run. It may not begin or end a run, appear twice consecutively, or stand beside the fraction point. A hexadecimal literal requires at least one hex digit after its prefix; `0X` is the same form.

Version one defines no exponent, no radix prefix other than `0x`, and no type suffix. A digit run followed immediately by identifier characters is one malformed literal rather than a literal beside a name, so `1e9`, `0b101`, and `123abc` are lexical errors reported across the whole run. Write the intended value explicitly instead.

A `.` is part of a literal only when a digit follows it. Otherwise it remains ordinary punctuation, so `1.type` is a member expression on a literal and `..` retains its namespace meaning.

### 6.9 Newlines and continuation

A newline normally terminates a statement.

A logical statement may continue after:

- a comma;
- an operator;
- an explicit call marker `;`;
- a deeper indentation that is syntactically attached to the preceding expression.

The formatter should prefer one statement per line and use indented continuation rather than backslash escapes.

### 6.10 Punctuation roles

The core punctuation has rigid jobs:

| Form | Meaning |
|---|---|
| `.name` | object-form lookup |
| `value.member` | member lookup |
| `;` | begin an invocation’s argument list |
| `,` | separate arguments or values |
| `|` between types | construct a union type |
| `=` | bind or assign a value |
| `/` before a namespace path | anchor lookup at the root namespace |
| `..` before a namespace path | ascend one namespace tier |
| `'...'` | delimited quoted string |
| `>text` | exact text through the physical end of line |
| `>>` followed by an indented block | exact multiline block text |
| `#` or `//` | begin a line comment outside text literals |
| `/* ... */` | block comment, possibly multiline |

Whitespace before a dot separates expressions; it is not a call form:

```text
print.concat   # member lookup
print .concat  # invalid adjacency
print; .concat # pass the dot-object explicitly
```

The formatter must preserve member attachment and must never turn invalid adjacency into invocation.

---

## 7. Namespaces and name resolution

### 7.1 Tiered namespaces

Namespace components are separated by whitespace in source.

```text
namespace my-output formatters
```

declares the tier:

```text
root
  my-output
    formatters
```

`my-output` is one component because its hyphen is internal. `formatters` is its child because it is separated by whitespace.

The namespace hierarchy is logical and is not required to mirror the filesystem.

A file may contribute declarations to an existing namespace. Multiple files may contribute to the same namespace unless a package policy forbids it.

Package metadata enumerates every source unit belonging to the package. The compiler parses that complete set before resolving namespace declarations; there is no filename-to-namespace convention and no on-demand search by namespace name. Incremental builds may avoid reparsing unchanged units from validated summaries, but adding or removing a source unit changes the package input and invalidates namespace assembly.

### 7.2 Root anchoring

`/` anchors namespace resolution at the root:

```text
from /image codec import .jpeg
```

The `/` is not a separator. It means “start at root”; `image codec` remains a whitespace-tiered path.

### 7.3 Relative anchoring

An unanchored path begins at the current namespace:

```text
from helpers formatters import .pretty
```

`..` ascends one tier before resolving child components:

```text
from .. shared import .config
```

Repeated parents are explicit:

```text
from .. .. platform import .clock
```

For a current namespace of:

```text
namespace my-app http handlers
```

the paths resolve as follows:

| Source path | Result |
|---|---|
| `helpers` | `my-app http handlers helpers` |
| `.. shared` | `my-app http shared` |
| `.. .. platform` | `my-app platform` |
| `/core output` | `root core output` |

Resolution never silently falls back from a failed relative path to the root. Ambiguous convenience is not worth non-local behaviour.

Whitespace—not `/`—separates namespace components. A leading standalone `/` anchors a path at the root; an internal `/` may belong to one component under the identifier rule. Therefore `from /network ip import .address` has components `network` and `ip`, while `from network ipv4/ipv6 import .address` contains the single component `ipv4/ipv6`. Likewise, `use ip/v6/ipv4` names one package rather than three nested packages. Filesystem layout is package metadata, not source path punctuation.

### 7.4 Namespaces as objects

A namespace has an object representation available to reflection and tooling. It can report:

- its parent;
- children;
- declarations;
- visibility;
- package ownership;
- source units;
- exported dot-objects;
- ordinary bindings.

The `/` and `..` anchors remain grammar, not replaceable runtime objects.

### 7.5 Two lookup views, one runtime ontology

The language distinguishes two syntactic lookup views:

1. **ordinary bindings**, written as plain names;
2. **object-form symbols**, written with a leading dot.

Both resolve to objects. They are not different runtime species.

This distinction exists to prevent imports from polluting the ordinary function/variable namespace.

```text
from /core output import .print
```

makes `.print` available in the current object-form scope. It does not automatically bind the plain name `print`.

The engineer chooses the ordinary binding:

```text
print = .print
```

or a program-global one:

```text
global print = .print
```

This separation is central to the language.

Object-form symbols have their own lexical scope chain. A `from ... import` populates the object-form scope containing that import: block imports last to the end of the block, function imports last to the end of the function, and namespace-top-level imports populate that exact namespace. Lookup proceeds from the current object-form lexical scope through enclosing lexical scopes, the current namespace, and parent namespaces nearest first; it does not consult program-global ordinary bindings or the prelude's ordinary view. Namespace-level object imports are inherited by descendant namespaces under the same visibility rules as ordinary namespace bindings.

A nearer object-form symbol shadows a farther one. Introducing two different symbols under the same object-form name in one scope is a compile-time collision; source order never chooses a winner. Reimporting the same export is idempotent. Aliasing is required when both colliding objects must remain available. Declaration-modifier lookup uses this same object-form chain.

### 7.6 Declarations and object-form symbols

A declaration creates:

- an ordinary binding in its defining namespace;
- an object-form symbol for direct import and object lookup.

For example:

```text
namespace text formatters

class concat
```

defines both the namespace-local binding `concat` and the importable object symbol `.concat`.

Importing `.concat` elsewhere does not create a plain `concat` binding there.

### 7.7 Ordinary name resolution

Plain names resolve in this order:

1. current lexical scope;
2. current function/method implicit bindings;
3. current class/object scope where applicable;
4. current namespace;
5. parent namespaces, nearest first;
6. program-global bindings;
7. the selected prelude.

A nearer binding shadows a farther binding.

Shadowing is legal. Linters may report it according to project policy.

### 7.8 Namespace-local bindings

At namespace top level, ordinary assignment binds in that namespace:

```text
namespace my-output formatters

from /core output import .print
print = .print
```

The binding is inherited by descendant namespace resolution unless hidden by a nearer binding.

### 7.9 Program-global bindings

`global` binds at the program assembly root:

```text
global print = .print
global log = .logger
global database = .database;
```

Program globals are ordinary bindings. They are not a privileged language-owned namespace.

`global` is required whenever source creates or replaces a program-global binding, including in a source unit assigned to the root namespace. A plain top-level assignment always remains namespace-local. Requiring the marker prevents moving a file or changing its namespace declaration from silently changing the reach of its bindings.

A global declaration still retains its lexical declaring namespace for visibility and name resolution. `global` controls program-wide identity and lifetime; it does not erase declaration provenance or imply public visibility. Therefore `private global max-threads int = 0` denotes one program-wide binding whose source-visible name is resolvable only inside its exact declaring namespace.

A package may not silently mutate the consuming program’s global bindings merely by being installed. Global composition belongs to the program entry configuration or explicitly evaluated program source.

### 7.10 The core prelude

The default prelude is a deliberately small set of ordinary program-global bindings selected from the `/core` implementation. `/core` is an ordinary, explicitly addressable root package namespace, so its objects remain directly importable:

```text
from /core output import .print
print = .print
```

That creates a namespace-local `print` even though the default prelude already supplies the same core object globally. The explicit form is useful in a project that disables the prelude, under an alias, or when declaring exactly which implementation a namespace uses.

The version-one default ordinary bindings are:

- `print`, sourced from `/core output`’s `.print`;
- scalar type objects `int`, `float`, `bool`, `string`, `bytes`, and `none`, sourced from `/core types`.

This is the complete default list. In particular, collections, filesystem access, concurrency, formatting helpers, and reflection helpers require imports. `import` remains structural syntax whose behaviour is supplied by the active importer object; it is not an ordinary prelude binding.

Prelude bindings are defaults, not reserved names. Explicit program composition may replace any of them:

```text
from mylib tools import .myprint
global print = .myprint
```

After this declaration, ordinary lookup of `print` through the program-global tier resolves to `mylib tools`’ `.myprint`. The original remains available by explicitly importing `/core output`’s `.print`. A prelude replacement does not mutate `/core`, the imported object-form scopes, or namespace-local bindings that shadow the global.

A project may replace, extend, or disable the selected prelude through its build manifest. Packages cannot do so merely by being installed or imported; program-global composition remains an entry-project decision.

Documentation fragments may omit imports when the import itself is not under discussion. Such omissions are editorial only: the fragment's fixture supplies explicit object-form imports. In this document `.list`, `.map`, `.set`, `.tuple`, `.range`, and `.entry` come from `/core collections`; `.file` comes from `/system files`; `.shared-map` comes from `/concurrency`; fixed-width numeric descriptors `.int8`, `.int16`, `.int32`, `.int64`, `.int128`, `.uint8`, `.uint16`, `.uint32`, `.uint64`, `.uint128`, `.float32`, and `.float64` come from `/core types`; and example-only objects such as `.device-handle` come from the named example fixture. A complete source unit must write those imports. None of these objects belongs to the default prelude.

---

## 8. Imports

### 8.1 Basic import form

```text
from /image codec import .jpeg
```

imports an object-form symbol from a namespace.

Multiple objects may be imported:

```text
from /image codec import .jpeg, .png, .webp
```

Object-form aliasing is allowed when collisions must be retained:

```text
from /core output import .print as .core-print
from /pretty output import .print as .pretty-print
```

The aliases remain dot-object symbols. Ordinary names are still bound explicitly:

```text
print = .pretty-print
fallback-print = .core-print
```

### 8.2 Import is a compile-time construct slot

The parser recognises the structural form:

```text
from path import objects
```

Its behaviour is supplied by the importer selected for the current compile-time construct scope. The standard importer is a precompiled `/core` host extension implementing the versioned compiler importer protocol; it is not an ordinary prelude object or runtime binding.

A namespace may select another importer for subsequent imports in that namespace and its descendants:

```text
namespace plugins

from /build importers import .sandboxed-import
import with .sandboxed-import
```

A program entry source may select one at the program-global construct tier:

```text
from /build importers import .content-addressed-import
global import with .content-addressed-import
```

`import with` and `global import with` are structural compile-time selection statements, not assignments. Their right operand must resolve through the object-form scope to a declared, precompiled host extension implementing the importer protocol. Namespace selection applies after the statement to that namespace and descendants unless a nearer selection replaces it. Global selection applies after the statement wherever no nearer namespace selection exists. Lexical blocks and functions cannot replace the importer because their imports are resolved before runtime scope exists.

If a replacement importer breaks importing, importing is broken. This is an intentional consequence of giving the entry project control over a fundamental compiler extension slot. An ordinary binding named `import` is legal but has no effect on import syntax.

### 8.3 The importer protocol

An importer receives at least:

- requesting namespace;
- requested path and anchor;
- requested object names and aliases;
- package/build profile;
- target triple;
- active dependency lock;
- permitted build-time capabilities;
- source location.

It returns an import plan containing at least:

- resolved namespace/package identity;
- object exports;
- dependency additions;
- generated source or Rust units, if any;
- reproducibility metadata;
- diagnostics;
- source-map provenance.

The exact protocol should be versioned independently of the source grammar.

### 8.4 Importer bootstrapping

The compiler has a minimal bootstrap importer capable of loading:

- the selected prelude;
- the root program source;
- the package manifest;
- a declared custom importer.

After a custom importer is installed, normal imports may be delegated to it.

For deterministic behaviour, imports and `import with` selections inside a compilation unit are processed in source order. A manifest-level importer applies before source selections and imports.

A recovery option should allow a build to force the bootstrap importer when a custom importer prevents the project from compiling.

### 8.5 Import security

An importer may perform extraordinary work, including generated modules, content-addressed resolution, policy checks, or remote retrieval. That makes it build-time executable code.

The package/build system must expose importer capabilities explicitly, including:

- filesystem read/write;
- network access;
- process execution;
- environment access;
- credential access.

Reproducible builds should reject undeclared or unrecorded importer inputs.

---

## 9. Objects, members, and invocation

### 9.1 Object-form lookup

A leading dot resolves an object symbol:

```text
.print
.concat
.jpeg
.database
```

The result is an object value: perhaps a function object, class object, singleton, prototype, namespace adapter, importer, or another callable object.

A dot lookup alone does not imply invocation:

```text
print = .print
```

binds the object.

### 9.2 Default invocation

A semicolon invokes an object’s default behaviour:

```text
print; message
```

For a function object, the default behaviour executes the function.

For a class object, the default behaviour constructs an instance.

For an importer, it resolves an import request.

For an ordinary object, the class may define whatever default invocation means.

A zero-argument invocation is explicit:

```text
thing = .thing;
```

### 9.3 Member lookup and member invocation

No whitespace before the dot means member access:

```text
print.concat
```

The result is the `concat` member object.

Invoking it is ordinary default invocation:

```text
print.concat; a, b, c
```

A zero-argument method invocation remains explicit:

```text
buffer.clear;
```

### 9.4 Dot-object arguments

A dot-object is an ordinary argument value and never invokes the expression to its left through adjacency. Calls always retain the explicit semicolon:

```text
print; (.render; report)
```

This invokes `.render` with `report`, then passes its result to `print`. It differs from:

```text
print.render; report
```

which invokes the `render` member of the print object. The invalid spelling `print .render; report` receives a diagnostic suggesting one of those two forms; whitespace is not general function application.

An uninvoked dot-object can be passed without grouping:

```text
configure; .render
```

### 9.5 Positional and named arguments

An invocation has one argument list:

```text
callable; arguments
```

Arguments may be positional or named. Positional arguments must precede named arguments:

```text
request; url, timeout=5, retries=2
```

Named arguments bind by parameter name rather than position. A call must not bind the same parameter both positionally and by name.

Because `-` has no call-specific role, subtraction remains an ordinary expression in an argument list:

```text
print; a - b
```

Parentheses group an expression; they never create an alternative call syntax. Because invocation is introduced by `;`, these are equivalent:

```text
if is-enabled; config-vmap-stack
  ...

if (is-enabled; config-vmap-stack)
  ...
```

The first is canonical when the call is the whole condition. Parentheses are useful only when they delimit a call inside a larger expression:

```text
if (flags & mask) != 0
  ...

result = (convert; uint64, pages) * page-size
```

`if (is-enabled; ...)` is therefore supported grouping, not C-style invocation. The formatter removes redundant whole-condition parentheses and preserves parentheses that determine expression structure.

A call clause extends to the end of its containing logical expression. Commas delimit its top-level arguments, but a semicolon inside an ungrouped argument does not start a nested call: `print; format; value` is invalid. A call used as an operand or argument inside a larger expression must be parenthesised, as in `print; (format; value)` or `result = (convert; uint64, pages) * page-size`.

The semicolons in a three-clause `for` belong to the `for` grammar and delimit its clauses. Any call inside one of those clauses must therefore be parenthesised: `for i = (start-at; limit); i < limit; i++`. These rules make every semicolon's owner syntactically determinate without a closing-call token.

### 9.6 Object protocols

The core object model defines protocols rather than a proliferation of special runtime species.

At minimum, the language requires protocols for:

- default invocation;
- construction;
- member lookup;
- type identity and compatibility;
- coercion;
- value assignment and copy-on-write separation;
- equality;
- ordering where supported;
- hashing where supported;
- truth evaluation;
- iteration;
- reflection;
- destruction/drop.

The core text-display protocol produces a `string` for human-facing output. Version one implements it for `string`, `int`, every fixed-width integer, `float`, `float32`, `float64`, `bool`, and `none`. Strings are returned unchanged; integers use base-ten digits with a leading `-` only when negative and no grouping; floating-point values use the shortest round-trippable decimal spelling while preserving negative zero and spelling non-finite values `inf`, `-inf`, and `nan`; booleans and absence render as `true`, `false`, and `none`. `bytes` deliberately does not implement text display because arbitrary bytes are not Unicode text.

The core `print` object accepts values implementing text display, invokes that protocol left to right, writes the resulting text, and terminates the record with a newline. A value without the protocol is a source type error when known statically and a typed runtime error otherwise. Formatting policy beyond this canonical scalar display remains in explicitly imported formatting facilities; `print` does not obtain locale, width, precision, or arbitrary object formatting implicitly.

Version one admits a dynamic binding only when its alternatives form a finite compiler-known set. Protocol availability and typed-boundary compatibility are therefore checked across every alternative statically. If any possible alternative lacks text display, passing that binding to `print` is a source type error; the first-version compiler does not defer that case to the runtime. The typed runtime-error rule above applies to later or foreign erased dynamic values whose complete alternatives are unavailable at compilation.

A particular object need not implement every protocol.

### 9.7 Classes

A class declaration creates a class object:

```text
class widget

  width int = 0
  height int = 0

  function construct; width int, height int
    this.width = width
    this.height = height

  function area int
    return this.width * this.height
```

The implicit binding `this` refers to the current instance. It is not written as an explicit first parameter.

The class object’s default invocation constructs:

```text
widget = .widget; 100, 50
```

### 9.8 Functions and methods are objects

A function declaration creates a callable object:

```text
function greet; name string
  message = ' '.concat; 'hello', name
  print; message
```

It may be passed, stored, value-assigned, reflected, or invoked through its default behaviour:

```text
handler = greet
handler; 'alice'
```

A selected method is also an object:

```text
handler = server.handle
handler; request
```

### 9.9 Static/class behaviour

Functions declared inside a class are instance methods by default.

A `static` qualifier declares a function on the class object rather than on instances:

```text
class widget

  static function from-config widget; config
```

Static state is state on the class object and follows the same visibility and concurrency rules as other globals/shared objects.

### 9.10 Construction and destruction

`construct` is the conventional constructor method used by a class object’s default invocation.

`drop` is the conventional deterministic destruction hook:

```text
class file-wrapper

  function drop
    this.file.close;
```

The compiler must guarantee deterministic destruction at scope exit or when the final owning reference is released, subject to explicit reference-cycle rules.

User code should not normally call `drop` directly. An explicit core operation may exist for early release when required.

---

## 10. Visibility

### 10.1 Default visibility

Declarations and members are public by default.

```text
class widget

  function render
```

The language gets out of the way where visibility does not matter.

Explicit visibility remains available and meaningful:

```text
public function render
private cache = .map;
protected function update-layout
```

Writing `public` is permitted as documentation even though it matches the default.

### 10.2 Class visibility

Inside a class:

- `public` is visible to all permitted callers;
- `protected` is visible to the class and descendants;
- `private` is visible only to the declaring class.

### 10.3 Namespace visibility

At namespace scope:

- `public` is importable from other namespaces/packages;
- `protected` is visible to the namespace and descendant namespaces;
- `private` is visible only inside the exact namespace.

### 10.4 Strict visibility mode

A project or namespace may enable a strict visibility policy requiring explicit qualifiers for selected API boundaries.

This is a lint/contract mode, not the default language experience.

### 10.5 Package-supplied declaration modifiers

The fixed declaration grammar cannot grow a keyword for every ecosystem's storage, linkage, ABI, section, calling-convention, or code-generation requirement. An imported object may implement the constrained declaration-modifier protocol and appear in object form before a declaration:

```text
from /linux kernel import .per-cpu, .cacheline-aligned, .weak, .syscall

.per-cpu global process-counts unsigned-long = 0
.cacheline-aligned global tasklist-lock rwlock = .rwlock;
.weak function arch-release-task-struct void; tsk ref task-struct
.syscall function unshare long; unshare-flags unsigned-long
```

A leading object-form symbol is structurally a modifier; bare identifiers are never inferred to be modifiers. Modifier lookup uses the object-form scope rules in §7.5. A declaration modifier receives the declaration's typed semantic descriptor during compilation and may return a constrained transformation or attach metadata consumed by lowering. In this example `.per-cpu` is supplied by the imported package; using it without importing that object is an unresolved-object error.

The protocol may affect only declared compiler extension points, including storage placement, linkage, exported symbol names, ABI/calling convention, alignment, target sections, generated wrappers, and checked declaration constraints. It must not replace a declaration body with hidden runtime behaviour, weaken source-visible ownership or effects, capture undeclared inputs, perform unrestricted syntax rewriting, or evade safety, capability, visibility, or type checks.

Modifier resolution, order, provenance, effects, and emitted native attributes are recorded in reflection and build metadata. Versions and consulted build inputs participate in cache keys. Unsupported or conflicting modifiers are compile-time errors.

Core declaration words determine declaration shape. A run of leading object names before a core declaration is a left-to-right zero-argument modifier list. Bare typed bindings do not admit prefix modifiers because they have no structural declaration introducer; metadata for one must use an explicit descriptor operation after declaration. A modifier requiring arguments likewise uses such an explicit compile-time descriptor operation. Failed calls and modifiers are never reinterpreted as one another.

---

## 11. Values, scalar objects, and types

### 11.1 Core scalar objects

At minimum, the language defines:

| Type | Proposed semantics |
|---|---|
| `int` | arbitrary-precision signed integer with transparent representation promotion and normalisation |
| `float` | IEEE 754 binary64 |
| `bool` | `true` or `false` |
| `string` | Unicode text, stored as UTF-8 by the standard implementation |
| `bytes` | arbitrary binary data |
| `none` | the single absence value |

The explicit fixed-width numeric types are:

```text
int8 int16 int32 int64 int128
uint8 uint16 uint32 uint64 uint128
float32 float64
```

These descriptor objects are exported from `/core types` under their corresponding dot-object names. The default prelude binds only `int`, `float`, `bool`, `string`, `bytes`, and `none`; a program using a fixed-width type must import and bind it explicitly:

```text
from /core types import .int64
int64 = .int64

count int64 = 42
```

An imported descriptor can be rebound under another ordinary name without changing the represented type. The fixed-width spellings are therefore standard object names, not reserved type keywords and not hidden compiler-only names.

`int` is one source type, not an alias for `int64` and not a union of source-visible width types. Its values have no language-level minimum or maximum. Ordinary `int` arithmetic produces the exact mathematical result; crossing a representation boundary is internal runtime control flow, not a throw, panic, type change, or observable conversion.

The standard runtime represents `int` values adaptively: a compact `i64` fast tier, an `i128` middle tier, and arbitrary-precision signed limb storage beyond that. The erased wrapper must keep an ordinary small integer machine-word-sized where the target permits; it must not inflate every `int` to an inline 128-bit payload merely because wider values are supported. A wide tier may therefore be boxed or share a wide/big allocation header. Statically proven values may lower directly to `i64`, `i128`, or specialised limb operations without constructing the erased wrapper.

Every completed `int` operation normalises its result to the smallest tier that represents it exactly: first `i64`, then `i128`, then arbitrary precision. Thus a widened value that later falls within `int64` range becomes compact again, and a big result that fits `i128` or `i64` does not remain unnecessarily large. Tier choice is not observable through equality, ordering, hashing, serialization, source reflection, ownership, or value semantics; profiling and generated-code inspection may report it as a physical cost.

The fixed-width integer names are distinct source types whose bounds and bit widths are contractual. Their ordinary arithmetic never promotes to `int` or another width. They exist for bounded storage, predictable machine operations, layout, and ABI contracts.

### 11.2 Literals are typed objects

```text
x = 42          # int
y = 3.14        # float
enabled = true  # bool
name = my rifle # string
empty = none    # none
```

An unconstrained whole-number literal is an `int` regardless of magnitude. The front end parses its magnitude without a fixed-width limit, and the compiler selects the smallest exact runtime tier. In a fixed-width initializer, the literal is checked at compile time against the destination range:

For a signed fixed-width initializer whose source is a syntactic unary `-` applied directly to a whole-number literal, range checking applies to the signed mathematical value after negation, not to the positive magnitude first. Thus `minimum int8 = -128` and the corresponding minimum of every signed width are valid, while `below int8 = -129` is rejected. Parenthesised constant expressions use the same compile-time constant evaluation and destination-range check; this rule introduces no general implicit conversion.

```text
large = 9223372036854775808
wide int128 = 9223372036854775808
too-large int64 = 9223372036854775808 # compile-time range error
```

This contextual literal check is not an implicit runtime conversion. The compiler may represent scalar objects as native Rust primitives when semantics permit.

### 11.3 Dynamic bindings

A binding without a type annotation may be rebound to another type:

```text
x = 42
x = forty two
```

This is dynamic binding, not an untyped value.

The compiler may still infer a concrete representation over regions where the type is stable.

### 11.4 Typed bindings and definite assignment

A type expression follows the binding name. An initializer is optional:

```text
count int = 42
ratio float = 0.5
name string = 'alice'

cpu int
result task-struct|none
```

An initialized typed binding is immediately available. A typed declaration without `=` creates a binding with no value; it does not construct a default value, contain `none`, zero storage, or invoke the type. Every control-flow path must definitely assign a compatible value before any read, reference creation, move, member access, argument passing, or capture of that binding. Failure is a compile-time error.

```text
cpu int

if use-current-cpu
  cpu = current-cpu;
else
  cpu = fallback-cpu

print; cpu
```

The compiler performs flow-sensitive definite-assignment analysis across branches, loops, `try`/`catch`/`finally`, labels, and `goto`. A jump may not bypass required initialization. Leaving the scope of a never-initialized binding drops nothing; once initialized, its ordinary lifetime and cleanup rules apply.

Untyped declarations without assignment do not exist: `value` alone remains an expression, not a declaration. `var` is not a declaration keyword. Initialization never requires ceremony:

```text
total int = 0
```

Typed assignment is strict:

```text
ratio float = 42
```

is a type error because the value is an `int`.
### 11.5 Explicit coercion

Coercion is a callable method family on the source value:

```text
x = 42
x = x.coerce; float
x = x.coerce.checked; int8
```

The bare invocation is its throwing default. `coerce.checked` returns an absence-aware result without a representability throw; `coerce.wrap` and `coerce.saturate` are available only where the source/destination policy table defines them. A receiver is evaluated exactly once before policy selection and arguments. The complete call, including its statically resolved destination descriptor, determines whether a policy exists; selecting a family alone does not make it a freely storable bound method value in version one.

`coerce` either returns an object compatible with the requested type or throws `.coercion-error`.

There is no universal guarantee that every type can coerce to every other type.

Coercion among integer types follows §17.7 exactly. Coercion to a floating-point destination rounds to the nearest representable value using the IEEE 754 default round-to-nearest, ties-to-even rule; because that rounding is defined for every finite source magnitude, an inexact numeric-to-float coercion is a normal result rather than a failure, and precision loss is visible through the destination type rather than through an error. A source magnitude beyond the destination's finite range throws `.coercion-error`; it never yields an infinity, because a silent infinity is a lost error rather than a result. `checked` returns absence for exactly that overflow case.

Conversions are declared rather than universal. A descriptor declares the source/destination pairs it supports, and `coerce` attaches exactly where a declaration exists, so an undeclared pair is absent from the type rather than a runtime failure. Declaration coherence — what happens when two protocols declare the same pair, and whether a declaration may be added for a type the author does not own — is part of the conversion-protocol contract. A caller-supplied conversion callback is admitted for pairs no descriptor declares, and therefore cannot precede first-class function values.

`bool` converts to integer destinations as a declared, total, lossless conversion: `false` is `0` and `true` is `1`. The reverse is not a conversion at all. Integer-to-`bool` is a predicate choice rather than a change of representation, and must be written as an explicit comparison.

Neither the default child nor `checked` substitutes a value for a failure: an unrepresentable, unparseable, or undeclared conversion throws under the default child and returns `none` under `checked`. A total conversion that yields a fixed value on failure — `0` for an unparseable string, in the style of PHP's `intval` — is permitted only as a separately named child, so the substitution is visible at the call site rather than inherited by every plain `coerce`. Such a child is optional and unspecified in version one; if it is added, its name must state that it substitutes.

Parsing coercion from `string` to a numeric destination accepts the canonical text-display spelling of that destination and throws `.coercion-error` when parsing fails. `coerce` takes no argument beyond its destination and must never acquire a radix or format option: acquiring one would absorb the interpretation role that belongs to `parse`, and the separation between the two would collapse. This is an invariant of the design rather than a description of the current surface.

Interpretation in a base other than ten is a distinct operation attached by receiver: `text.radix; 16` interprets base-sixteen text and yields an adaptive `int`, while `value.radix; 16` renders a number in that base as `string`. Narrowing after interpretation is ordinary coercion and follows the call-extent rule, as in `(text.radix; 16).coerce; int8`.

Locale-dependent parsing belongs to an imported formatting facility, never to `coerce`.

### 11.5.1 User-supplied interpretation: `parse`

`coerce` covers the conversions the language defines. Interpretation the language does not define is supplied by the program through `parse`, which always takes a callback as a required argument:

```text
function to-code int|int8; input string
  if input == 'foobar'
    return 10
  return 20

d string = 'foobar'
print; d.parse; to-code
```

There is no built-in destination-owned `parse`. The member exists to apply a program's own interpretation to a receiver, so a form without a callback would have no operation to perform.

`parse` differs from every other member in where its result type comes from: `coerce; int8` is typed by its destination descriptor, whereas `d.parse; to-code` is typed by the callback's declared return, here `int|int8`. That union is then checked at the destination by ordinary union rules — `value int8 = d.parse; to-code` is rejected because the `int` alternative is not assignable to `int8` — and the diagnostic is available statically from the callback's declaration. No parse-specific runtime recheck exists.

The `checked` child catches a callback that throws and yields absence, which plain application of the same function cannot express:

```text
d.parse; to-code            # propagates a throw from the callback
d.parse.checked; to-code    # int|int8|none
```

In version one the callback must be a statically resolvable function name rather than an arbitrary expression. The compiler then resolves and inlines it exactly as it resolves a coercion destination, with no runtime callable representation and no boxed value. The restriction lifts when first-class function values arrive.

### 11.6 Type objects

A type is a language construct backed by a canonical object, not an independently instantiated value. Binding one names the construct; it does not produce a runtime value:

```text
target-type = float
x = x.coerce; target-type
```

`target-type` is a compile-time descriptor alias. It may appear anywhere the construct may appear — annotation position, a coercion destination, the right side of `is a` — and it may not appear where a runtime value is required. Passing it to `print`, using it in arithmetic, or handing it to a parameter expecting a value is rejected at the source span, because a descriptor has no display or value protocol in version one.

A descriptor binding therefore has no runtime representation and lowers to nothing. Erasure is definitional rather than an optimisation the compiler is permitted to make: there is no storage to elide. A binding that emits a Rust name for a descriptor is a defect, not a fallback.

A class object may be bound and used as a type expression:

```text
from /models import .user
user-type = .user

person user-type = .user; data
```

The compiler resolves type compatibility through the object’s type protocol.

Alongside the concrete descriptors, `/core types` exports abstract category descriptors: `number`, `integer`, `fixed-integer`, `signed-fixed-integer`, `unsigned-fixed-integer`, and `floating`, beneath the two identity roots `value` and `object`. `int` implements `integer` and `number` but no fixed-width contract; `int8` through `int128` implement `signed-fixed-integer`, `fixed-integer`, `integer`, and `number`; `uint8` through `uint128` implement `unsigned-fixed-integer` in place of the signed contract; `float`, `float32`, and `float64` implement `floating` and `number`. The roots `value` and `object` classify identity, copy, and ownership behaviour rather than numeric capability, so no arithmetic or conversion member attaches to them.

These are interface and category contracts used for member attachment, compatibility, reflection, and finite-union reasoning. None of them is a storage supertype, and none creates an implicit assignment conversion. Like the concrete fixed-width descriptors, they are descriptor constructs available without import rather than prelude bindings: the default prelude's ordinary bindings are unchanged, and a construct name is usable in construct position directly while explicit import remains available for rebinding, aliasing, and shadowing. In particular, fixed-width integers are not assignment-compatible subclasses of `int`: that would contradict explicit coercion and the differing arithmetic result contracts.

Type objects are canonical compiler-owned descriptors with stable type identity. The backing object is real — `.type` returns it, `is a` compares it, canonical identity survives rebinding under another name, and reflection exposes it later — but it is never independently constructed by source. Source-observable behavior must remain the same as naming the descriptor directly: `.type`, identity, compatibility queries, and operations such as `coerce` all consult the same canonical descriptor. Version one does not accept an arbitrary runtime value as a type expression or coercion destination; the value must resolve to a finite, compiler-known descriptor alternative so lowering remains statically representable.

### 11.7 Union and parameterised types

Union types use `|`. `none` is an ordinary union member rather than a special generic wrapper:

```text
name string|none = none
value int|float = 42
function parse int|parse-error; source string
```

The spelling `optional<thing>` is not part of the language: write `thing|none`. `none` is not automatically admitted into every type.

The word `of` applies a parameterised type constructor using the language's fixed constructor-application grammar:

```text
items list of string = .list;
stacks array of vm-struct|none, nr-cached-stacks
callback function from int, borrowed-ref of opaque to int
```

Packages may supply type-constructor objects, but they cannot add type-expression grammar. Every constructor argument is parsed into the same unified constructor-argument syntax node; the parser does not guess whether an identifier denotes a type or a compile-time value. Semantic analysis resolves each argument against the constructor's declared signature and reports whether a type, constant value, or other permitted compile-time object was required. Thus `array of vm-struct|none, nr-cached-stacks` can accept a type followed by a constant extent without lexer or parser knowledge of `array`.

Comma-separated arguments after `of` belong to the same type application. `|` forms a union within the current constructor argument; grouping may override the resulting structure. Angle-bracket generic spelling such as `list<string>`, `array<thing, 4>`, or `function-reference<int, void>` is not Terrane syntax.

Functions have one core type shape because functions are core objects:

```text
function to result
function from int, string to boolean
ref function from int to int
array of function from int to boolean, 16
```

`function to R` takes no arguments. In `function from A, B to R`, the comma separates parameter types and the final `to` introduces the return type. Function types associate to the right: an ungrouped nested `function` consumes its own `to` and return type before parsing resumes in the enclosing parameter list. The formatter must add grouping whenever nested `from`/`to` structure would otherwise be difficult to scan. Calling convention, variadic behaviour, and foreign ABI are type-constructor or declaration metadata; they do not alter this core grammar.

Type constructors and function types remain human-facing and compositional. Compilers, formatters, documentation, and generated bindings must render these canonical forms rather than leaking Rust, C++, or adapter-specific generic notation.
### 11.8 Source generic declarations

The first core language deliberately does not declare source type parameters. `list of string` applies a constructor supplied by the language or a package; it does not imply that users can declare `T`. Generic Rust APIs may be exposed only when an adapter can erase them behind a concrete object/interface contract or generate named concrete instantiations. Otherwise they require a wrapper and are not directly representable.

Strict code uses concrete types, unions, interfaces, or generated concrete declarations. It must not fall back to dynamic typing merely to simulate a missing type parameter. Source-declared generics remain a future language change requiring syntax, constraint rules, inference, dispatch, reflection, and code-generation semantics; no implementation may invent private syntax meanwhile.
### 11.9 Strict typing scopes

A `strict types` directive may apply to a function, class, namespace, package, or build profile.

In strict type mode:

- public parameters and returns must be typed;
- fields and globals must be typed;
- incompatible assignments are errors;
- implicit coercion remains forbidden;
- dynamic locals may still be permitted when explicitly marked or inferred under a project policy.

Strictness is local and composable. A strict package may call a dynamic package through generated checked boundaries.

### 11.10 Type checking time

A type violation should be reported at compile time when provable.

When a dynamic value crosses a typed boundary and its concrete type is not known until runtime, the generated program performs a runtime check and throws a source-language type error.

Version one reaches that runtime path in no ordinary program. Because it admits a dynamic binding only when its alternatives form a finite compiler-known set, as stated in §9.6, protocol availability and typed-boundary compatibility are decided statically across every alternative and incompatibility is a compile-time error. The runtime check exists for later erased or foreign dynamic values whose complete alternatives are unavailable at compilation.

### 11.11 Truth

Conditions use an object’s truth protocol:

```text
if value
  ...
```

`bool` implements truth directly.

Other standard objects may implement truth, but the rules are explicit and inspectable rather than a collection of ad hoc coercions.

Strict type mode may require a `bool` condition.

### 11.12 `none`

There is exactly one core absence value:

```text
none
```

It is distinct from:

```text
false
0
''
.list;
```

A typed binding rejects `none` unless its type expression includes it.

### 11.13 Equality, identity, and type membership

The language keeps three different questions separate:

- `a == b` asks whether the values are equal;
- `a is b` asks whether both expressions denote the same source-visible identity;
- `a is a type` asks whether the value is an instance of, subtype of, or interface-compatible with the type expression.

`==` performs value equality with no unrelated implicit coercion. A type may explicitly define meaningful cross-type equality through its equality protocol, but equality never performs a hidden general conversion merely to make operands comparable.

`is` observes semantic identity only. Copy-on-write backing storage, compiler boxing, interning, and other representation sharing are not observable through it. If either evaluated operand has no source-visible identity, the result is false, even for `x is x` or two evaluations of `items[0]`. Obtaining an explicit `ref` creates or preserves source-visible identity; comparing aliases of that identity is true.

```text
a = .list; 1, 2
b = a
c = ref a
d = c

a == b  # true
a is b  # false under value assignment
a is c  # true
c is d  # true: value assignment of a ref value preserves the referenced identity
42 is 42 # false
```

`is a` is a contextual two-word operator whose right operand is a type expression:

The parser treats `is a` as type membership only when the contextual `a` is followed by a complete type expression. At the end of an expression, or whenever no type expression follows, `a` remains an ordinary identifier and `left is a` is identity comparison against that binding. Thus `value is a serializable` is membership while `c is a` is identity. Formatters preserve the two-word membership spelling and do not rewrite identity comparisons.

```text
if value is a serializable
  print; value
```

It tests assignability to that type, not exact runtime-type equality. It is true for an instance of the named class, a permitted subclass, an implementation of the named interface, or a value admitted by a union type. `isa` is not an operator: it remains available as an ordinary identifier and is less readable than the separated phrase.

The following values carry source-visible identity without requiring a new `ref` at the comparison site:

- every value participating in an explicit `ref` identity group, including the original logical value from which the reference was obtained;
- linear and other uniquely owned resource objects, such as device handles, capabilities, guards, and foreign-runtime proxies;
- canonical semantic descriptor objects whose contract defines one identity, including type, namespace, package, and declared-function descriptors.

Other ordinary values—including scalars, strings, collections, non-linear class instances, closures, and bound methods—have no source-visible identity merely because an implementation boxes, interns, caches, or shares them. Their type may expose identity only through `ref` or by declaring an inherently identity-bearing linear/resource/descriptor contract. Whether a type is inherently identity-bearing is reflected in its public type metadata and cannot vary secretly by representation or instance.

Exact runtime type is expressed through the value’s `type` descriptor. Requiring both exact type and value equality remains an explicit conjunction:

```text
left == right and left.type is right.type
```

The language does not define `===`. Because `==` already forbids unrelated coercion, a “strict equality” spelling would be redundant; making it secretly combine type equality and value equality would hide two independent predicates and leave subclass/interface semantics unclear.

Mutable values used as hash keys must either be rejected or use a stable immutable key projection.

---

## 12. Assignment, copying, references, and ownership

### 12.1 The central rule

> Assignment creates an independently mutable value using copy-on-write. `ref` shares mutable identity. `move` transfers ownership.

### 12.2 Value assignment

```text
b = a
```

has value semantics. After the assignment, mutations to `b` must not become visible through `a`, and mutations to `a` must not become visible through `b`.

This guarantee applies uniformly to ordinary scalars, strings, collections, class instances, functions, and other non-linear values. Assignment already provides independently mutable value semantics, so the source language needs no separate operation for eager duplication.

### 12.3 Universal copy-on-write

The normal implementation should share a value’s backing representation until mutation requires separation:

```text
a = .list; 1, 2, 3
b = a
```

At this point `a` and `b` may share the same storage. Neither binding has shared mutable identity.

```text
b.append; 4
```

must separate the storage needed by `b` before mutation so `a` remains unchanged.

The same rule applies recursively to objects and collections. Mutating a nested field or element separates enough of the path to preserve the other logical value:

```text
b.profile.name = 'new name'
```

An implementation may use reference-counted backing storage, persistent data structures, path copying, a trivial machine copy, Rust `Copy`, copy elision, or an immutable representation. These representation references are not source-language `ref` values and are not observable as shared identity.

By-value containment cannot create an identity cycle: a cyclic or back-reference edge must use an explicit `ref`. Implementations may therefore share acyclic value storage without turning every program into a tracing-GC program.

Creating `ref a` makes the logical value currently denoted by `a` and every alias in the resulting reference group identity-bearing; this is why `a is c` is true after `c = ref a`. It does not give identity to independent values that merely share copy-on-write storage.

A reference to a field, element, or other path inside a copy-on-write value is permitted only while the compiler can preserve a stable logical owner. Taking `ref items[0]` first separates `items` from any independent values with which it shares backing storage, then creates identity for that element path and pins that path against relocation while the reference is live. A later value assignment of `items` produces an independent logical value; mutations of that copy separate from the pinned owner. The reference continues to reach the element in the original `items`, never whichever backing allocation happens to survive a split. Operations that could invalidate or remove the referenced path, such as removing that element or replacing its container wholesale, are rejected while the reference is live. If a container cannot implement this contract for an operation or target, taking the interior `ref` is rejected at compile time.

Tracing and profiling must distinguish:

- semantic value assignments;
- shared-storage assignments;
- physical copies;
- copy-on-write splits;
- copies elided by optimisation.

### 12.4 Explicit reference

```text
b = ref a
```

creates shared mutable identity for the logical value currently held by `a`.

Mutations through either identity are visible through the other:

```text
a = .thing;
b = ref a

b.value = 10
print; a.value  # 10
```

If `a` previously received its value through ordinary assignment, creating or mutating an explicit reference must not pull other independently mutable values into the reference group:

```text
original = .thing;
copy = original
alias = ref copy

alias.value = 10
print; copy.value      # 10
print; original.value  # unchanged
```

The implementation separates `copy` from `original` when required, while `copy` and `alias` intentionally retain shared identity.

In this draft, `ref` aliases the logical value identity, not the lexical binding slot. Rebinding `a` later does not retarget `b`:

```text
a = .other;
```

`b` still refers to the original value.

Binding-slot aliases are deliberately not part of the core draft because they complicate closures, concurrency, and source reasoning. They may be revisited only if a compelling use case survives those costs.

### 12.5 Reference type contracts

`ref` is also a prefix type constructor for a binding whose contract requires source-visible shared identity:

```text
p ref task-struct
p = ref task

function update-task; task ref task-struct
```

The two positions are deliberately symmetric. `ref task-struct` is a type expression; `ref task` is an expression that obtains shared identity. Parsing is unambiguous because a type expression follows a declared binding name, while the operation appears where a value expression is required.

A value assigned or passed to `ref T` must already carry compatible reference identity or be produced with the explicit `ref` operation at that boundary. The compiler must not silently turn value assignment into reference sharing merely because the destination expects `ref T`.

`reference T`, `reference<T>`, and adapter-shaped aliases such as `function-reference<...>` are not core spellings. The unqualified `ref T` contract means a safe, lifetime-checked alias to source-visible object identity; it does not mean “some machine address”. `void` means that an operation produces no value, principally as a return contract; it is not an erased storage type and `ref void` is invalid.

`opaque` is the core type whose representation is unavailable at the current boundary. It supplies no operations by itself. Reference and adapter contracts compose with it explicitly: `borrowed-ref of opaque` is a lifetime-bounded erased borrow, while `raw-address of opaque`, `user-ref of opaque`, or a package-owned `c-pointer of opaque` retain their distinct provenance and safety rules. An adapter must not translate `void *` mechanically: it selects the narrowest contract actually guaranteed by that API.

Lower-level packages may expose stricter type constructors when the distinction changes what operations are legal:

- `borrowed-ref of T` is a non-owning, lifetime-bounded borrow and cannot outlive its lender;
- `user-ref of T` is an untrusted userspace address that cannot be dereferenced until an adapter validates or copies it;
- `raw-address of T` is an integer-like machine address with provenance and alignment obligations, usable only in `unsafe`;
- `array-ref of T` is a borrowed contiguous view whose extent is carried by its value or an accompanying contract;
- `function from A, B to R` is the core callable type; `ref function from A, B to R` adds safe source-visible callable identity, while a package-owned ABI-address constructor may impose a calling convention or foreign provenance.

These contracts are not aliases for one another. Adapters define package-owned operations and lowering, but may not weaken the core guarantees: a `user-ref` never silently becomes `ref`, a `raw-address` never silently becomes dereferenceable, and a borrow cannot escape its proven lifetime. Use `ref T` when shared language-level identity is intended; use a narrower domain type only when provenance, address space, extent, ABI, or lifetime differs observably.

Linear resource values are inherently identity-bearing because their unique ownership denotes one source-visible resource even before `ref` is taken. Moving such a value preserves that identity; the moved-from binding becomes unavailable. If the type permits `ref`, aliases compare identical to the resource. Foreign-runtime proxies follow the same rule because the proxy contract denotes a particular foreign object.

Every borrow carries compiler-assigned provenance and a compiler-assigned lifetime region; ordinary source does not name these regions. Member lookup, indexing, iteration, destructuring, calls, and other values derived from a borrow preserve its provenance and may retain or narrow its lifetime, but never widen it. Assignment, return, closure capture, field storage, global storage, and async suspension must preserve that constraint. A borrowed collection yields borrowed elements unless its declared protocol explicitly returns independently owned values or shared identity. Diagnostics identify the source binding that originated the borrow and the operation that would let it escape.

### 12.6 Ownership transfer

Some values are inherently linear or non-copyable: exclusive device handles, unique capabilities, interrupt guards, or other low-level resources.

Such a type may reject ordinary value assignment.

Ownership transfer is explicit:

```text
b = move a
```

After a successful move, `a` is unavailable until rebound.

This keeps Rust-like ownership consequences available without imposing move semantics on ordinary application values.

### 12.7 Linear classes

A class may declare itself linear:

```text
linear class device-handle
```

Linear objects:

- cannot be value-assigned unless they define an explicit copy protocol;
- may be moved;
- may be referenced subject to lifetime and mutability rules;
- are deterministically dropped.

### 12.8 Constants and immutability

A constant binding cannot be rebound:

```text
constant answer = 42
```

This does not necessarily make the referenced object deeply immutable.

Deeply immutable/frozen values should be expressed through the object/type contract rather than conflated with binding constancy.

### 12.9 Reference implementation strategy

The generated Rust may realise references as:

- ordinary borrows when statically provable;
- mutable borrows when exclusive mutation is provable;
- `Rc`-like ownership in single-threaded hosted code;
- `Arc`-like ownership where cross-thread sharing is required;
- target-specific handles;
- custom runtime references for dynamic object graphs.

The compiler must not silently introduce locks merely to make an unsafe sharing pattern compile.

### 12.10 Reference cycles

Ordinary value assignment does not create shared-identity cycles.

Explicit references can.

The core model therefore includes a weak reference form:

```text
parent = weak ref child
```

A hosted runtime may optionally provide cycle detection/collection for dynamic reference graphs, but the language does not require a tracing garbage collector for all programs. Collection of an unreachable strong-reference cycle does not have a deterministic time.

For allocator-free targets:

- strong cycles must be rejected when provable;
- runtime-created uncollectable cycles are a program error or leak;
- weak references are the standard back-reference mechanism.

The profiler should report retained strong-reference cycles where runtime metadata permits.

### 12.11 Deterministic lifetime

Owned values are destroyed deterministically when they leave scope. Acyclic reference-backed objects are destroyed when the final strong owner is released.

This guarantee does not extend to unreachable strong-reference cycles: they may leak, be rejected by a target profile, or be reclaimed later by optional hosted cycle collection. Code must not depend on a cycle's collection time or finalisation order. Scarce resources and externally visible cleanup should use lexical ownership, a scoped guard, or an explicit close/release protocol rather than rely on cyclic graph collection.

These guarantees permit ordinary lexical code to manage resources without requiring a Python-style context-manager ceremony for every resource.

`try`/`finally` remains available when cleanup must happen at a control-flow boundary independent of ownership.

---


## 13. Functions, parameters, and returns

### 13.1 Function declarations

A function with no declared arguments is:

```text
function main
  ...
```

Parameters follow a semicolon:

```text
function add; a, b
  return a + b
```

Return types follow function names, and parameter types follow parameter names:

```text
function add int; a int, b int
  return a + b
```

### 13.2 Optional parameters

A parameter with a default value is optional:

```text
function connect; host string, port int, timeout float = 5, retries int = 2
  ...
```

Calls may provide optional parameters positionally:

```text
connect; host, port, 10, 3
```

Named arguments are clearer when selected optional values are overridden:

```text
connect; host, port, timeout=10, retries=3
```

### 13.3 Variadic parameters

A parameter followed by `...` collects remaining values:

```text
function collect; values ...
```

Variadic values are exposed as a list-like object.

Only one variadic parameter is permitted.

### 13.4 Default values

Defaults use ordinary assignment syntax:

```text
function request; url string, timeout float = 5, retries int = 0
```

Default expressions are evaluated according to a declared policy:

- immutable compile-time values may be shared;
- mutable defaults must be freshly value-copied for each call;
- expressions with side effects are evaluated at call time.

This avoids Python-style shared mutable default behaviour.

### 13.5 Named arguments

Arguments may be named when the function exposes stable parameter names:

```text
resize; width=100, height=50
```

A call must not bind the same parameter both positionally and by name.

### 13.6 Return types

A return type follows the function name:

```text
function area int
  return this.width * this.height
```

A function without a return type is dynamically returning. A function with several possible return types uses a union:

```text
function parse int|parse-error; source string
```

A function may return `none` explicitly or implicitly at the end of its body.

Multiple logical results should normally be returned as an object or tuple:

```text
return .tuple; value, error
```

rather than inventing a second assignment protocol.

### 13.7 Early return

```text
if invalid
  return none
```

`return` without a value returns `none`.

### 13.8 Anonymous functions and closures

An anonymous function omits the name:

```text
handler = function; request
  return process; request
```

Closures capture outer values by value by default, following ordinary assignment semantics.

To share mutable identity with a closure, capture or assign an explicit reference before creating it:

```text
counter-ref = ref counter

handler = function
  counter-ref.increment;
```

This keeps closure capture consistent with the rest of the object model.

### 13.9 Recursion

A named function may refer to its own binding.

Mutually recursive functions are resolved at namespace analysis time. Their declarations are visible throughout the namespace compilation group, while executable top-level assignments retain source order.

### 13.10 Generators

A `yield` form should be part of the core language:

```text
function numbers; maximum int
  for i = 0; i < maximum; i++
    yield i
```

A yielding function returns an iterator object.

The compiler may lower a generator to:

- a static Rust iterator;
- a generated state machine;
- a boxed dynamic iterator when required.

Generator support may follow the first compiler milestone, but its semantics should be reserved early to avoid later control-flow conflicts.
### 13.11 Generic functions

Functions cannot declare type parameters in the first core language. See §11.8. An interface-typed function is dynamically dispatched through that interface contract; it is not an implicitly monomorphised generic.

---

## 14. Control flow

### 14.1 Conditions

```text
if condition
  ...

else
  ...
```

Else-if is written plainly:

```text
if first
  ...

else if second
  ...

else
  ...
```

No trailing colon or parentheses are required.

### 14.2 `while`

```text
while condition
  ...
```

### 14.3 Collection iteration

```text
for item in things
  print; item
```

Destructuring is permitted when the iterator yields a matching tuple/object shape:

```text
for key, value in mapping
  message = ': '.concat; key, value
  print; message
```

### 14.4 Three-clause `for`

The same `for` construct supports explicit initialisation, condition, and update clauses:

```text
for i = 0; i < 10; i++
  print; i
```

The update may be written without `++`:

```text
for i = 0; i < 10; i = i + 1
  print; i
```

The parser distinguishes the two forms by `in` versus semicolon-separated clauses.

### 14.5 Increment and decrement

Postfix `++` and `--` are statement/update operations on compatible mutable numeric bindings.

They return the previous value only if used in an expression; linters should discourage clever expression use.

The forms lower through numeric increment/decrement protocols. For fixed-width receivers they retain checked overflow behaviour unless an explicitly wrapping operation is selected; for `int` they compute the exact mathematical successor or predecessor and promote representation as necessary.

### 14.6 Loop control

```text
break
continue
```

A value-returning `break` may be considered for expression loops later, but is not required in the first implementation.

### 14.7 Labels and `goto`

Low-level control flow may name a statement position and jump to it:

```text
if error
  goto bad-fork-cleanup-mm

...

label bad-fork-cleanup-mm
  release-mm; task
```

Labels are function-local. A `goto` may target only a label in the same function.

A jump may remain in its current lexical scope or leave scopes, but it may not enter a deeper lexical scope. Leaving scopes performs their deterministic destruction and other language-required cleanup in the same order as ordinary scope exit.

A jump must not cross an initialisation, move, borrow, deferred cleanup, `unsafe` boundary, or other lifetime transition in a way that would leave a value uninitialised, use a moved value, bypass required cleanup, or otherwise violate the language's ownership and lifetime rules. These are compile-time errors; `unsafe` does not relax them.

The compiler must prove that the generated Rust representation is sound. It may lower labels and jumps to structured control flow, a state machine, or another explicit representation, but it must preserve source control flow, cleanup order, diagnostics, debugging, and source mapping. It must not emit unsound Rust or rely on Rust having a native `goto`.

This feature exists for state machines, parsers, kernels, and failure-unwind paths where forced restructuring would duplicate cleanup or obscure the real control flow. Ordinary structured control flow remains preferred when it expresses the same behaviour clearly.

### 14.8 Pattern matching

Pattern matching is useful enough to reserve `match`, but it is not required for the minimum compiler.

A likely form is:

```text
match value

  case .success as result
    ...

  case .failure as error
    ...

  else
    ...
```

The final grammar should be validated against ordinary object/type matching before implementation.

---

## 15. Errors and exceptional control flow

### 15.1 Throwing

```text
throw error
```

A constructed error may be thrown directly:

```text
throw .file-error; path
```

Any object may technically be thrown in dynamic mode. Standard tooling expects thrown objects to implement the error protocol.

All catchable failures implement a structural `error` interface carrying a stable `kind`, a human-readable `message`, an optional `cause`, and a source-context chain. `kind` is the matchable identity and is stable across releases; `message` is for humans and is not a matching key. Throwing uses a compiler-owned result propagation representation rather than native unwinding, so lowering stays deterministic and readable.

Strict mode may require an error-compatible object.

### 15.2 Catching

```text
try
  file = .file; path
  data = file.read;

catch .file-error as error
  print; error.message

catch .error as error
  throw error

finally
  log; 'finished'
```

Catch clauses are evaluated in source order. The written order is the executed order: the compiler never reorders clauses by specificity, and a clause made unreachable by an earlier one is a compile-time diagnostic rather than silently dead code.

A catch object denotes a compatible error type or matcher — a concrete error descriptor or a declared error interface.

Uncaught errors render the deterministic cause and source chain, then exit through the profile's failure policy.

### 15.3 `finally`

`finally` executes regardless of:

- normal completion;
- `return`;
- `break`;
- `continue`;
- source-language throw.

Behaviour during process abort, hardware failure, or unsafe Rust undefined behaviour cannot be guaranteed.

### 15.4 Lowering model

Recoverable source-language throws should lower primarily through Rust `Result`-like control flow, not Rust panic unwinding.

The compiler may synthesise propagation code so source remains uncluttered.

A function's public contract records whether it may throw. The `throws` qualifier may declare that effect before `function`; otherwise it is inferred for non-public functions and must be written or compiler-generated in exported interface metadata. A direct call to a function proven not to throw is non-throwing. A call through a dynamic callable or interface whose contract does not explicitly exclude throwing is conservatively may-throw. Propagation remains implicit in source, but reflection and generated signatures expose it; generated Rust therefore uses `Result`-like propagation at every may-throw boundary.

Rust panic is reserved for unrecoverable invariant failure, explicit panic, or a native dependency panic that is not translated.

### 15.5 Standard error objects

The `/core errors` namespace defines the standard error protocol and the following language-mandated error objects:

| Object | Meaning | Operations that raise it | Required information |
|---|---|---|---|
| `.arithmetic-overflow` | A checked fixed-width arithmetic result is outside the receiver type's range. | Ordinary checked fixed-width addition, subtraction, multiplication, signed negation, increment/decrement, and signed `MIN / -1`. | operation and fixed-width type |
| `.division-by-zero` | An integer division or remainder operation has a zero divisor. | `/`, `%`, and `div-rem` for every integer type and arithmetic mode. | operation and numeric type |
| `.integer-conversion-overflow` | An explicit throwing integer conversion cannot represent the mathematical source value in its destination type. | `coerce` to a fixed-width integer destination. | source value/type and destination type |
| `.negative-shift-count` | An integer shift count is negative. | Unbounded-`int` `<<` and `>>`. | attempted count and shift operation |
| `.coercion-error` | An explicit coercion has no result compatible with the requested destination, outside the integer-overflow case above. | `coerce` where the source value or text cannot be represented in the destination type, including parsing coercion from `string` and an out-of-range floating-point destination whose protocol does not declare infinity. | source value/type and destination type |

Each is a subtype or conforming instance of `.error`, is catchable through the ordinary `throw`/`catch` model, and has the standard `message` plus the structured information listed above. Implementations may attach additional diagnostic fields without changing program-visible matching. Names such as `.file-error`, `.not-found`, `.config-error`, and `.python-error` used elsewhere are package- or adapter-defined error objects, not additional implicit core errors.



### 15.6 Panic

A standard panic object or operation should exist separately from `throw`.

```text
panic; impossible state
```

Build profiles may choose abort or unwind behaviour.

Kernel and embedded profiles will commonly abort or invoke a target panic handler.

### 15.7 Stack traces

An uncaught error reports:

- source-language namespace/function frames;
- source spans;
- object/type context;
- generated Rust spans as expandable detail;
- native frames for explicit Rust/C code;
- foreign-runtime frames and tracebacks at explicit runtime boundaries;
- causal chains for wrapped errors;
- async task ancestry where available.

---

## 16. Collections and iteration

### 16.1 Standard collection objects

The core standard environment should provide:

```text
.list
.map
.set
.tuple
.range
.entry
```

These remain objects and are not compiler-only species.

### 16.2 Lists

A list may be constructed with ordinary invocation:

```text
items = .list; a, b, c
```

Square-bracket syntax is recommended as compact sugar:

```text
items = [a, b, c]
```

On a standard UK keyboard, brackets do not violate the ordinary no-Shift ergonomic goal.

### 16.3 Maps

A map with simple textual keys may use named construction arguments:

```text
users = .map; alice=user-a, bob=user-b
```

Computed keys use entries:

```text
users = .map;
users.set; key-a, user-a
users.set; key-b, user-b
```

or:

```text
users = .map; .entry; key-a, user-a
```

The exact multiline entry sugar may be refined by prototype use; the object and method semantics are fixed.

### 16.4 Sets and tuples

```text
unique = .set; a, b, c
pair = .tuple; first, second
```

Tuples are fixed-length value objects.

Lists, maps, and sets are value-semantic copy-on-write objects by default.

### 16.5 Indexing

Indexing uses brackets:

```text
first = items[0]
value = mapping[key]
```

Assignment through an index is mutation and therefore triggers copy-on-write separation where required:

```text
items[0] = replacement
```

### 16.6 Slices and ranges

Ranges are objects:

```text
range = .range; 0, 10
```

A concise range form such as `0..10` may be supported.

Slicing should use range objects rather than accumulating multiple special colon grammars:

```text
part = items[.range; 10, 20]
```

### 16.7 Iteration protocol

`for ... in ...` invokes the iteration protocol.

An iterator's advancing operation returns a dedicated finite result, `iteration-step of Item`, with `item of Item` and `end` alternatives. The item may itself be a tuple or destructurable object.

Exhaustion is `end`, never `none`, because `none` may be a legitimate item. Iterators are stateful linear objects; `end` is sticky, and advancing after `end` returns `end` without consulting the source again. `for` desugars through this protocol and neither exposes nor synthesises a sentinel value.

The compiler may statically lower standard iterators to native Rust iterator chains.

### 16.8 String iteration

A `string` stores Unicode text, conventionally as UTF-8.

The standard API distinguishes three explicit units:

- `bytes`: UTF-8 encoded bytes;
- `scalars`: Unicode scalar values;
- `graphemes`: Unicode extended grapheme clusters, corresponding most closely to user-perceived characters.

The default `string.length` is the number of grapheme clusters:

```text
text.length
text.bytes.length
text.scalars.length
text.graphemes.length
```

`text.length` and `text.graphemes.length` are semantically identical. The explicit form is useful when the unit deserves emphasis alongside byte or scalar operations. `text.bytes.length` reports encoded storage bytes; an API named `raw` is deliberately avoided because it does not identify a unit.

Grapheme and scalar counts generally require traversal, while the UTF-8 byte length may be available in constant time. Performance tooling should expose that distinction rather than changing the default unit. Grapheme operations, including default `string.length`, require the Unicode grapheme-segmentation-data capability. A target without it reports the source operation and suggests `text.bytes.length`, `text.scalars.length`, or enabling/providing the capability; it must not silently substitute another unit. Programs that use only byte or scalar views do not acquire the grapheme capability.

String indexing should either return graphemes or be rejected in favour of explicit views; it must never ambiguously mean bytes on one target and characters on another.

### 16.9 Bytes

`bytes` is separate from `string`.

No operation silently treats arbitrary bytes as valid text. Decoding and encoding are explicit object operations:

```text
text = data.decode; utf8
data = text.encode; utf8
```

---

## 17. Operators

### 17.1 Standard operators

The language supports familiar operators:

```text
+ - * / %
& | ^ << >>
== != < <= > >=
and or not
~
is
is a
```

A symbolic infix operator may be detached from both operands (`left & right`) or right-attached to its right operand (`left &right`). In both forms, whitespace before the symbolic run prevents it from joining the left identifier. A symbolic run at the start of an expression is a prefix operator only when that behaviour is declared, as with `-einval` and `~mask`. A run attached only to its left operand is a postfix operator only when declared and is otherwise an error. A joiner-only run directly surrounded by identifier characters belongs to an operator-bearing identifier instead.

`&`, `|`, `^`, `<<`, and `>>` are the core binary bitwise operators for numeric types; `~` is the core unary bitwise-complement operator. Thus `clone-flags &clone-thread` and `clone-flags & clone-thread` have the same value semantics. In type position, `|` constructs a union and may remain compact, as in `string|none`; in value position, `left |right` and `left | right` are bitwise OR. A compact joiner-only run between identifier characters remains an operator-bearing identifier rather than an implicit bitwise expression. When a bitwise result is compared, parentheses make the intended grouping explicit and are canonical: `if (flags &mask) != 0` or its detached equivalent `if (flags & mask) != 0`.

### 17.2 Object lowering

Operators lower through object/type protocols.

The compiler may statically emit native Rust operators only where the operand types are known and the Rust operation has the same complete source contract. In particular, signed integer `/` and `%` cannot lower directly to Rust's truncating operators when the dividend may be negative; lowering must use an equivalent Euclidean operation or correction sequence. The same rule applies to overflow, shifts, and every other host/source semantic difference.

Dynamic dispatch occurs only where required by source semantics.

### 17.3 No implicit cross-type arithmetic

```text
1 + '2'
```

is an error unless a type explicitly defines that operation.

Coercion remains explicit:

```text
1 + '2'.coerce; int
```

### 17.4 Unbounded `int` arithmetic

Ordinary `int` arithmetic is exact and promotes only when required. Addition, subtraction, and unary negation first use checked operations in the current representation tier and continue in the next tier on representation overflow. Negating the value represented as `i64::MIN`, for example, produces positive `2^63` in the `i128` tier.

Multiplication uses an exact wider intermediate rather than losing the operands and retrying a source-level operation. The product of two `i64` values is computed exactly in `i128`; multiplication involving `i128` values uses an exact 256-bit/two-limb or arbitrary-precision intermediate; operations involving a big value use the arbitrary-precision backend. The result is then normalised. Implementations may specialise multiplication by `0`, `1`, and `-1` only when the same exactness and normalisation rules remain true.

Promotion is not implemented as a thrown `.arithmetic-overflow` followed by retry. It is part of the integer operation's normal runtime path. A promotion that requires storage has an allocation effect and must be transactional: compute and normalise the new value before publishing it, leave value-semantic aliases unchanged, and leave the destination unchanged if allocation fails. Allocation failure follows the ordinary allocation-failure contract, never the `.arithmetic-overflow` contract.

Bitwise operations on `int` use the mathematical infinite two's-complement model. Conceptually, nonnegative values have infinitely many leading zero bits and negative values infinitely many leading one bits; `&`, `|`, `^`, and `~` operate pointwise on that representation and return the corresponding mathematical integer. Consequently `~x == -x - 1`, `-1 & x == x`, and no finite runtime limb width is source-observable.

For `int`, `x << n` is exact multiplication by `2^n`, and `x >> n` is arithmetic right shift, equal to floor division by `2^n`, for a nonnegative `int` count `n`. A negative shift count throws `.negative-shift-count`. A count that cannot be represented by the target's indexing/allocation machinery, or a left shift whose exact result cannot be materialised, follows the ordinary resource/capability failure contract rather than wrapping the count or reporting `.arithmetic-overflow`. Right shift by a count at least the represented significant width yields `0` for nonnegative values and `-1` for negative values without requiring proportional allocation.

The implementation may perform these operations in `i64`, `i128`, or limb storage, but it must normalise the result and preserve the same value across representation tiers. Fixed-width bitwise operations instead operate on exactly `N` two's-complement bits and retain their declared type. Their shift-count policy must be selected explicitly by the fixed-width protocol and must never inherit host debug/release behaviour; it is not the unbounded-`int` rule above.

### 17.5 Division and remainder

Integer `/` and `%` use Euclidean division. For divisor `b != 0`, quotient `q` and remainder `r` satisfy:

```text
a = b * q + r
0 <= r < abs; b
```

Consequently:

```text
 7 /  3 ==  2    7 %  3 == 1
-7 /  3 == -3   -7 %  3 == 2
 7 / -3 == -2    7 % -3 == 1
-7 / -3 ==  3   -7 % -3 == 2
```

The standard integer protocol exposes `div-rem; divisor`, returning a named immutable `div-rem-result of T` with `quotient: T` and `remainder: T`, so an implementation need not divide twice. Both operands evaluate once and one backend operation is performed. A tuple is deliberately not used: named fields give a stable reflected result contract. `div-rem` exposes only its throwing default and `checked` — `wrap` and `saturate` are absent even on fixed-width receivers, because a wrapped or clamped quotient no longer satisfies the quotient/remainder identity the result object exists to guarantee. `/` selects the quotient and `%` selects the remainder. Division by zero throws `.division-by-zero` for every integer type and arithmetic mode.

For `int`, a representation minimum divided by `-1` promotes and then normalises; it is not overflow. For a signed fixed-width type, `MIN / -1` is arithmetic overflow because the mathematical quotient is outside that type.

### 17.6 Fixed-width overflow modes

Ordinary arithmetic on `int8` through `int128` and `uint8` through `uint128` is checked. Its result has the same fixed-width type, and an exact mathematical result outside that type's range throws the standard catchable `.arithmetic-overflow` error through `Result`-like control flow rather than platform unwinding. This includes addition, subtraction, multiplication, signed negation, `MIN / -1`, and any increment or decrement expressed through those operations. Unsigned negation is rejected.

Arithmetic uses the same callable-family shape as `coerce`, not a set of flat prefixed names. The families attach to `integer`:

```text
add   subtract   multiply   divide   remainder   div-rem   negate   shift-left   shift-right
```

Each family's bare invocation is its throwing default, and the operators select exactly that default child. The overflow-policy children are:

```text
value.add.checked; rhs        -> T|none
value.add.wrap; rhs           -> T          modulo 2^N, resulting bits read with destination signedness
value.add.saturate; rhs       -> T          clamped to the nearest bound
value.add.overflowing; rhs    -> overflow-result of T   with value T and overflowed bool
```

`wrap`, `saturate`, and `overflowing` attach to `fixed-integer` only. Adaptive `int` has no bounds to wrap or clamp against, so those children are absent from its type rather than being runtime no-ops; `int` exposes its throwing default always, and `checked` only where an operation is genuinely fallible — `divide`, `remainder`, and `div-rem` by zero.

For signed `MIN / -1`, `divide.wrap` returns `MIN`, `divide.saturate` returns `MAX`, and `divide.checked` returns `none`; `divide.overflowing` returns `MIN` with `overflowed = true`. Division by zero still throws `.division-by-zero` under every policy because it is not overflow, and it is never converted into a wrapped or saturated value.

Shifts accept a non-negative count. On a fixed-width receiver, the default and `checked` reject counts outside the width, and `wrap` reduces the count modulo the width; `saturate` is absent, because saturating a shift *count* has no coherent value contract. On `int`, `shift-left` is unbounded and total and `shift-right` is an arithmetic shift, with no count-policy children. Shift behaviour never inherits host-language debug/release behaviour.

Postfix `++` and `--` remain statements selecting the default `add`/`subtract` child only. A non-default policy is written as an ordinary assignment, `value = value.add.wrap; 1`.

The profiler and debugger identify the selected overflow mode in lowered Rust. An explicitly selected panic-on-overflow operation, if supplied by a package, is a panic and follows the target panic policy; it is not an ordinary core arithmetic mode.

### 17.7 Integer conversions

Cross-type integer conversion is explicit. The canonical throwing form remains:

```text
converted = value.coerce; int64
```

It returns the exact destination value when representable and otherwise throws `.integer-conversion-overflow`, a numeric error distinct from `.arithmetic-overflow`. It never silently truncates, wraps, saturates, changes signedness interpretation, or promotes the destination.

Every integer source exposes one canonical family:

```text
value.coerce.checked; T
value.coerce.wrap; T
value.coerce.saturate; T
```

`checked` returns `T|none`. `wrap` reduces the mathematical value modulo `2^N` and interprets the resulting bits using the destination signedness. `saturate` clamps to the destination bounds. Therefore `-1.coerce.wrap; uint8` is `255`, `255.coerce.wrap; int8` is `-1`, and `300.coerce.saturate; uint8` is `255`.

Conversion from any fixed-width integer to `int` is exact and cannot overflow, though it remains explicit under the no-implicit-cross-type rule. Conversion among fixed-width types follows the same checked, wrapping, or saturating contract; widening is not a privileged implicit coercion. Compile-time literal initialization remains governed by §11.2.

Wrapping and saturation have no arithmetic meaning for an unbounded `int` destination because it has no maximum width. They are defined only for an explicitly fixed-width destination or fixed-width arithmetic receiver. The flat spellings `checked-coerce`, `wrapping-coerce`, and `saturating-coerce` are not language syntax.

---

## 18. Classes, interfaces, traits, and inheritance

### 18.1 Fields

Fields are ordinary object bindings declared in class scope:

```text
class request

  method string = 'GET'
  path string = '/'
  body bytes|none = none
```

Fields are public by default and may be narrowed:

```text
private cache = .map;
protected state = none
```

### 18.2 Inheritance

Single class inheritance is supported:

```text
class secure-request extends request
```

Multiple class inheritance is not part of the core language.

The compiler may lower inheritance through generated composition, enums, trait objects, or static specialisation. Source semantics must not depend on Rust having class inheritance.

Assigning a subclass instance to a superclass-typed binding preserves the complete dynamic object and its subclass state. Subsequent value assignment copies that complete dynamic value under the ordinary COW contract; Terrane never slices to the statically named superclass fields. A superclass annotation constrains the visible interface and accepted dynamic classes, not storage layout. Targets unable to represent the permitted dynamic class set without an unavailable capability reject the boundary at compile time rather than changing this rule.

### 18.3 Interfaces

Interfaces describe required object protocols:

```text
interface serializable

  function serialize bytes
```

A class declares implementation:

```text
class message implements serializable
```

Interfaces are type objects and can be used in annotations.

### 18.4 Traits

Traits provide reusable behaviour:

```text
trait timestamped

  created-at = none

  function touch
    this.created-at = clock.now;
```

A class may use traits:

```text
class record uses timestamped
```

Trait conflicts must be resolved explicitly. No silent “last one wins” rule is permitted.

These mechanisms occupy distinct layers of one object-contract model. A **protocol** is a structural semantic operation understood by the language or libraries; any object may satisfy it without a declaration. An **interface** is a named type object collecting required protocols and method signatures for annotations and dynamic dispatch. A **trait** is reusable field/method implementation copied into a class with explicit conflict resolution; using a trait can satisfy protocols or interfaces but is not itself subtyping. **Class inheritance** extends one concrete class, preserving its state and substitutability. The iteration protocol is therefore implementable by any user class directly or through a trait, and an interface may name that requirement when a typed boundary needs it.

### 18.5 Protected visibility

`protected` exists because inheritance and extension are real use cases. It is not emulated through naming convention.

### 18.6 Overloading

The first implementation should not permit multiple declarations with the same name and signature-dispatch magic by default.

Dynamic dispatch is already available through objects and interfaces.

A multimethod/generic-dispatch facility may be supplied as a library or later language feature after its interaction with imports, reflection, and Rust monomorphisation is understood.

---

## 19. Mutation and effects

### 19.1 Mutable by default, visible by consequence

Ordinary object fields may be mutated unless the object/type contract forbids it.

The compiler infers whether a method requires mutable access to `this`.

A stricter mode may require explicit `mutating` declarations on public methods:

```text
mutating function append; value
```

This qualifier is optional in the default language.

### 19.2 No hidden global mutation

A package import must not execute arbitrary runtime mutation merely by being referenced.

Build-time importer execution and runtime initialisation are separate, visible phases.

### 19.3 Effect metadata

Functions and methods should expose inferred or declared effects through reflection:

- may throw, carrying its typed error alternatives;
- performs I/O;
- blocks;
- awaits;
- mutates receiver;
- mutates global/shared state;
- uses unsafe Rust;
- crosses FFI.

Allocation is deliberately absent from this public vocabulary. Nearly every exported function allocates, so an `allocates` annotation carries no information at an API boundary while taxing every signature that crosses one. The compiler still tracks allocation internally, and a no-allocation profile may require it to be declared where the guarantee actually matters. `blocks` is retained for the opposite reason: once async exists, a blocking callee inside async code is a defect the checker should catch.

Effect inference is permitted for private functions. Exported functions declare their public effect contract, and strict packages may require further effects to be declared.

`throws`, `async`, and other effects are part of callable type compatibility. An implementation may have fewer effects than its interface contract, never more. A dynamic callable with unknown effect metadata is treated as may-throw and otherwise unknown for capability checking rather than optimistically inferred safe.

This metadata supports optimisation, auditing, AI tooling, and target capability checks.

---

## 20. Globals and initialisation

### 20.1 Global values

Program globals are initialised before the program entrypoint.

Immutable compile-time globals should lower to native statics/constants where possible.

Dynamic initialisers execute in dependency order.

### 20.2 Dependency ordering

The compiler constructs a global-initialisation graph.

Cycles are errors unless all participating objects explicitly support lazy cyclic initialisation.

Source order is used only where no dependency relationship determines order.

### 20.3 Mutable globals

A mutable global used from multiple threads must satisfy the language’s shared-thread-safe protocol.

The compiler must not insert a mutex silently.

The engineer must select or construct an appropriate synchronised object:

```text
global cache = .shared-map;
```

or explicitly wrap one.

### 20.4 Thread-local globals

A standard thread-local object/facility should be provided rather than special-casing a second global declaration grammar.

Target profiles without threads reject it.

### 20.5 Build-time selection

`when build` selects declarations or statements from immutable build configuration:

```text
when build; config-vmap-stack
  function allocate-stack
    ...

else when build; config-thread-info-in-task
  function allocate-stack
    ...

else
  function allocate-stack
    ...
```

The predicate after `when build;` is evaluated by the compiler, never at runtime. It may inspect declared package features, target properties, capabilities, and other deterministic build inputs. It may not depend on runtime state, mutable program globals, undeclared environment state, network access, or other untracked inputs. Every input participates in dependency resolution and the incremental-build cache key.

`when build` is valid wherever its selected contents would be valid, including namespace declaration lists and function bodies. Exactly one branch of a chain is selected. Only the selected branch participates in name resolution, type checking, initialisation, code generation, and runtime reflection for that build; this permits target-specific branches to refer to APIs unavailable on other targets.

Every branch is nevertheless lexed, parsed, formatted, retained in source maps, and available to tooling. A project matrix build can require every branch to be selected and checked under at least one declared configuration. An inactive branch must never be silently treated as having been validated for the current build.

This is compile-time source selection, not an optimiser hint and not an ordinary `if`. Generated Rust must contain no runtime branch for a resolved `when build`, and diagnostics must identify the build predicate and configuration that selected the failing source.

Build-time execution has two stages in the first implementation. The bootstrap compiler loads custom importers and declaration modifiers only as precompiled, versioned host extensions implementing the compiler protocol; ordinary Terrane source is not recursively executed as an importer or modifier. `when build` evaluates a restricted constant-expression subset: literals, immutable manifest/target/capability descriptors, boolean/comparison operators, and calls to compiler-provided pure build-query objects. It cannot allocate mutable program objects, perform I/O, throw, access runtime declarations, or invoke arbitrary source functions.


Stage order is: load and validate the manifest and lockfile; load declared host extensions; process compilation-unit imports in source order; assemble namespaces; evaluate build selections; then resolve and type-check selected declarations and apply modifiers to their typed descriptors. Extension inputs and outputs are serialisable import/modifier plans included in cache keys. A future self-hosted compile-time Terrane subset would be a separate specified feature, not an accidental consequence of runtime language semantics.

---

## 21. Async and concurrency

### 21.1 Async functions

```text
async function fetch response; url string
  return await client.get; url
```

`async` marks a function whose invocation returns a task/future object.

### 21.2 Await

```text
response = await request.send;
```

`await` is control-flow syntax because suspension affects lifetime, cancellation, and diagnostics.

`await` is valid only in an `async` function or async closure. Calling an async function from synchronous code is legal and returns its task/future object; only `await` drives it to a result. An ordinary closure containing `await` is inferred async, and its invocation therefore returns a task. Callable type compatibility distinguishes synchronous from async callables.

Values live across suspension are captures of the generated task. They follow ordinary value, `ref`, move, provenance, thread-transfer, and cancellation rules. A borrow may cross suspension only when its lender is proven to outlive the task and the selected executor's movement/thread requirements are satisfied; otherwise the compiler diagnoses the capture at the `await`. An async implementation may have fewer throwing effects than declared, but cannot implement a synchronous callable contract.

### 21.3 Runtime independence

The source language should not hard-code one async executor.

A package/build profile selects the runtime implementation.

The compiler lowers async code into Rust futures and target runtime integration.

### 21.4 Structured concurrency

The structured-concurrency scope is a version-one language-level object, not a library preference. It arrives with the async callable type, the task object, and the cancellation core, because the timeout, stream-cancellation, and network-deadline contracts elsewhere in this document are all defined against it.

- child tasks belong to a parent scope;
- a scope joins its children before completing, and waits for cancellation cleanup rather than abandoning it;
- a child that throws while siblings run must have a defined effect on those siblings, and that effect is part of the scope contract;
- cancellation propagates predictably and is cooperative: cancellation points are defined, and a cancelled operation reports what it completed rather than silently discarding partial progress;
- deadlines are explicit values that additionally propagate down scope boundaries; a child inherits its parent's deadline and may shorten but never extend it. This is not ambient task-local state, because the boundary is written in the source;
- unobserved task failure is reported;
- task lifetime is visible to tracing.

Detached tasks must be explicit.

The task object's identity category, whether it is linear, and whether dropping an un-awaited task cancels it are contracts this document must fix before the async surface is implemented.

### 21.5 Sharing

Value assignment across tasks produces independent values semantically.

Shared mutable state requires `ref` plus a thread-safe object contract.

The compiler checks the source-language equivalents of Rust’s thread-transfer and shared-access requirements and reports them in source terms.

### 21.6 Channels and locks

Channels, mutexes, read/write locks, and atomics are ordinary library objects. The structured-concurrency scope is not among them: it is language-level, per §21.4.

They are not all injected into the prelude.

### 21.7 Kernel and embedded profiles

Targets without an async runtime reject or statically lower async features according to available capabilities.

An interrupt/future executor can be provided by a target package without changing source grammar.

---

## 22. Low-level and systems programming

### 22.1 Target profiles

A build selects a target profile, for example:

```text
hosted
no-std
embedded
kernel
wasm
```

Profiles define available capabilities rather than changing the basic language.

Capabilities include:

- allocator;
- threads;
- filesystem;
- sockets;
- process spawning;
- dynamic loading;
- reflection metadata;
- unwinding;
- wall clock;
- entropy;
- floating point;
- Unicode grapheme segmentation data;
- exact arbitrary-precision integer storage;
- atomics of particular widths.

### 22.2 Capability diagnostics

If source semantics require an unavailable capability, the compiler reports the source construct and the requirement:

```text
error: this value requires heap allocation

  buffer = .dynamic-list;

target:
  kernel-x86_64

available:
  stack
  static storage
  fixed-capacity collections

generated rust:
  available with --rust-errors
```

### 22.3 Dynamic language, static realisation

Dynamically typed source may compile for a kernel when the compiler can lower the used values to finite, target-compatible representations.

For example:

```text
x = 42
x = x + 1
```

does not require a dynamic runtime merely because `x` lacks an annotation.

A binding that may hold unrelated runtime types may lower to:

- a generated enum;
- a tagged stack value;
- a boxed dynamic object if an allocator exists;
- a compile error if no permitted representation exists.
Representation analysis is performed within a package compilation unit and consumes dependency semantic summaries, not dependency source bodies. Exported package boundaries have representation-independent source contracts. A dynamic exported binding or callable whose possible concrete types are not closed by that contract uses the standard erased dynamic representation and therefore requires its declared capabilities, commonly an allocator; it is never specialised from unknown future consumers.

Packages may distribute source plus summaries or profile-specific compiled artefacts. A consumer may specialise only private code or an explicitly generic/generated concrete boundary without changing the dependency's public ABI. Cache keys include the target profile, dependency summaries, and closed type sets, preserving deterministic incremental and separate compilation.

### 22.4 `no_std`

A `no-std` build uses a minimal support crate and target-provided capabilities.

Features that can be compiled away remain available. Features that require unavailable runtime support are rejected at source level.

The target capability model records whether arbitrary-precision `int` promotion and its required allocation are available. Lacking that capability does not change `int` into a bounded or wrapping type: the compiler must prove that every reachable value remains within a target-supported representation or reject the program with a capability diagnostic. Engineers selecting guaranteed bounded, allocation-free arithmetic use an explicit fixed-width integer type.

The minimal support layer includes the adaptive integer representation and its normative integer failures when core `int` first requires them. This is part of the same layered support architecture: hosted and allocation-capable targets may provide arbitrary-precision storage, while constrained targets use proof or capability rejection rather than changed integer semantics.

### 22.5 Layout and ABI

Low-level code needs explicit representation contracts.

A provisional declaration form is:

```text
layout c class packet-header
```

Additional layout qualifiers may include:

```text
packed
align
transparent
```

The exact syntax may evolve, but the compiler must support:

- C-compatible field layout;
- explicit integer widths;
- alignment;
- packing;
- endianness conversion;
- stable exported ABI;
- static size checks.

### 22.6 Pointers and memory

Raw pointers are specialised objects or explicit Rust values, not ambient behaviour.

A safe wrapper may look like:

```text
pointer = .pointer; address, type=int32
value = pointer.read;
```

Operations involving arbitrary addresses, aliasing violations, volatile memory, or unchecked lifetime must occur inside an `unsafe` boundary or inline unsafe Rust.

### 22.7 Volatile and atomic access

Volatile and atomic operations are explicit types/protocols.

Normal assignment must not silently become volatile or atomic merely because a value happens to point at device memory.

### 22.8 Unsafe blocks

```text
unsafe
  ...
```

marks source operations whose safety contract cannot be verified by the normal compiler model.

Inline Rust has its own `unsafe rust` form.

Unsafe usage is recorded in reflection, build reports, diagnostics, and tracing metadata.

### 22.9 Deterministic resource management

Rust lowering provides RAII-like deterministic cleanup.

Files, locks, mappings, device handles, and other resources should not require a garbage collector or universal context-manager syntax.

Resource classes may be linear where copying is nonsensical.

---

## 23. Packages and dependencies

### 23.1 Four dependency origins

The package system supports:

1. native Terrane packages;
2. Rust crates;
3. system libraries, ordinarily exposed through C ABI metadata or a wrapper;
4. foreign-runtime packages hosted through an explicit runtime adapter.

Example manifest/source declarations:

```text
use image-tools
use rust serde
use system libjpeg
use runtime python
```

A native package is the default dependency kind.

### 23.2 `use` versus `from ... import`

`use` declares a build dependency.

`from ... import` brings object symbols from an available namespace into source scope.

```text
use image-tools

from /image tools import .resize
```

The distinction is intentional:

- dependency graph composition is not the same operation as name binding;
- installing a package must not automatically pollute source names.

### 23.3 Package contents

A package may contain:

```text
source/
rust/
c/
headers/
tests/
package manifest
```

The first-version authored manifest is `package.toml`. It is a TOML document with
the following minimal contract:

```toml
package = "example.tools"
prelude = true
sources = ["src/main.trn", "src/support.trn"]
```

`package` is a required non-empty package identity. `sources` is a required,
non-empty array that enumerates the complete set of relative `.trn` source
paths; absolute paths and paths containing `..` are invalid. Duplicate paths are
an error. `prelude` is an optional boolean and defaults to `true`. Unknown fields
are rejected. Source units receive stable file identities in sorted path order,
independent of the array order. A single `.trn` CLI input is instead an
implicit one-unit package with identity `single-file` and the default prelude.

A package may expose one coherent object namespace regardless of which implementation language supplies each object.

Consumers should not need to know whether `.resize` is implemented in source, generated Rust, handwritten Rust, or a C library wrapper.

### 23.4 Locking and reproducibility

The package manager must produce a lockfile covering:

- native package versions and content hashes;
- Rust crate versions, features, and checksums;
- system library constraints and resolved ABI metadata;
- foreign runtime adapter, runtime ABI, interpreter, and package constraints;
- compiler version;
- importer version;
- target profile;
- generated binding versions;
- build-time capabilities and material inputs.

System packages are not inherently reproducible merely because their name is locked. Production builds should record the actual library version, ABI, headers/binding hash, and linker identity.

### 23.5 Generated Cargo project

The language compiler owns the generated Cargo manifests for ordinary projects.

It resolves:

- crate dependencies;
- features;
- target-specific dependencies;
- build profiles;
- link directives;
- native modules;
- support runtime versions.

Users may inspect the generated `Cargo.toml`.

They should not normally need to maintain it separately unless a project deliberately takes ownership of the Rust layer.

### 23.6 Build scripts

Declarative build metadata is preferred.

Arbitrary build scripts are powerful and therefore capability-gated.

The build report must identify packages that executed code during compilation.

### 23.7 Rust crates

A Rust crate dependency is declared with:

```text
use rust crate-name
```

A native wrapper may expose its API as language objects.

Direct crate access is also permitted through generated adapters when the Rust API can be represented safely.

Rust generics may be translated only into concrete generated instantiations or erased interface/object boundaries under §11.8; Rust traits, lifetimes, and errors map into the language model where their contracts remain representable. APIs that cannot be represented cleanly require a handwritten or generated Rust wrapper.

### 23.8 System and C libraries

```text
use system libjpeg
```

declares a system dependency.

The build layer may use platform adapters such as package metadata, SDK discovery, toolchain files, or explicitly configured paths.

C integration requires:

- ABI declarations or headers;
- generated or maintained bindings;
- linker metadata;
- ownership/error contracts;
- safe wrapper objects where appropriate.

Raw C calls are unsafe unless proven safe by a wrapper contract.

### 23.9 C++

Arbitrary C++ ABI integration is not a version-one goal.

C++ libraries should initially be consumed through:

- an existing C API;
- a small C-compatible shim;
- a handwritten Rust bridge.

### 23.10 Exporting back to Rust and C

Language packages should be able to expose stable Rust and C APIs.

The compiler generates:

- Rust modules/types/functions;
- C ABI wrappers where requested;
- headers;
- ownership and error conventions;
- symbol metadata;
- versioned ABI descriptors.

This makes the language embeddable rather than a one-way consumer.

### 23.11 Native interop versus foreign runtimes

Rust is Terrane’s canonical lowering language. Inline Rust and maintained Rust modules inhabit the generated program and may use its documented native representations directly. System/C libraries cross an ABI boundary but do not introduce another language runtime.

A foreign runtime is different:

```text
use runtime python
```

declares that the program hosts a subordinate runtime with its own object model, allocator or garbage collector, exceptions, module loader, concurrency rules, and deployment requirements.

The distinction is constitutional:

- `rust` is a native lowering escape hatch;
- `python` is foreign-runtime execution;
- a runtime adapter must not silently replace Terrane typing, assignment, error, thread, or ownership semantics with the foreign language’s semantics.

Python is the first foreign runtime and the first adapter implementation. The initial adapter targets the CPython `libpython3` embedding API. Other adapters, such as Lua or JavaScript, may be added later through the same contracts; they are not version-one requirements.

### 23.12 Runtime imports and Python objects

After declaring the runtime dependency, Python modules may expose object-form bindings:

```text
use runtime python
from python numpy import .array

values = .array; 1, 2, 3, 4
mean = values.mean;
```

`from python ...` performs Python module resolution through the selected adapter and binds foreign object proxies in the object-form namespace. Attribute lookup and invocation use ordinary Terrane member syntax, but the semantic descriptor records a foreign transition.

A Python object proxy is a Terrane object whose implementation, mutable identity, and lifetime belong to CPython. Reflection must identify that fact rather than presenting it as a native value:

```text
foreign
runtime python
foreign-type numpy.ndarray
```

Foreign proxies are identity-bearing resources, not ordinary COW values. They cannot be value-assigned unless an adapter exposes a specific value-copy contract. Sharing one requires explicit `ref`; transferring an exclusive proxy uses `move`. Calls may borrow a proxy without transferring it. This prevents Python aliasing from silently weakening Terrane’s ordinary assignment rule.

An adapter may expose a native Terrane wrapper with normal COW semantics when it can genuinely preserve those semantics—for example, through a verified immutable value or buffer-backed representation.

### 23.13 Embedded foreign source

An indented runtime block executes foreign source:

```text
python
  import numpy as np

  x = np.array([1, 2, 3])
  print(x.mean())
```

The compiler preserves and source-maps the block, while the Python adapter compiles and executes it through `libpython3`. Values enter or leave only through an explicit adapter interface; lexical bindings are not implicitly shared with the block.

Unlike inline Rust, an embedded Python block is never inserted into generated Rust as native code. Tooling must label it as foreign runtime execution and account for every transition.

### 23.14 Conversion and zero-copy data

The Python adapter may convert scalars with direct, documented mappings:

```text
int
float
bool
string
bytes
none
```

Conversion is not coercion merely because it appears obvious. Runtime calls perform only conversions declared by the adapter and visible through reflection. Collections default to explicit conversion because ownership, mutability, shape, and copying costs matter:

```text
py-values = values.coerce; python.list
```

Large data must have standard zero-copy paths where representation and lifetime permit them. The Python adapter should support the Python buffer protocol first and may add DLPack and Arrow adapters. A zero-copy bridge must pin or otherwise preserve the producer’s storage, declare mutability and element layout, and reject incompatible lifetimes rather than copying silently.

Build explanations and profiling must report whether a boundary conversion borrowed, wrapped, pinned, or copied data.

### 23.15 Errors, lifetime, and threads

A Python exception becomes a Terrane `.python-error` preserving:

- the Python exception type and message;
- the formatted Python traceback;
- the original Python exception object while its runtime remains alive;
- the Terrane source location and boundary operation;
- a causal chain when wrapped or rethrown.

```text
try
  result = python-object.do-thing;

catch .python-error as error
  print; error.message
  print; error.python-trace
```

Crossing the boundary may acquire the CPython GIL, allocate in Python’s managed heap, execute arbitrary Python code, and trigger Python finalisers. The adapter owns reference-count transitions and interpreter shutdown ordering. Foreign finalisation must not be described as deterministic Terrane destruction when CPython cannot provide that guarantee.

The compiler and runtime must reject unsupported cross-thread use rather than silently adding locks or moving a proxy between interpreters.

### 23.16 Runtime adapter contract

Every foreign runtime adapter defines:

- runtime discovery, initialisation, selection, and shutdown;
- module loading and package resolution;
- proxy object representation and lifetime;
- attribute lookup, invocation, and reflection;
- scalar and collection conversion;
- zero-copy buffer protocols where supported;
- exception and traceback translation;
- thread, lock, and re-entry rules;
- debugger, profiler, and source-map integration;
- deployment and capability metadata.

Adapters expose these behaviours through Terrane’s object and binding model. They do not create a universal multi-language VM, and they do not make foreign semantics the defaults for native Terrane code.

### 23.17 Deployment contract

`use runtime python` adds an explicit runtime dependency. It does not preserve the pure Terrane/Rust guarantee that no language runtime is needed in production.

The build report and lockfile must identify at least:

```text
runtime python
abi libpython3
interpreter constraint
python packages
adapter version
link or bundle strategy
```

The default hosted strategy may discover and link a compatible system `libpython3`. A deployment profile may instead bundle CPython and its selected packages. Neither choice may be silent, and a build must fail when its locked ABI or package requirements cannot be satisfied.

Allocator-free, firmware, kernel, and similarly constrained profiles reject foreign runtimes unless a target-specific adapter explicitly proves support.

---

## 24. Handwritten Rust

### 24.1 Why it is first-class

Rust is already the generated language. Dropping into Rust is therefore not a foreign-runtime transition.

The escape path is:

```text
high-level source
  -> inline rust
  -> maintained rust module
```

Callers need not change as an implementation moves down that path.

### 24.2 Inline Rust statement block

```text
function checksum uint64; data bytes
  rust
    checksum_impl(data)
```

The indented block is preserved as Rust after stripping its common source indentation.

The compiler inserts it into the generated Rust function and maps its spans back to the source block.

### 24.3 Inline Rust expression

A Rust block used as an expression returns its final Rust expression:

```text
result int = rust
  native_calculation()
```

The compiler checks that the Rust result can cross back into the declared/source object type.

### 24.4 Inline Rust in classes

```text
class fast-buffer

  function checksum uint64
    rust
      self.inner.checksum()
```

The compiler exposes a documented Rust representation for `this` and in-scope values.

A class may also contain a larger Rust implementation block for generated impl items, subject to explicit contracts.

### 24.5 Safe and unsafe Rust

`rust` accepts safe Rust.

```text
unsafe rust
  ...
```

permits unsafe Rust and records the unsafe boundary.

Writing `unsafe` inside a nominally safe raw block does not bypass source-level accounting; the compiler scans/parses the Rust block sufficiently to classify it or delegates classification to `rustc` metadata.

### 24.6 Name mapping

Source identifiers are represented internally by their exact source spelling and lexical scope. Punctuation is never deleted, word-substituted, or normalised, so `foo+bar`, `foobar`, and `fooplusbar` are three unrelated symbols.

Generated Rust uses a deterministic, injective encoding. A suitable canonical scheme prefixes the name with `__terrane_`, preserves ASCII letters and digits, and encodes every other UTF-8 byte as `_xHH_`; underscore itself is encoded if it becomes legal in source identifiers. For example:

```text
my-value    -> __terrane_my_x2d_value
foo+bar     -> __terrane_foo_x2b_bar
ipv4/ipv6   -> __terrane_ipv4_x2f_ipv6
```

No two distinct source spellings may produce the same encoded spelling. Scope/module identity is represented separately and deterministically where Rust requires further disambiguation; it must never repair a lossy spelling conversion with an arbitrary suffix.

The debugger and `terrane rust-name` tooling expose both directions of the mapping. Inline Rust uses the generated Rust names, with editor tooling able to complete and display the originating source names.

A later interpolation syntax may permit direct source-name references, but it is not required for the first implementation.

### 24.7 Full Rust files

A project may include maintained `.rs` files as native modules.

The package manifest associates them with generated crate modules and exported language objects.

A companion declaration or Rust attribute exposes public objects through the language ABI.

The exact annotation syntax may evolve, but the contract must cover:

- exported object/type identity;
- default invocation;
- methods;
- ownership;
- value-assignment/COW/ref behaviour;
- errors;
- thread safety;
- reflection metadata;
- target capabilities.

### 24.8 No FFI cliff

Calls between generated and handwritten Rust occur within the same Rust crate graph whenever possible.

There is no C-style FFI boundary merely because one function was handwritten.

### 24.9 Ejecting generated Rust

Tooling should support:

```text
terrane eject-rust /image codec resize
```

This copies a generated implementation into a maintained native Rust module, adds the appropriate bridge metadata, and replaces source generation for that object.

The operation must be explicit, reviewable, and reversible only through source control or a deliberate migration.

---

## 25. Reflection

### 25.1 Reflection is a core service

A language in which everything is an object requires a coherent way to inspect those objects.

Reflection should be accessed through a normal object:

```text
info = reflect; value
```

`reflect` may be rebound like other prelude objects, while the underlying compiler/runtime reflection service remains available from an explicit core namespace.

### 25.2 Semantic reflection

A reflection descriptor should expose, where applicable:

```text
info.name
info.type
info.namespace
info.package
info.visibility
info.members
info.methods
info.fields
info.interfaces
info.traits
info.parent
info.callable
info.constructible
info.arguments
info.options
info.return-type
info.effects
info.source
info.documentation
info.foreign
info.runtime
info.foreign-type
info.foreign-identity
info.conversion-contracts
```

### 25.3 Compilation reflection

For functions, methods, classes, and live frames, reflection may also expose:

```text
info.name
info.compile.rust
info.compile.rust-name
info.compile.rust-type
info.compile.source-map
info.compile.target
info.compile.profile
info.compile.optimised
info.compile.size
info.compile.alignment
info.compile.allocations
info.compile.dynamic-dispatch
info.native.symbol
```

The source name, generated Rust name, and external native symbol are three independent identifiers. Source-to-Rust encoding is deterministic and injective; an external symbol is never inferred by lossy normalisation. A declaration that must expose or bind an exact ABI spelling records it explicitly through parameterised compile-time metadata, for example `native-name; mmdrop, '__mmdrop'`. All three names and the metadata operation that established them remain visible to reflection and source maps.

This is one of the language’s defining features.

### 25.4 Asking for generated Rust at runtime

In a development build:

```text
info = reflect; my-function
print; info.compile.rust
```

returns the generated Rust corresponding to that build.

A live frame may be inspected:

```text
frame = debug.current-frame;
print; frame.compile.rust
```

This answers “what is this invocation actually executing?” even when generic specialisation or target configuration matters.

### 25.5 Metadata levels

Reflection metadata must be selectable:

```text
none
names
semantic
full
```

A hosted development build will normally use `full`.

A release build may:

- embed compressed metadata and Rust source;
- ship a signed sidecar;
- retain only names and source maps;
- strip reflection entirely where permitted.

Kernel and embedded builds commonly use `none` or `names`.

A reflection query for stripped information returns an explicit unavailable result, not fabricated data.

### 25.6 Reflection mutation

Ordinary reflection is read-only.

Mutating private fields through reflection, replacing methods, or altering class layout at runtime would constrain optimisation and safety severely.

Such behaviour, if ever supported, must occur through:

- an explicit mutable-reflection capability;
- an unsafe boundary;
- a dynamic-hosted profile;
- clear loss of static guarantees.

It is not core version-one behaviour.

### 25.7 Runtime representation is not semantic identity

Reflection must distinguish:

- source type;
- source object identity;
- generated Rust type;
- physical storage representation.

A source `int` remains an `int` whether realised as an unboxed `i64`, a specialised `i128`, the adaptive wrapper's wide tier, or arbitrary-precision limb storage. Reflection reports `int`; only explicit compilation/profiling reflection may expose the current physical tier.

---

## 26. Debugging, tracing, and performance as first-class facilities

### 26.1 Stable identity through the toolchain

Every meaningful source construct receives a stable compiler identity carried through:

```text
source node
  -> resolved object/binding
  -> generated Rust span
  -> native symbol/debug location
  -> trace and allocation site
```

The identity should be stable across builds when the semantic source construct remains unchanged, subject to compiler-versioned rules.

### 26.2 Source-level debugger

The built-in debugger presents source-language concepts:

- namespaces;
- functions and methods;
- objects and fields;
- dynamic and constrained bindings;
- value versus reference identity;
- copy-on-write state;
- tasks;
- thrown errors;
- source stack frames;
- foreign proxies, runtime ownership, and lock state;
- foreign-runtime stack frames and transitions.

Rust/native and foreign-runtime details are expandable rather than hidden.

### 26.3 Value inspection

A debugger view may report:

```text
buffer

source type       bytes
binding           local
binding contract  dynamic
identity          value
physical storage  shared copy-on-write
strong refs       2
weak refs         0
size              8.2 mb
rust type          CowBytes
```

The exact Rust type is supplementary; source semantics come first.

### 26.4 Source-level stepping

Stepping should follow source statements and expressions, not generated helper functions.

The debugger uses source maps and custom debug metadata to collapse generated frames.

An “enter Rust” action permits stepping into generated or handwritten Rust when desired.

An “enter runtime” action permits stepping from a Terrane boundary into embedded foreign source or an available foreign debugger. If an adapter cannot provide statement-level stepping, tooling must say so rather than presenting a native call as foreign source execution.

### 26.5 Tracing

Compiler-supported tracepoints should cover:

- function/method entry and exit;
- errors and catches;
- async task creation, suspension, wake, cancellation, and completion;
- I/O operations;
- locks and waits;
- allocation sites;
- value assignments;
- physical copies;
- copy-on-write splits;
- refs and weak refs;
- moves;
- native FFI calls;
- foreign-runtime entry/exit, conversions, copies, lock acquisition, and exceptions;
- unsafe blocks.

Tracing is feature/profile controlled and may be sampled.

### 26.6 Profiling

The profiler should report source-level metrics such as:

```text
request-handler

calls                  12,481
wall time              842 ms
cpu time               611 ms
self cpu               311 ms
allocations            42,190
bytes allocated        18.4 mb
semantic assignments  128,402
physical copies         1,931
cow splits                417
refs created             8,441
lock wait                29 ms
foreign transitions          418
foreign boundary time       21 ms
foreign data copied       8.2 mb
```

### 26.7 Causal performance explanation

The toolchain should connect cost to source semantics:

```text
unexpected cost:
  buffer was physically copied 14,284 times

source:
  result = buffer

reason:
  result escaped the copy-on-write region through a C ABI call

possible actions:
  avoid repeated value assignment inside the loop
  pass a read-only ref
  use a Rust wrapper accepting a borrowed slice
```

This is more valuable than merely producing a flame graph.

### 26.8 Build-time cost reports

A build may request:

```text
terrane explain /image codec resize
```

and receive:

- inferred source types;
- generated Rust types;
- stack versus heap placement;
- static versus dynamic dispatch;
- allocations;
- physical copies and COW splits;
- synchronisation;
- FFI boundaries;
- required capabilities;
- generated Rust location.

### 26.9 Production observability

Production builds may retain low-overhead stable trace IDs without embedding full source.

A symbol/source sidecar can decode events later.

Sensitive values must not be captured by default merely because tracing exists.

### 26.10 Time-travel and replay

Deterministic replay is a plausible later capability because the compiler owns object, task, and effect instrumentation.

It is not required for the first implementation, but stable event identities and effect metadata should avoid foreclosing it.

---


## 27. Compiler architecture

### 27.1 The public compilation pipeline

```text
source
  -> lexer and indentation parser
  -> resolved semantic model
  -> generated Rust
  -> Cargo/rustc
  -> artefact
```

The semantic model is transient compiler machinery.

Generated Rust is the public lowered representation.

### 27.2 Frontend phases

A practical compiler performs:

1. UTF-8 decoding and indentation tokenisation;
2. lexical analysis;
3. parsing into a lossless syntax tree;
4. namespace and object-form import resolution;
5. ordinary binding resolution;
6. class/interface/trait resolution;
7. type, capability, ownership, copy, ref, and effect analysis;
8. lowering decisions;
9. Rust source emission;
10. source-map emission;
11. Cargo graph generation;
12. Rust compilation;
13. diagnostic translation;
14. debug, reflection, and trace metadata generation.

A lossless syntax tree is useful for formatting, comments, refactoring, and IDE support. A smaller semantic tree may be used for lowering.

Neither is a user-visible canonical IR.

### 27.3 Compiler implementation language

The first compiler frontend should be implemented in Rust.

Rust provides one distributable toolchain executable, precise and exhaustively checked compiler phase models, and direct integration with generated Cargo projects, structured rustc diagnostics, source maps, and any future support crates.

Mature parser tooling should be evaluated rather than assuming that a Rust implementation requires every frontend component to be handwritten. A parser-combinator library such as Chumsky may provide token parsing, spans, recursive grammars, Pratt expression parsing, rich errors, and recovery. Terrane's token, syntax, span, and diagnostic models remain compiler-owned so a library can be replaced or selectively bypassed without changing language semantics.

The hardest whitespace-sensitive and operator-attachment cases must be prototyped before the parser architecture is frozen. A narrow handwritten lexer remains appropriate if indentation, tail/block strings, or attached-operator rules are clearer there.

The runtime characteristics of compiled programs do not depend on the frontend implementation language.

### 27.4 Generated crate graph

The compiler generates a normal Rust workspace or crate graph containing:

```text
generated application crates
generated package crates
handwritten rust modules
runtime support crates
ffi wrapper crates
target support crates
```

The mapping from source namespace/package to Rust module/crate must be deterministic and inspectable.

### 27.5 Runtime support library

The support runtime should be layered and pay-for-use.

Possible components include:

- the adaptive exact `int` representation, its arithmetic, and its normative integer failures;
- dynamic `Value` representation;
- type/object descriptors;
- callable/default-invocation adapters;
- copy-on-write collections;
- reference and weak-reference support;
- throw/error propagation;
- reflection registry;
- trace event support;
- source identity tables;
- package ABI adapters.

A program that needs only statically lowered scalars and functions should not drag in the entire hosted dynamic runtime.

### 27.6 Dynamic value lowering

The compiler chooses the narrowest representation preserving source semantics.

Examples:

**Known and potentially widening integers**

```text
x = 42
x = x + 1
```

may lower directly to `i64` where range analysis proves the fast representation sufficient. Where runtime widening is possible, generated code uses an `i64` hot path with a cold exact-promotion path into the `i128` tier and then arbitrary-precision storage. It must not model promotion as a source throw or re-evaluate operands after detecting representation overflow.

The erased `int` representation keeps the small case compact and boxes or otherwise out-of-lines wider payloads rather than imposing an inline `i128` size on every value. Arithmetic helpers normalise completed results back through `i128` to `i64` whenever exact bounds permit. Equality, ordering, and hashing operate on the mathematical value across all tiers and must produce identical answers for equal values reached through different representations.

**Known finite alternatives**

```text
if condition
  x = 42
else
  x = 'unknown'
```

may lower to a generated enum.

**Open dynamic value**

A value crossing an open plugin/reflection boundary may lower to a boxed/tagged dynamic object.

**Typed contract**

```text
x float = 0.5
```

should lower directly to `f64` unless reflection or ABI requirements force otherwise.

### 27.7 Class lowering

A source class may lower to:

- a Rust struct;
- an enum;
- a trait plus concrete structs;
- a value/COW wrapper;
- a reference-backed state object;
- a linear native resource;
- a dynamic object implementation.

The representation is not source-observable except through explicit compilation reflection.

### 27.8 Invocation lowering

Calls should be statically dispatched wherever the receiver is known.

Dynamic default invocation uses generated callable traits/tables only where needed.

The source expressions:

```text
message = ' '.concat; a, b, c
print; message
```

have a stable semantic lowering:

```text
concat-member = member-lookup ' ', concat
message = default-invoke concat-member with:
  receiver: ' '
  arguments: a, b, c

result = default-invoke print with:
  arguments: message
```

The emitted Rust may inline, monomorphise, or eliminate either object when behaviour remains identical.

### 27.9 Exceptions

The compiler transforms source throws/catches into explicit generated Rust control flow.

It may use a generic source error object at dynamic boundaries and concrete Rust error enums in statically known regions.

This allows application-level exception ergonomics without using panic as normal control flow.

### 27.10 Ownership analysis

The compiler analyses:

- value assignment;
- COW opportunities;
- explicit refs;
- weak refs;
- moves;
- closure capture;
- task crossing;
- FFI crossing;
- resource drop;
- reflection escape.

It should prefer ordinary Rust ownership and borrowing before allocating reference-counted wrappers.

### 27.11 No hidden semantic repair

The compiler must not “make code work” by silently:

- adding locks;
- changing value assignment into reference sharing;
- copying a linear resource;
- converting a throw into panic;
- coercing unrelated scalar types;
- switching a relative import to root;
- retaining reflection metadata a target forbids.

It should report the source-level conflict and available explicit choices.

---

## 28. Generated Rust contract

### 28.1 Readability

Generated Rust is intended to be read by:

- humans;
- AI coding models;
- `rustfmt`;
- `clippy`;
- profilers;
- debuggers;
- security scanners;
- ordinary Rust tooling.

It should avoid deliberately opaque macro expansion when straightforward Rust can express the same semantics.

### 28.2 Determinism

For the same:

- source;
- dependency lock;
- compiler version;
- target;
- profile;
- feature set;
- importer inputs;

the generated Rust must be deterministically equivalent and should be byte-identical after canonical formatting.

Stable generation provides meaningful diffs and reproducible builds.

### 28.3 Build artefact layout

A default build tree may be:

```text
build/
  semantic/
  rust/
    Cargo.toml
    src/
  maps/
  diagnostics/
  metadata/
  target/
```

The exact directory names are not semantic.

Generated Rust must be easy to locate by source namespace/object.

### 28.4 Source comments

Generated units should include compact comments identifying:

- source package;
- namespace;
- object/function;
- source span;
- compiler node identity;
- generation profile.

Comments are supplementary to machine-readable source maps.

### 28.5 Formatting

Generated Rust is passed through a pinned/canonical formatter configuration.

Formatting is part of deterministic output.

### 28.6 Editing policy

Generated Rust is read-only from the language toolchain’s perspective.

Manual changes may be overwritten.

The correct options are:

- change source;
- change compiler lowering;
- use inline Rust;
- add a maintained Rust module;
- eject a generated unit.

### 28.7 Rust validation

The build may run:

- `cargo check`;
- tests;
- linting;
- target-specific static analysis;
- unsafe audit checks.

Findings are mapped back to source where possible.

### 28.8 Build identity

Every binary records or accompanies:

- source build hash;
- compiler version;
- generated Rust hash;
- dependency lock hash;
- source-map identity;
- reflection/trace metadata identity.

This permits runtime traces and crash reports to resolve to the exact generated Rust.

---

## 29. Source maps and diagnostic translation

### 29.1 Bidirectional maps

The compiler emits bidirectional mappings among:

```text
source span
semantic node
object/binding identity
generated Rust span or spans
native symbol
trace/allocation identity
```

A source expression may map to multiple Rust spans.

A Rust helper span may map back to the semantic operation that caused it.

### 29.2 Rust diagnostic collection

Cargo and `rustc` are invoked with structured diagnostic output.

The language compiler collects:

- primary spans;
- secondary spans;
- error codes;
- notes;
- suggestions;
- macro/backtrace information where relevant;
- target/toolchain messages.

### 29.3 Returning Rust errors to source

A Rust error should normally be shown in source terms.

A generated borrow/move error might become:

```text
error: buffer is no longer available here

  42 | request.send; move buffer
  43 | log; buffer
             ^^^^^^

buffer ownership was transferred on line 42

generated rust:
  build/rust/network_client.rs:428

rust diagnostic:
  available with --rust-errors
```

A trait error might become:

```text
error: this object cannot cross a task boundary

value:
  cache

reason:
  cache permits shared mutation but does not implement the
  thread-safe sharing protocol

possible actions:
  use a shared-map
  keep the task on one thread
  pass a value copy
```

### 29.4 Raw diagnostics remain available

Translation must not discard the original compiler information.

Commands and flags should include:

```text
terrane check --rust-errors
terrane explain-error error-id
terrane rust /namespace function
```

An experienced engineer or AI agent can inspect the raw Rust evidence.

### 29.5 Inline Rust errors

Diagnostics originating inside an inline Rust block map directly to that source block and retain normal Rust wording where it is already the clearest explanation.

The surrounding source-language type/ownership context is added as notes.

### 29.6 Diagnostic translation strategy

Translation combines:

- source-map projection;
- semantic-node knowledge;
- known Rust diagnostic patterns;
- object/type/capability metadata;
- fallback presentation of raw Rust diagnostics.

The translator should be versioned and tested independently from code generation.

A failed high-level translation is not a failed build diagnostic; the raw Rust error remains a trustworthy fallback.

---

## 30. Development and deployment workflow

### 30.1 Transparent development compilation

The normal workflow is:

```text
terrane run
terrane test
terrane dev
terrane check
```

These commands transparently:

- detect changed source;
- regenerate affected Rust;
- reuse cached generated modules;
- invoke incremental Cargo/rustc;
- run or restart the target;
- map errors to source.

Compilation is real, but ordinary development should not require manually operating Cargo.

### 30.2 Compiler daemon

`terrane dev` may run a resident compiler service retaining:

- parsed syntax trees;
- resolved namespace graphs;
- dependency graph;
- inferred types/effects;
- generated Rust fragments;
- source maps;
- Cargo incremental state;
- running process/debug connection.

This provides dynamic-language-like edit/run ergonomics without inventing a VM.

### 30.3 Restart and reload

A hosted development service may restart automatically after successful compilation.

Hot code replacement is optional and must not be faked. Stateful reload requires explicit object migration semantics and is not core version-one behaviour.

### 30.4 Production builds

Production uses an explicit build:

```text
terrane build --release
```

The deployed artefact is normally:

- a native executable;
- a native library;
- a container image containing the compiled artefact;
- firmware/kernel/wasm output.

For a pure Terrane/Rust program, the production target does not require:

- source files;
- the Terrane compiler;
- Cargo;
- `rustc`;
- dynamic recompilation;
- a language VM.

A declared foreign runtime remains a production dependency. The build report must distinguish system-linked, bundled, and externally provided runtimes and packages.

### 30.5 Containers

A normal container build is multi-stage:

```text
source and compiler
  -> generated rust
  -> release binary
  -> minimal runtime image
  -> declared foreign runtimes and packages, if any
```

The language toolchain belongs in the builder stage, not the runtime image.

### 30.6 Compilation transparency

The default CLI should be quiet enough for ordinary use but precise when work occurs:

```text
changed:
  /api users

generated:
  build/rust/api/users.rs

compiled:
  api

running:
  localhost:8080
```

Machine-readable output is always available.

### 30.7 Cache correctness

Compiler and Cargo caches are content-addressed by all semantically relevant inputs.

The compiler must never reuse generated Rust after an importer, target capability, package feature, inline Rust unit, foreign runtime adapter or ABI, or strictness mode changes without including that change in the cache key.

---

## 31. Tooling

### 31.1 Required first-party tools

A serious first release needs:

```text
terrane fmt
terrane check
terrane build
terrane run
terrane test
terrane dev
terrane rust
terrane rust-name
terrane explain
terrane explain-error
terrane debug
terrane trace
terrane profile
terrane package
```

### 31.2 Formatter

The formatter is essential because whitespace around dots and infix operators is semantic.

It must preserve and visually regularise:

```text
print.concat
print; .concat
foo+bar
foo + bar
count - 1
```

It must canonicalise every parsed infix expression to one space around its operator and must never insert spaces inside an identifier. One-sided operator spacing is rejected rather than guessed. Formatting `x=foo+bar` produces `x = foo+bar`; `x=count-1` is rejected and may be fixed explicitly to `x = count - 1`.

The formatter must reject or loudly expose ambiguous/non-canonical spacing.

### 31.3 Language server

The language server should expose:

- completion for ordinary and dot-object scopes;
- namespace path resolution;
- inferred and declared types;
- value/ref/move consequences;
- generated Rust preview;
- diagnostics;
- references and renames;
- import object provenance;
- effects/capabilities;
- source-to-Rust navigation;
- Rust-to-source navigation.
- exact source-to-generated identifier mappings;
- token classification for operator-bearing identifiers;
- a targeted unknown-name diagnostic that may suggest `foo + bar` when unresolved `foo+bar` appears, without silently rewriting it.

### 31.4 Documentation generation

Because objects, functions, methods, classes, packages, and namespaces share reflection metadata, documentation should be generated from the same semantic descriptors.

Docs should identify whether an API is implemented in source, generated Rust, handwritten Rust, or C, without making that implementation origin part of ordinary call syntax.

### 31.5 Testing

Testing is ordinary source code plus a standard test object/framework.

The compiler should also support compile-pass and compile-fail tests with expected source diagnostics.

### 31.6 Conformance suite

The language needs a public conformance corpus covering:

- lexical and indentation edge cases;
- dot whitespace distinctions;
- namespace anchoring;
- importer replacement;
- type/coercion behaviour;
- value/ref/move behaviour;
- COW observability and recursive separation;
- error propagation;
- Rust lowering snapshots;
- source-map accuracy;
- diagnostic translation;
- hosted/no-std target differences;
- package and FFI boundaries.

Generated Rust snapshots are useful but semantic execution tests remain authoritative.

The conformance suite should contain many minimal Terrane snippets, each isolating one lexical, syntactic, semantic, or lowering decision. Where lowering is expected to succeed, the case should be able to assert canonical generated Rust byte for byte. This turns the public lowered representation into a precise, reviewable compiler contract and makes broad coverage inexpensive.

Not every minimal snapshot needs its own Cargo invocation. The harness may combine independent accepted snippets into deterministic generated crates for batched `cargo check`, while cases whose contract depends on crate structure, linking, diagnostics, or runtime behaviour remain individually compiled or executed. Snapshot agreement proves what the compiler emitted; Rust compilation proves that emission is valid; selected execution tests remain the authority for observable language semantics.

### 31.7 Fuzzing

The lexer/parser, importer request decoder, source-map mapper, and diagnostic translator should be fuzzed early.

Whitespace-sensitive dot syntax deserves dedicated mutation tests.

---

## 32. AI and agent support

### 32.1 Generated Rust as the model’s semantic escape hatch

A coding agent can be instructed:

```text
when language behaviour or performance is unclear:

1. inspect the generated rust
2. inspect the source-to-rust mapping
3. treat generated rust as the authoritative lowered semantics
4. make ordinary fixes in source, not generated rust
5. use inline or maintained rust only when intentionally dropping a layer
```

This belongs naturally in `AGENTS.md`.

### 32.2 Machine-readable compiler interface

Every important command must offer structured output:

```text
terrane check --json
terrane lower --json
terrane explain --json
terrane profile --json
terrane trace --json
```

Records should include:

- source span;
- semantic node ID;
- resolved object;
- inferred/declared type;
- effects;
- generated Rust spans;
- original Rust diagnostic;
- translated diagnostic;
- suggested source fixes;
- allocation/copy/ref facts;
- build identity.

### 32.3 Stable navigation

An agent should be able to request:

```text
source object -> generated rust
generated rust span -> source object
runtime frame -> source and rust
trace event -> source and rust
```

without searching the build tree heuristically.

### 32.4 Rust ecosystem tools remain useful

AI can inspect:

- generated Rust;
- native Rust modules;
- Cargo metadata;
- Rust lints;
- tests;
- profiler output.

The new language benefits immediately from models’ existing Rust competence.

### 32.5 Avoid generated-code edits

Tooling should mark generated Rust as read-only and return an actionable diagnostic when an agent attempts to patch it.

A suggested path should point to:

- originating source;
- inline Rust escape hatch;
- eject command;
- compiler codegen issue.

### 32.6 Agent-friendly diagnostics

Diagnostics should state semantic consequences, not merely parser tokens.

For invalid dot adjacency:

```text
error: whitespace does not invoke print

did you mean:
  print.concat
to select print's concat member?

or:
  print; (.concat; value)
to invoke .concat and pass its result to print?
```

Diagnostics must never suggest adjacency as a call form.

---

## 33. Security and trust

### 33.1 Build-time code is code

Custom importers, native package build code, binding generation, arbitrary build scripts, and foreign-runtime package installation execute with explicit capabilities.

Their actions are recorded in build metadata.

### 33.2 Unsafe inventory

The compiler emits an unsafe inventory covering:

- source `unsafe` blocks;
- `unsafe rust`;
- raw C calls;
- unchecked layout/pointer operations;
- native packages declaring unsafe contracts.
- embedded foreign runtimes and their loaded extension modules.

### 33.3 Reflection privacy

Full reflection and tracing may expose:

- source;
- generated Rust;
- field names;
- paths;
- values;
- package versions.

Release profiles must control what is embedded or emitted.

Sensitive values are redacted unless explicitly opted into capture.

### 33.4 Supply-chain provenance

Package artefacts should be content-addressed and signed where the ecosystem supports it.

Generated Rust and final binaries should be traceable to locked source inputs.

### 33.5 Reproducible importer behaviour

An importer that uses network, time, randomness, or environment state must declare those effects.

Strict reproducible builds may deny them or require recorded inputs.

### 33.6 Sandboxing

Build-time extensions should run under a capability sandbox where platform support permits.

A project may deliberately grant full access. The language does not pretend that powerful custom import behaviour is safe merely because it is elegant.

Foreign packages execute with the authority of their host runtime; a Terrane object proxy is not a sandbox. Runtime adapters must expose filesystem, network, environment, process, and native-extension requirements to capability analysis where the runtime can report them, and must mark unknown effects rather than claiming isolation.

---

## 34. Provisional grammar sketch

This is normative EBNF for the covered core forms. Names in capitals are layout tokens emitted by the lexer. Lexical terminals such as `letter`, `digit`, `literal`, `namespace-component`, and the opaque foreign/Rust/text bodies are defined by their dedicated sections. Semantic restrictions and layout notes remain outside the machine-readable fence.

```text
identifier-unit
  = letter { letter | digit }

post-joiner-identifier-unit
  = { letter | digit } letter { letter | digit }

identifier
  = identifier-unit
    { identifier-joiner-run post-joiner-identifier-unit }

identifier-joiner-run
  = identifier-joiner { identifier-joiner }

identifier-joiner
  = "+" | "-" | "*" | "/" | "%" | "<" | ">"

comment
  = line-comment
  | block-comment

line-comment
  = ( "#" | "//" ) { non-newline-character }

block-comment
  = "/*" { character except the terminating sequence "*/" } "*/"

object-name
  = "." identifier

namespace-declaration
  = "namespace" namespace-component { namespace-component }

namespace-path
  = [ namespace-anchor ] namespace-component { namespace-component }

namespace-anchor
  = "/"
  | ".." { ".." }

dependency-declaration
  = "use" package-name
  | "use" ( "rust" | "system" ) package-name
  | "use" "runtime" runtime-name

package-name
  = identifier { namespace-component }

runtime-name
  = identifier

foreign-source-block
  = runtime-name indented-foreign-body

from-import
  = "from" namespace-path "import"
    object-import { "," object-import }

object-import
  = object-name [ "as" object-name ]

importer-selection
  = [ "global" ] "import" "with" object-name

visibility
  = "public" | "private" | "protected"

declaration-modifier
  = object-name

binding
  = { declaration-modifier }
    [ visibility ] ( "global" | "constant" )
    identifier [ type-expression ] [ "=" expression ]
  | { declaration-modifier } visibility
    identifier [ type-expression ] [ "=" expression ]
  | identifier type-expression [ "=" expression ]

class-declaration
  = { declaration-modifier }
    [ visibility ] [ "linear" ] "class" identifier
    [ "extends" type-expression ]
    [ "implements" type-expression { "," type-expression } ]
    indented-body

function-declaration
  = { declaration-modifier }
    [ visibility ] { function-qualifier }
    "function" [ identifier [ type-expression ] ]
    [ ";" parameter-list ]
    indented-function-body

function-qualifier
  = "static" | "async" | "mutating" | "throws"

parameter-list
  = parameter { "," parameter }

parameter
  = identifier [ type-expression ] [ "=" expression ] [ "..." ]

type-expression
  = union-type

union-type
  = prefix-type { "|" prefix-type }

prefix-type
  = "ref" prefix-type
  | function-type
  | applied-type

applied-type
  = type-primary [ "of" constructor-argument-list ]

constructor-argument-list
  = constructor-argument { "," constructor-argument }

constructor-argument
  = type-expression
  | constant-expression

type-primary
  = identifier
  | "(" type-expression ")"

function-type
  = "function" [ "from" function-parameter-types ] "to" type-expression

function-parameter-types
  = type-expression { "," type-expression }

compilation-unit
  = statement-list

statement-list
  = { statement NEWLINE }

indented-body
  = NEWLINE [ INDENT statement-list DEDENT ]

statement
  = namespace-declaration
  | dependency-declaration
  | from-import
  | importer-selection
  | binding
  | class-declaration
  | function-declaration
  | assignment-statement
  | expression
  | if-statement
  | while-statement
  | for-statement
  | try-statement
  | throw-statement
  | return-statement
  | break-statement
  | continue-statement
  | yield-statement
  | match-statement
  | unsafe-statement
  | rust-statement
  | label-statement
  | goto-statement
  | build-selection
  | foreign-source-block

assignment-statement
  = assignment-target "=" expression

assignment-target
  = primary-expression
    { "." identifier | "[" expression "]" }

if-statement
  = "if" expression indented-body
    { "else" "if" expression indented-body }
    [ "else" indented-body ]

while-statement
  = "while" expression indented-body

for-statement
  = "for" for-target "in" expression indented-body
  | "for" for-clause ";" expression ";" for-clause indented-body

for-target
  = identifier { "," identifier }

for-clause
  = assignment-statement
  | expression

try-statement
  = "try" indented-body
    ( catch-clause { catch-clause } [ "finally" indented-body ]
    | "finally" indented-body )

catch-clause
  = "catch" call-free-expression [ "as" identifier ] indented-body

throw-statement
  = "throw" expression

return-statement
  = "return" [ expression ]

break-statement
  = "break"

continue-statement
  = "continue"

yield-statement
  = "yield" expression

match-statement
  = "match" expression NEWLINE
    [ INDENT { match-arm } [ "else" indented-body ] DEDENT ]

match-arm
  = "case" call-free-expression [ "as" identifier ] indented-body

unsafe-statement
  = "unsafe" indented-body

rust-statement
  = [ "unsafe" ] "rust" indented-rust-body

label-statement
  = "label" identifier

goto-statement
  = "goto" identifier

build-selection
  = "when" "build" ";" expression indented-body
    { "else" "when" "build" ";" expression indented-body }
    [ "else" indented-body ]

expression
  = logical-or-expression

logical-or-expression
  = logical-and-expression { "or" logical-and-expression }

logical-and-expression
  = identity-expression { "and" identity-expression }

identity-expression
  = comparison-expression
    [ "is" comparison-expression
    | "is" "a" type-expression ]

comparison-expression
  = bitwise-or-expression
    [ ( "==" | "!=" | "<" | "<=" | ">" | ">=" )
      bitwise-or-expression ]

bitwise-or-expression
  = bitwise-xor-expression { "|" bitwise-xor-expression }

bitwise-xor-expression
  = bitwise-and-expression { "^" bitwise-and-expression }

bitwise-and-expression
  = shift-expression { "&" shift-expression }

shift-expression
  = additive-expression { ( "<<" | ">>" ) additive-expression }

additive-expression
  = multiplicative-expression { ( "+" | "-" ) multiplicative-expression }

multiplicative-expression
  = prefix-expression { ( "*" | "/" | "%" ) prefix-expression }

prefix-expression
  = ( "not" | "-" | "~" ) prefix-expression
  | ( "ref" | "move" | "await" ) postfix-expression
  | postfix-expression

postfix-expression
  = primary-expression
    { "." identifier | "[" expression "]" | "++" | "--" }
    [ call-clause ]

primary-expression
  = identifier
  | object-name
  | literal
  | tail-string
  | block-string
  | "(" expression ")"

call-clause
  = ";" [ argument-list ]

argument-list
  = argument { "," argument }

argument
  = [ identifier "=" ] call-free-expression

call-free-expression
  = expression-with-the-call-clause-production-disabled

tail-string
  = ">" { source-character } physical-line-end

block-string
  = ">>" physical-line-end indented-text-body
```

A maximal compact token matching `identifier-unit identifier-joiner-run digit { digit }` is a lexical error rather than multiple tokens. This rejection applies only when the digits-only unit follows a joiner; an ordinary `identifier-unit` may end in digits.

`assignment-target` is syntactically a primary followed only by member or index operations. Semantic analysis accepts a mutable bare binding, or a member/index path whose final operation implements assignable storage. It rejects literals, object-form symbols, calls, postfix updates, temporary values without assignable storage, and any path forbidden by ownership, borrow, visibility, or COW-pinning rules. Every receiver and index is evaluated exactly once.

A bare `identifier = expression` is the ordinary assignment form: it initializes a new binding when declaration is permitted and no binding resolves, otherwise it rebinds the resolved mutable binding. Visibility, declaration modifiers, `global`, `constant`, and an uninitialised declaration always use `binding`, so `private cache = .map;` is structurally unambiguous.

Each function qualifier may appear at most once, and incompatible combinations are rejected semantically. The recursive operator production permits conventional combinations such as `not -value`, while `ref`, `move`, and `await` consume a postfix operand and therefore reject accidental forms such as `ref ref value` and `move move value`. Unary `+` is not a core operation.

The `is a` alternative is selected only when `a` is followed by a complete `type-expression`; otherwise the comparison alternative treats `a` as an ordinary identifier. `call-free-expression` is the expression grammar instantiated with the optional `call-clause` on `postfix-expression` disabled. This parameterisation avoids duplicating every precedence production; parser-generator sources must expand it mechanically. A parenthesised `expression` re-enables calls, which is why nested invocation requires grouping.

The semantic resolver checks the first component of a `from` path against declared runtime names before native namespace resolution. Thus `from python numpy import .array` is syntactically an ordinary `from-import`, but resolves through the adapter introduced by `use runtime python`. A runtime name at statement position begins an opaque, indentation-delimited `foreign-source-block`; its adapter owns the nested grammar and source map.

The parser emits every `constructor-argument` as one unified syntax-node kind because identifiers and other forms may resolve as types or compile-time values. Constructor signatures classify those nodes during semantic analysis. Function types associate to the right; grouping overrides that association.

Postfix/member operations bind most tightly, followed from high to low by prefix operators, multiplicative, additive, shifts, bitwise AND, XOR, OR, comparisons, identity/type membership, logical AND, and logical OR. Binary arithmetic, shift, bitwise, `and`, and `or` operators associate left. Comparisons are non-associative: `a < b < c` is invalid and must be written as `a < b and b < c`. Prefix operators associate right. A postfix call clause applies to the complete postfix expression immediately to its left.

Operands and call arguments evaluate strictly left to right. Member receivers evaluate before member selection; an assignment target's receiver and indices evaluate once, left to right, before the assigned value; `and` and `or` short-circuit; all other listed binary operators evaluate both operands. Default argument expressions evaluate at the call site after supplied arguments have been evaluated, in parameter order. The compiler may reorder only when it proves that source-observable values, effects, throws, mutation, destruction, and reference/COW separation are unchanged.

`print.concat` is a member expression. `print .concat` is invalid because adjacency is not invocation. `>native executable` is a tail string when `>` appears in expression-start position. An exact `>>` followed by a newline opens an indented block string. Neither text form is admitted as a non-final ungrouped subexpression.

### 34.1 Indentation grammar

The lexer emits:

```text
NEWLINE
INDENT
DEDENT
```

like other indentation-sensitive languages.

Unlike Python, a grammar production that opens a possible block may legally receive no `INDENT`, producing an empty body.

Compound clauses (`else`, `catch`, `finally`, `case`) align with the construct that owns them. A `return` without an expression ends at `NEWLINE`; `throw` and `yield` require expressions. `break` and `continue` take no value in version one. `try` requires at least one `catch` or `finally`; `catch` clauses precede the optional `finally`. Labels and `goto` remain function-local and are checked against the ownership and cleanup rules in §14.7.

`match` is reserved by this grammar but remains outside the minimum compiler milestone under §14.8; an implementation that accepts it must implement this complete statement shape rather than private syntax. Rust and foreign-source bodies are opaque, indentation-delimited token regions whose owning adapter preserves nested source maps.

### 34.2 Call expressions

A call clause owns the remainder of its containing logical expression. Its arguments cannot contain an ungrouped call clause; nested calls are grouped:

```text
call; a - b, (convert; value)
```

The semicolons separating a three-clause `for` are owned by that statement, so calls within its clauses are likewise grouped.

---

## 35. End-to-end examples

### 35.1 Namespace-local output override

```text
namespace my-app

from /my-output import .print
print = .print

function main
  print; >Hello! From, "Terrane"!
```

Only `my-app` and descendants see this `print` unless it is promoted globally.

### 35.2 Program-global output override

```text
from mylib tools import .myprint
global print = .myprint
```

Ordinary global lookup of `print` now resolves to `mylib tools`’ function. The core implementation has no sacred claim on the binding and remains available through `from /core output import .print`.

### 35.3 Custom importer

```text
namespace plugins

from /build importers import .sandboxed-import
import with .sandboxed-import

from third-party plugin import .plugin
```

The final import is resolved by `.sandboxed-import`.

### 35.4 Strict conversion

```text
function read-ratio float; input
  ratio float = input.coerce; float
  return ratio
```

An invalid conversion throws `.coercion-error`.

### 35.5 Value, ref, and move

```text
a = .list; 1, 2, 3

b = a
# b is independently mutable; storage may initially be shared by cow

c = ref a
# c shares a's identity

handle = .device-handle;
worker-handle = move handle
# handle is unavailable
```

### 35.6 Both forms of `for`

```text
for item in things
  print; item

for i = 0; i < 10; i++
  print; i
```

### 35.7 Error handling

```text
function load bytes; path string
  try
    file = .file; path
    return file.read;

  catch .not-found as error
    throw .config-error; error

  finally
    trace; load complete
```

### 35.8 Inline Rust hot path

```text
function checksum uint64; data bytes
  rust
    fast_checksum(data.as_slice())
```

Callers do not care that the implementation is handwritten Rust.

### 35.9 Reflection and Rust inspection

```text
info = reflect; checksum

print; info.source
print; info.compile.rust
print; info.compile.rust-type
```

### 35.10 Embedded Python

```text
use runtime python

from python numpy import .array

values = .array; 1, 2, 3, 4
print; values.mean;

python
  import torch
  tensor = torch.tensor([1, 2, 3])
  print(tensor.sum())
```

The imported NumPy array is a foreign proxy with explicit runtime reflection. The embedded block crosses the same visible Python boundary and retains Python source locations for errors and debugging.

### 35.11 Kernel-oriented code

```text
namespace kernel memory

linear class mapped-page

function map-page mapped-page; virtual uint64, physical uint64
  unsafe rust
    page_table::map(virtual, physical)
```

The source remains high-level, but the target profile rejects unavailable allocation, reflection, unwinding, or thread features.

---

## 36. Conceptual generated Rust examples

These examples illustrate intent, not a fixed runtime ABI.

### 36.1 Typed scalar

Source:

```text
count int = 42
```

Possible Rust:

```rust
let mut count: i64 = 42;
```

### 36.2 Potentially widening `int`

Source:

```text
value = 9223372036854775807
value++
```

Possible conceptual Rust:

```rust
let mut value = Int::Small(i64::MAX);
value = match value {
    Int::Small(current) => match current.checked_add(1) {
        Some(result) => Int::Small(result),
        None => Int::from_i128(i128::from(current) + 1),
    },
    other => other.add_small(1)?,
};
```

The `i64` overflow branch is ordinary representation promotion, not a source throw. `from_i128` normalises when possible, `add_small` may widen transactionally to limb storage, and `?` represents only declared effects such as allocation failure. A compiler may prove a narrower representation or use a different runtime layout while preserving this behaviour.


### 36.3 Dynamic finite union

Source:

```text
if condition
  value = 42
else
  value = 'unknown'
```

Possible Rust:

```rust
enum ValueAtNode123 {
    Int(i64),
    String(String),
}
```

### 36.4 Text method passed to print

Source:

```text
message = ' '.concat; a, b, c
print; message
```

Possible conceptual Rust:

```rust
let message = " ".concat([
    a.as_text(),
    b.as_text(),
    c.as_text(),
])?;
print.call(message)?;
```

The real compiler may inline the concatenation or stream directly when source-observable behaviour permits.

### 36.5 Value assignment with COW

Source:

```text
b = a
```

Possible Rust:

```rust
let mut b = CowValue::share(&a);
```

The profiler still reports one semantic assignment and zero physical copies until separation.

### 36.6 Explicit reference

Source:

```text
b = ref a
```

Possible Rust depends on escape analysis:

```rust
let b = &mut a;
```

or a generated reference-counted identity wrapper when the lifetime escapes.

---

## 37. Standard library shape

The standard library should remain namespaced and capability-oriented.

A plausible hierarchy is:

```text
/core types
/core output
/core errors
/core reflection
/core collections
/text formatters
/text encoding
/system files
/system process
/system memory
/system time
/system observability
/network
/concurrency
/testing
```

The prelude imports or binds only a very small subset.

Standard APIs should follow the same object conventions as user packages. Compiler magic must be limited to facilities that cannot be expressed otherwise.

---

## 38. Implementation sequencing

The normative language design does not duplicate the compiler's operational roadmap. Implementation milestones, ordering, deliverables, and validation commands live in [the compiler plan](compiler-plan.md). This specification constrains that plan through the semantics and invariants stated here; changing milestone order does not change the language contract.

---

## 39. Prototype acceptance tests

The first serious prototype should prove all of these:

Unless a snippet explicitly tests unresolved lookup, the conformance harness supplies the imports named by that snippet's fixture. Prose examples outside the harness must either show their imports or state the standard namespace from which omitted object symbols come; object-form names are never implicitly added to the prelude.

1. `print.concat` and `.concat` parse as member and object-form expressions, while `print .concat` is rejected.
2. `namespace my-output formatters` and `from /my-output formatters` resolve symmetrically.
3. `/` anchors root and is never treated as a namespace separator.
4. `ipv4/ipv6` is one identifier and one namespace/package component, while `ipv4 / ipv6` is division.
5. `a+b`, `a + b`, `a+ b`, and `a +b` respectively tokenise as an identifier, an addition, an undeclared-postfix error, and an addition; `count-1` is a lexical error suggesting `count - 1`, while `sha256` remains an identifier.
6. `foo+bar`, `foobar`, and `fooplusbar` resolve independently and map injectively to distinct valid Rust identifiers.
7. `.. foo` resolves one tier upward.
8. importing `.print` does not bind `print`.
9. `print = .print` binds namespace-locally.
10. `global print = .print` replaces the program-global binding.
11. `import with .custom-import` changes subsequent import resolution in its namespace, `global import with .custom-import` selects the program fallback, and an ordinary binding named `import` changes neither.
12. `#`, `//`, and `/* ... */` comments lex and format without changing indentation structure.
13. an unterminated block comment fails at its opening delimiter, and an unused string is never treated as a comment.
14. quoted, tail, and indented block strings preserve their specified content deterministically.
15. typed scalars lower to native Rust primitives.
16. dynamic finite alternatives lower without a universal heap object.
17. explicit coercion succeeds or throws cleanly; integer checked, wrapping, and saturating conversions obey their destination-width contracts.
18. value assignment prevents mutation leakage.
19. COW avoids a physical copy until mutation.
20. `ref` preserves shared identity.
21. nested COW values separate on mutation without leaking changes.
22. a foreign proxy requires explicit `ref` or `move` rather than weakening value assignment.
23. a Python import resolves through `libpython3` and exposes a reflected foreign proxy.
24. Python exceptions retain their traceback in a `.python-error`.
25. both `for` forms compile.
26. throw/catch lowers without ordinary panic.
27. a Rust error is mapped back to the source span.
28. inline Rust sees source values through documented generated names.
29. a function’s generated Rust is retrievable in a development build.
30. profiling distinguishes semantic assignment, shared storage, physical copy, COW split, ref, and move.
31. profiling exposes Python transitions and data copies.
32. a simple allocator-free target rejects hosted-only capabilities at source level.
33. `==`, `is`, and `is a` respectively test value equality, source-visible identity, and type assignability; exact type-and-value comparison uses an explicit conjunction and `===` is rejected.
34. labels are function-local; `goto` cannot enter a deeper lexical scope or cross initialisation/lifetime transitions unsafely, and every accepted jump lowers to sound Rust with identical cleanup order.
35. `when build` selects namespace declarations and function statements deterministically, excludes inactive branches from the current build, and records every selection input in the build cache key.
36. `ref T`, `borrowed-ref of T`, `user-ref of T`, `raw-address of T`, `array-ref of T`, `c-pointer of T`, and `function from ... to ...` enforce distinct identity, lifetime, address-space, provenance, extent, and ABI contracts without implicit conversion between them.
37. In declaration-modifier position, `.weak` resolves through object-form lookup; a bare `weak` binding is never a modifier, and an unavailable `.weak` is a compile-time error.
38. `constant` declarations parse in every binding position and `const` is rejected as a declaration word.
39. `array of vm-struct|none, nr-cached-stacks` parses as one constructor application whose signature classifies its first argument as a type and its second as a compile-time integer.
40. `function from int, c-pointer of opaque to int` associates to the right; nested callable parameters format with grouping whenever the ungrouped form would be difficult to scan.
41. `void` is accepted only as the no-produced-value contract, while `opaque` is accepted as a type with hidden representation; neither substitutes for the other.
42. a borrow derived through member access or collection iteration retains the origin borrow's anonymous provenance and cannot escape or widen its inferred lifetime.
43. reflection reports source name, generated Rust name, and native symbol independently, and `native-name; mmdrop, "__mmdrop"` changes only the last.
44. lexical ownership and acyclic strong references destroy deterministically, while a provable strong cycle is rejected and an uncollectable runtime cycle is diagnosed or documented as a leak rather than promised deterministic reclamation.
45. object-form imports obey lexical and namespace scope, nearer imports shadow farther ones, same-scope collisions are rejected, aliases retain both objects, and ordinary bindings never satisfy object-form lookup.
46. plain top-level assignment remains namespace-local even in the root namespace; creating or replacing a program-global binding without `global` is rejected.
47. the default prelude contains exactly `print`, `int`, `float`, `bool`, `string`, `bytes`, and `none`; disabling it removes those defaults while explicit `/core` imports still work.
48. a call owns its remaining logical expression, nested calls require grouping, zero-argument calls require `;`, and three-clause `for` semicolons cannot be consumed as call delimiters.
49. source type parameters are rejected; strict code uses concrete types, unions, interfaces, or generated concrete declarations rather than silently becoming dynamic.
50. `c is a` parses as identity against the binding `a`, `c is a widget` parses as type membership, ordinary identity-less values compare false even to themselves, explicit refs alias one identity, and linear resources preserve identity across moves.
51. core text display renders supported scalar values canonically, `print` consumes that protocol and appends a newline, arbitrary `bytes` and values without text display are rejected rather than guessed, and locale-sensitive or styled formatting remains explicitly imported.
52. an interior `ref` separates COW storage, remains attached to its original logical owner, pins the referenced path, and rejects removal, replacement, escape, or lifetime widening while live.
53. exported may-throw functions expose `throws`, non-throwing callable contracts reject may-throw implementations, fixed-width checked arithmetic throws a catchable `.arithmetic-overflow`, `int` representation promotion does not throw, and explicit wrapping operations do not.
54. assigning a subclass value to a base-typed binding preserves the complete dynamic value and dispatch; implementations that would slice are rejected.
55. protocols express structural capabilities, interfaces define typed dispatch boundaries, traits reuse implementation without becoming types, and single inheritance preserves value and dynamic-type semantics.
56. only declared precompiled host extensions execute as importers or modifiers; `when build` accepts only its restricted deterministic query subset, records inputs and plans in cache keys, and never recursively executes ordinary Terrane source.
57. an `async function` has an async callable type, `await` is rejected outside async context, sync and async callables are incompatible without an explicit adapter, and no borrow crosses suspension unless its contract proves that lifetime.
58. default `string.length` requires grapheme segmentation capability; a target lacking it diagnoses the operation instead of substituting scalar or byte length, while explicit scalar/byte views remain available.
59. representation specialisation may inspect only a package compilation unit and declared dependency metadata; downstream packages consume the published representation contract rather than changing upstream layout.
60. precedence, associativity, comparison non-associativity, short-circuiting, receiver/index evaluation, assignment-target evaluation, argument order, and default-argument order match §34 exactly under both interpreted tooling and generated Rust.
61. `private cache = .map;`, `protected state = none`, bare rebinding, member assignment, and index assignment parse; literals, calls, postfix updates, non-assignable temporaries, and ownership-invalid paths are rejected as assignment targets.
62. every statement form in §34 parses with empty and non-empty bodies where allowed; `else`, `catch`, `finally`, and `case` bind only to their owning constructs, and `return`, loop control, throw, yield, labels, and jumps preserve required cleanup.
63. unary `-`, `~`, and `not` compose according to precedence; unary `+`, `ref ref value`, and `move move value` are rejected.
64. unconstrained integer literals beyond `int64` and `int128` range remain `int`; runtime addition, subtraction, and negation promote exactly from the compact tier through `i128` to arbitrary precision without a source-visible overflow.
65. completed `int` operations normalise back to the smallest exact tier, including an `i128`-tier value crossing into `int64` range and a big value producing a small result; equality and hashing remain identical across every tier.
66. multiplying two small `int` values uses an exact `i128` intermediate, wider multiplication produces the exact arbitrary-precision result, and multiplication by `0`, `1`, or `-1` preserves promotion and normalisation edge cases.
67. signed `/`, `%`, and `div-rem` obey the Euclidean quotient/remainder invariant for every sign combination; division by zero throws `.division-by-zero`, `int` division promotes for a representation `MIN / -1`, and fixed-width `MIN / -1` follows its selected overflow mode.
68. every signed and unsigned fixed width through 128 bits keeps its declared type under arithmetic and implements throwing ordinary, checked, wrapping, saturating, and overflowing operation contracts without build-mode-dependent behaviour.
69. `coerce`, `coerce.checked`, `coerce.wrap`, and `coerce.saturate` handle signedness and every `int`/fixed-width boundary exactly; checked failure never mutates the destination, wrapping uses destination-width bits, fixed-width-to-`int` conversion cannot overflow, and the obsolete flat spellings are rejected.
70. `int` bitwise operations behave as infinite two's-complement arithmetic across positive and negative operands and every representation tier; `~x == -x - 1`, left shift is exact, right shift is arithmetic/flooring, negative counts throw `.negative-shift-count`, and very large right shifts produce `0` or `-1` without count wrapping or proportional allocation.
71. direct signed fixed-width initialisers accept each type's syntactically negated minimum literal, including `-128` as `int8` and `-2^127` as `int128`, reject the next lower value, and do not first reject the unsigned positive magnitude.
72. fixed-width numeric descriptor objects resolve only through explicit `/core types` object-form imports and ordinary bindings; they are not added to the exact default prelude or treated as reserved type words.
73. canonical type descriptors are real values with stable identity, while a first-version type expression or coercion destination must resolve to a finite compiler-known descriptor alternative even when the lowering erases the runtime descriptor.
74. numeric-to-float coercion rounds to nearest with ties to even and reports precision loss through the destination type rather than an error, unrepresentable float destinations and unparseable text throw `.coercion-error`, and parsing coercion accepts exactly the destination's canonical text-display spelling.

---

## 40. Deliberate validation points

The architecture is coherent enough to implement, but these details should be tested in real code before being frozen.

### 40.1 Zero-argument dot objects

The current draft treats:

```text
.thing
```

as object lookup and:

```text
.thing;
```

as zero-argument default invocation/construction.

A prototype should test whether zero-argument class construction deserves a safe shorthand without making imported singleton/function objects ambiguous.

### 40.2 Map literal syntax


`.map` construction and methods are semantically sufficient.

A compact computed-key literal syntax should be added only after it can be made consistent with the language’s punctuation model.

### 40.3 Generic type spelling

```text
list of string
```

is readable and unshifted, but needs parser and tooling validation in complex signatures.


### 40.4 Class inheritance lowering

Single inheritance is useful, especially with `protected`, but generated Rust quality should be tested against composition plus interfaces.

The source feature should remain only if its costs stay inspectable and unsurprising.

### 40.5 Reference implementation

The source semantics of `ref` are fixed as shared object identity.

The compiler’s thresholds for borrow, `Rc`-like, `Arc`-like, or custom dynamic storage need profiling and target-specific tuning.

### 40.6 Public-by-default package APIs

Public-by-default matches the language philosophy.

A package linter or strict API mode may still be desirable to prevent accidental long-term compatibility commitments.

### 40.7 Reflection embedding

Runtime access to generated Rust is extremely useful in development.

The default release policy—embedded, sidecar, or stripped—must balance inspectability, binary size, security, and deployability.

### 40.8 Import evaluation order

Source-order `import with` selection is understandable and bootstrappable.

Large projects may prefer manifest/declarative importer composition. Both can coexist if precedence is rigidly specified.

---

## 41. Core invariants

The following are the design’s constitutional layer. They govern the entire document and override conflicting illustrative prose, examples, lowering sketches, or implementation plans:

1. Everything is an object semantically.
2. Runtime representation is free to be non-object-shaped when behaviour remains identical.
3. Values have types even when bindings are dynamic.
4. Dynamic typing never implies weak implicit coercion.
5. Type constraints are optional, local, and real.
6. Coercion is explicit and object-driven.
7. Ordinary assignment has value semantics.
8. Ordinary values may share backing storage, but mutation separates them before changes become observable elsewhere.
9. `ref` is the visible shared-identity operation.
10. `move` is the visible ownership-transfer operation.
11. Imports do not automatically pollute ordinary bindings.
12. Plain names and dot-object names are distinct lookup views over objects.
13. Namespace tiers are whitespace-separated.
14. `/` only anchors root; it is not a separator.
15. Operator-bearing identifiers and spaced infix expressions are lexically distinct and formatter-protected.
16. `foo.bar`, `.bar`, and `foo; .bar` are member lookup, object lookup, and explicit argument passing; whitespace adjacency never invokes.
17. The global namespace is small by default and engineer-controlled.
18. Prelude facilities such as `print` have no sacred claim to their ordinary names; replacing one is the engineer's responsibility.
19. Compile-time constructs such as import selection use dedicated structural slots and never depend on same-spelled ordinary bindings.
20. Control flow is conventional unless novelty buys something concrete.
21. Empty blocks require no ceremonial statement.
22. Public/dynamic is the permissive default; private/protected/strict are available where wanted.
23. Rust is the canonical lowered form.
24. Generated Rust is deterministic, readable, inspectable, and source-mapped.
25. Source-to-Rust identifier encoding is exact, deterministic, and injective.
26. Rust diagnostics are returned to source without hiding the originals.
27. Inline and full-file Rust are first-class, not an afterthought.
28. Native, Rust, system/C, and declared foreign-runtime dependencies belong in one inspectable package graph.
29. Rust is native lowering; foreign runtimes remain explicit semantic, performance, ownership, and deployment boundaries.
30. Compilation is transparent in development and explicit in deployment.
31. Production does not require dynamic source compilation or a bespoke Terrane VM.
32. Reflection, debugging, tracing, and performance explanation are compiler contracts, not later plugins.
33. Hosted convenience must not prevent allocator-free, embedded, firmware, or kernel realisation where source capabilities permit it.
34. The compiler must explain costs and constraints rather than silently repairing semantics.
35. The abstraction must always have a clean downward path to Rust.
36. Value equality, source-visible identity, and type membership are distinct predicates; no combined equality operator obscures which relation is intended.
37. Labels and `goto` are function-local, lifetime-checked low-level control flow; no accepted jump may compromise deterministic cleanup or sound Rust lowering.
38. `when build` is deterministic compile-time source selection over declared build inputs, never hidden runtime branching or untracked configuration.
39. A safe object reference, a bounded borrow, an untrusted userspace address, a raw machine address, an ABI-erased pointer, a contiguous view, and a callable ABI address are distinct contracts; adapters may refine but never silently weaken them.
40. Declaration modifiers are explicit object-form lookups; bare identifiers never become modifiers or consult the ordinary binding view.
41. Package-defined type constructors classify a common constructor-argument syntax as type or compile-time value without extending the parser grammar.
42. `void` means no produced value and never acts as erased storage; `opaque` names unavailable representation, whose reference contract must still identify ownership, lifetime, address space, and operations.
43. Every derived borrow retains compiler-assigned provenance and may preserve or narrow, but never widen, the origin lifetime.
44. Source names, generated Rust names, and native ABI/link symbols are independent reflected identities.
45. Deterministic destruction is guaranteed by lexical ownership and acyclic final strong-reference release, not by arbitrary strong-cycle reachability.
46. `int` denotes an exact arbitrary-precision signed value with compact adaptive representation; representation overflow promotes and completed results normalise, while explicitly fixed-width integers alone make width overflow and conversion policy source-visible.

---

## 42. Deferred language additions

This section records directions that the current design should leave room for but does not make part of the version-one language contract. Entries here are neither reserved syntax nor permission for implementations to introduce incompatible private variants. Each requires a later specification change, grammar and tooling work, lowering rules, diagnostics, reflection behaviour, and conformance tests.

### 42.1 Core constructs supplied as objects

The object model may eventually extend beyond replaceable facilities such as `print`: named language constructs could be selected from `/core` through one uniform compile-time construct protocol. The family must be designed together rather than adding an isolated hook for `function`. Candidates include declarations and control-flow constructs such as `function`, `class`, `if`, `for`, `while`, `try`, `throw`, `async`, `await`, and `return`.

The intended architectural split is:

```text
fixed lexical and layout substrate
  -> structurally parsed construct
  -> scoped construct implementation selected from /core or a package
  -> validated typed semantic IR
  -> ordinary lowering
```

Tokenisation, comments, indentation, literals, grouping, separators, namespace anchors, ownership and safety invariants, and the mechanism that selects construct implementations remain constitutional compiler structure. A construct implementation may validate or constrain a parsed construct, select compiler-supported ABI or lowering behaviour, attach reflected metadata, and produce source-mapped declarations through declared extension points. It must not reinterpret arbitrary source text, mutate the grammar opportunistically, hide effects, bypass safety or capability checks, or emit unsourced code.

Construct selection must use a dedicated scope and explicit syntax; it must not depend on an ordinary binding that happens to be named `function` or `if`. The eventual design must specify lexical, namespace, package, and program-global replacement; interactions among related constructs such as `if`/`else` and `try`/`catch`; compatibility with editor parsing before dependency resolution; hygiene; reproducibility; compiler-protocol versioning; and how source declares the language profile it expects.

Declaration modifiers are the version-one local customization mechanism. A future construct binding would select the default semantics for a whole scope, while a modifier would customize one declaration. Until the common construct protocol is specified, version one keeps named core constructs structurally built in, and implementations must not expose an ad hoc replaceable `function` or any equivalent one-off hook.

### 42.2 Other deferred candidates

The following already-motivated features may be specified later when implementation experience justifies them:

- source-declared generics, including constraints, inference, dispatch, reflection, and monomorphisation or erasure rules;
- compact map literals consistent with the punctuation and computed-key model;
- stateful hot-code replacement with explicit object migration semantics;
- arbitrary C++ ABI integration beyond C-compatible shims and Rust bridges;
- multimethod or generic-function dispatch supplied as a library or language feature without making overload resolution implicit;
- additional foreign-runtime adapters governed by the same explicit boundary contracts as Python.

This list is intentionally non-exhaustive. Adding an item here protects a design direction from accidental closure; it does not give that feature priority over the version-one compiler plan.

---

## 43. Closing proposition

The language is not justified merely by prettier syntax.

Its claim is the combination:

```text
human-friendly object language
  + clean and controllable namespaces
  + dynamic bindings with typed values
  + strictness on demand
  + value semantics with explicit identity
  + transparent generated Rust
  + native/Rust/C package interoperability
  + explicit access to foreign runtime ecosystems
  + first-class diagnostics and observability
  + direct Rust escape hatches
  + compiled deployment from ordinary dynamic-language ergonomics
```

That is a credible reason for one more language: not another isolated world, but a human-facing layer that consolidates several existing ones and deliberately refuses to trap its users above the implementation.
