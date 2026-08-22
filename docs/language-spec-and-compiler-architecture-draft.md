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
- Imports bind ordinary names, scoped to the block, function, or namespace containing them.
- Namespace segments are separated by `/`, which also anchors resolution at the root; `../` ascends one tier and nests.
- Ordinary syntax favours unshifted characters and readable words over punctuation gymnastics.
- Control flow is conventional where conventional syntax is already good.
- The language lowers to readable, deterministic Rust, then uses the normal Rust toolchain.
- Native Terrane packages, Rust crates, system/C libraries, full and inline Rust, and explicit foreign-runtime adapters are first-class.
- Compilation is transparent during development and explicit at deployment boundaries.
- Reflection, source mapping, diagnostics, debugging, tracing, allocation analysis, and performance explanation are designed in from the beginning.
- A VM or JIT is not required. Fast incremental Rust compilation is the default development model.
- `no_std`, embedded, firmware, and kernel targets are possible when the program uses only capabilities available on those targets.

A representative program is:

```terrane
namespace my-app

function main

  project-name = >Terrane
  build-target = >native executable
  build-status = >ready to build

  message = ': '.join; project-name, build-target, build-status
  print; message
```

Conceptually:

1. `namespace my-app` declares this unit's namespace. Nested namespaces separate segments with `/`, as in `my-app/http/handlers`.
2. No import appears because none is needed: `print` is one of the seven default prelude bindings, and every type descriptor is a construct available without import.
3. `': '.join` looks up the `join` member on the `': '` text object.
4. Invoking that member joins its arguments using the receiver as the separator, accepting any number of arguments. This is the shape of Python's `str.join` and PHP's `implode`: the separator supplies the member rather than being passed to it. `join` is distinct from `concat`, which appends its arguments to the receiver without a separator — `'a'.concat; 'b', 'c'` is `abc`.
5. `print; message` invokes `print`’s default behaviour with `message` as its argument.

The output is:

```text
Terrane: native executable: ready to build
```

Imports exist for the cases the prelude does not cover — reaching a namespaced object, or binding one under a different name:

```terrane
namespace my-app

from /core/types import int64 as word

function main
  size word = 4096
  print; size
```

Here `/` anchors the path at the root and separates its segments, `int64` is the exported name, and `as word` binds it under a different name in this scope. Writing `from /core/output import print` would be redundant, since `print` is already available.

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

```terrane
x = 42
```

When a contract matters, it can be added locally:

```terrane
x int = 42
```

A declared destination performs its own conversion, preserving the value exactly or throwing. Where there is no destination, or where a policy other than that default is wanted, the conversion is written:

```terrane
x = x.coerce; float
```

Strictness should be additive and selectable at binding, member, function, class, namespace, package, and build-profile boundaries.

### 3.3 Clean names by default, real control when desired

The language should not begin by pouring hundreds of functions, variables, classes, helpers, and framework artefacts into global scope.

At the same time, the engineer should be able to define an actual project-global binding without fighting the language:

```terrane
global log = logger
global database = database;
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

```terrane
x = 42
```

creates an `int` object semantically. The compiler may realise it as:

```rust
let x: i64 = 42;
```

when no observable source behaviour requires boxing.

### 5.2 Values are typed; bindings may be dynamic

`42` is an `int`. It is not an “untyped scalar”.

```terrane
x = 42
```

means that `x` currently contains an `int` object. A later assignment may bind `x` to a different type:

```terrane
x = forty two
```

A type annotation constrains the binding:

```terrane
x int = 42
```

### 5.3 Dynamic does not mean weak

The language does not silently turn `'42'` into `42` merely because an operation would otherwise fail.

```terrane
x int = '42'
```

is a type error.

```terrane
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

```terrane
function render
```

is public and dynamically typed by default.

```terrane
private function render
```

narrows visibility.

```terrane
function add int; a int, b int
```

adds a type contract.

```text
strict types
```

may require contracts throughout a selected scope.

### 5.5 Dangerous behaviour should look deliberate

Ordinary assignment should not unexpectedly create shared mutable identity.

```terrane
b = a
```

means value assignment.

Shared identity is explicit:

```terrane
b = ref a
```

The implementation should satisfy ordinary value assignment through copy-on-write sharing until either logical value is modified.

A transfer of ownership for a linear value is explicit:

```terrane
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


### 5.7 Standard facilities are written in Terrane

The Rust core is deliberately minimal. Standard facilities — document formats, networking protocols, compression framing, date and time arithmetic, path handling, command-line parsing, logging, package machinery — are written in Terrane over that core rather than implemented as Rust support crates.

The decisive reason is that a Rust support crate is permanently opaque to the Terrane compiler. It is a call boundary the optimiser can never see through, so implementing a facility in Rust does not merely forgo optimisation today, it forecloses inlining, specialisation, and whole-program analysis for that facility permanently. A Terrane implementation stays visible to the entire pipeline.

Three further consequences follow. Writing the libraries in Terrane exercises the lowering against real code rather than minimal fixtures, so gaps in the language surface immediately. The libraries become a substantial corpus before a public one exists. And a failure inside a standard facility surfaces as readable Terrane frames, which the diagnostics contract already requires and a Rust support crate can never provide.

**The boundary is per layer, not per facility.** Rust owns the layer that is irreducible or externally audited; Terrane owns the object model, policy, diagnostics, and integration above it. A JSON facility may have a Rust byte-level scanner beneath a Terrane document model, descriptor-driven mapping, data-path diagnostics, and canonical output. A TLS facility uses an audited protocol implementation beneath Terrane stream integration, trust-store and ALPN objects, connector policy, and capability gating. Reimplementing TLS in Terrane is not dogfooding; it is a security liability.

Rust is the correct choice for a layer when one of the following holds, and a layer that claims to be Rust states which:

1. it is a syscall or ABI boundary — file descriptors, sockets, clocks, process control;
2. it requires a guarantee the language cannot express or the optimiser would destroy, such as constant-time comparison, memory ordering, or zeroisation. This is not a performance judgement: a constant-time equality written in ordinary Terrane can be short-circuited by the optimiser, so the guarantee must live in a primitive that forbids it;
3. it is a large, externally audited, security-critical implementation where reimplementation would be a liability rather than an exercise;
4. it is data rather than code — Unicode tables, timezone databases — generated into whichever form is cheapest.

Everything else is Terrane.

A standard facility that depends on a Rust crate uses the ordinary dependency mechanism of §23: a declaration plus a deliberately authored wrapper, with the wrapper being exactly the boundary machinery that section describes. Core libraries receive no privileged path, which means they also serve as worked examples of dependency use. They declare their Rust dependencies explicitly so that a profile may exclude them.

Two consequences shape the implementation rather than the language. Package-level artifact caching becomes load-bearing rather than an optimisation, because a source-form standard library would otherwise be recompiled by every build. And capability profiles become a question of which Terrane packages are present rather than which support crates were compiled in, which is the simpler story.
## 6. Lexical structure

### 6.1 Encoding

Source files are UTF-8.

The version-one compiler restricts identifier characters to ASCII letters and digits while the grammar stabilises. A later language version may admit Unicode identifier characters deliberately; non-ASCII characters are not silently normalized or accepted by version one.

### 6.2 Indentation and blocks

Blocks are indentation-delimited.

```terrane
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

```terrane
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
- runs of the identifier-joiner glyphs `+`, `-`, `*`, `%`, `<`, and `>`.

`/` is deliberately **not** an identifier joiner. It is the namespace separator, and a character cannot be both without making `namespace foo/bar` ambiguous between one segment and two. Context-sensitive lexing is rejected here because it would contradict the rule below that a compact joiner sequence is always an identifier, permanently. The cost is that a name such as `ipv4/ipv6` must be written `ipv4-ipv6`.

All user-declared names are lowercase: namespaces, functions, classes, interfaces, traits, fields, and bindings. The convention is kebab-case, so `parse-json` rather than `parseJson` or `parseJSON` — which removes the acronym-casing question permanently rather than leaving it to per-project taste.

Case carries no semantic load in Terrane, which is what makes this affordable. Go uses case for export, Haskell for constructors, Rust for types against values; Terrane expresses type membership through `is a` and member access through `receiver.member`, so case is free to constrain.

Uppercase parses and is then rejected with a diagnostic naming the lowercase form, plus a formatter fixit. It is never silently folded.

Type parameters are the one carve-out and remain uppercase: `list of T`, `map of K, V`, `iteration-step of Item`. A type parameter is a different kind of name — it stands in for a thing rather than naming one — is never user-declared in version one, and never forms part of a path.

Identifiers may end in digits: `http2`, `sha256`, and `vector4` are valid. The restriction applies only when a terminal digits-only unit is introduced by an identifier joiner. Compact forms such as `count-1` and `x+4` are lexical errors rather than identifiers or arithmetic. Names such as `http2-client`, `ipv4-ipv6`, and `sha3-256sum` remain valid because each unit after a joiner contains a letter.

A compact letter-to-letter joiner sequence is always an identifier, permanently: `total-count`, `page-size`, and `width-height` never mean subtraction without surrounding operator whitespace, even if a same-spelled binding exists. Arithmetic must be written `total - count`. This asymmetry is intentional: kebab-case names require a stable lexical interpretation, while a terminal joiner-plus-digits form is reserved as an error because it is not needed for that naming convention.

Examples:

```text
print
my-class
http2-client
foo+bar
ipv4-ipv6
input>output
```

The rule is lexical and universal for those glyphs: a maximal joiner run directly surrounded on both sides by identifier characters belongs to the identifier only when the following identifier unit contains a letter. A symbolic run cannot begin an identifier. When it begins a token after whitespace, a delimiter, or the start of a line and is immediately followed by an identifier character, it has behavioural/operator meaning rather than becoming part of the following name.

```terrane
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

```terrane
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

```terrane
project-kind = >native executable
message = >Hello! From, "Terrane"! >>
send; recipient, >Error: file not found!
```

The second value is exactly `Hello! From, "Terrane"! >>`. Quotes, commas, operators, comment markers, and further `>` characters have no grammatical meaning after the opening marker. Whitespace is preserved exactly, including whitespace immediately after `>` and trailing horizontal whitespace. An attached `>` with no following content is the empty string.

The marker must begin an expression and must be lexically attached to the expression position; its content begins with the very next character, which may be whitespace. This keeps it distinct from infix comparison:

```terrane
is-larger = left > right
message = >left > right
```

A tail string consumes the remainder of its line, so it is necessarily the final syntactic element on that line. It may nevertheless be the final argument of a call, as in `send; recipient, >Error: file not found!`. Use a quoted string when member access, another argument, an operator, or any other syntax must follow the literal.

An exact `>>` in an expression-start position opens a **block string** whose content is the following indented block:

```terrane
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

```terrane
x = hello
```

To create text, use one of the three explicit forms:

```terrane
inline = 'hello'
tail = >Hello, from Terrane!
multiline = >>
  Hello,
  from Terrane!
```

### 6.8 Numeric literals

A numeric literal is a run of decimal digits with an optional single `.` fraction, or a `0x` hexadecimal run:

```terrane
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
| `value.member` | member lookup; `.` appears only after a receiver |
| `;` | begin an invocation’s argument list |
| `,` | separate arguments or values |
| `|` between types | construct a union type |
| `=` | bind or assign a value |
| `/` in a namespace path | anchor at the root when leading, separate segments otherwise |
| `../` before a namespace path | ascend one namespace tier, repeatable |
| `'...'` | delimited quoted string |
| `>text` | exact text through the physical end of line |
| `>>` followed by an indented block | exact multiline block text |
| `#` or `//` | begin a line comment outside text literals |
| `/* ... */` | block comment, possibly multiline |

A leading `.` is not a name form. `.` appears only between a receiver and its member, so whitespace before a dot is always an error rather than a second interpretation:

```terrane
value.concat    # member lookup
value .concat   # invalid: whitespace before a member dot
print; value    # pass value as an argument
print; (value;) # pass the result of invoking value
```

The formatter must preserve member attachment and must never turn invalid adjacency into invocation. A bare name in argument position passes the object; invoking it requires its own semicolon, grouped when anything follows.

---

## 7. Namespaces and name resolution

### 7.1 Tiered namespaces

Namespace components are separated by `/` in source.

```terrane
namespace my-output/formatters
```

declares the tier:

```text
root
  my-output
    formatters
```

`my-output` is one component because its hyphen is internal. `formatters` is its child because it follows a `/` separator.

The namespace hierarchy corresponds to the directory tree under the manifest's declared mappings, as specified below. It is not derived from filenames: a namespace is declared in its source unit, and the correspondence is checked rather than inferred.

A file may contribute declarations to an existing namespace. Multiple files may contribute to the same namespace unless a package policy forbids it.

Package metadata declares namespace-root to directory-root mappings, and discovery is bounded to those roots. The compiler resolves the complete source set before resolving namespace declarations; there is no on-demand search by namespace name at any point, and nothing is resolved lazily. Incremental builds may avoid reparsing unchanged units from validated summaries, but adding or removing a source unit changes the package input and invalidates namespace assembly.

### 7.2 Root anchoring

`/` anchors namespace resolution at the root:

```terrane
from /image/codec import jpeg
```

A leading `/` means “start at root”. The same character separates every subsequent segment, so `/image/codec` is one anchored path rather than an anchor plus a differently-delimited remainder.

### 7.3 Relative anchoring

An unanchored path begins at the current namespace:

```terrane
from helpers/formatters import pretty
```

`/` is the only boundary marker. It anchors the root and separates every subsequent segment, so one delimiter expresses one concept:

```terrane
from /image/codec import resize
namespace my-app/http/handlers
```

`..` ascends one tier and composes as an ordinary path component:

```terrane
from ../shared import config
from ../../platform import clock
```

One delimiter expresses one concept: using `/` to anchor the root and whitespace to separate segments would be two markers for the same kind of boundary, and would degrade badly for repeated parents.

### Directory correspondence and manifest mappings

The namespace tree corresponds to a directory tree. A source unit whose declared namespace disagrees with its location is an error, unless the manifest declares that mapping explicitly. Making the correspondence checkable is the point: a misplaced file becomes a build error rather than a namespace that silently never resolves.

The manifest maps canonical namespace roots to relative directory roots:

```toml
[namespaces]
"foo/bar" = "some/path"
"foo/generated" = "generated"
```

`foo/bar/dave` then corresponds to `some/path/dave`. The same mechanism serves two purposes: relocating your own sources, and describing a dependency whose internal layout is not your concern.

- discovery recursively includes `.trn` files beneath the declared directory roots and ignores everything else, including a `.trn` file elsewhere in the tree: a file no mapping reaches is not part of the package, and the compiler neither compiles it nor reports it;
- overlapping mappings resolve by longest matching namespace prefix;
- two namespace roots mapped to the same directory are an error at manifest load, not at resolution, because a file there would have two valid namespaces;
- each mapped root must discover at least one `.trn` source;
- a dependency's namespaces come from its own manifest and are never discovered by scanning outside those roots. That is a correctness boundary rather than an optimisation: a package's public namespace structure should not depend on unrelated private file layout, and your build should not depend on either.

Expansion is bounded to declared roots and sorted by package-relative path. The compiler records the resolved source set in build metadata, so a build remains auditable and reproducible even though the manifest declares roots rather than listing every file.

Correspondence is directory-level, not file-level. A namespace spans as many source units as it likes, so every `.trn` file in one directory belongs to that directory's namespace; there is no file-per-declaration rule. For a discovered file, the longest directory mapping determines the namespace root and its relative parent directory supplies any suffix. A differing source declaration is an error with the expected namespace. A direct single-file CLI input has no manifest directory contract and is therefore exempt.

### Namespace segment grammar

A namespace segment is:

```text
[a-z]([a-z0-9]|-[a-z0-9])*
```

A lowercase ASCII letter, followed by letters, digits, and internal hyphens.

This is a **distinct production from `identifier`, and a strict subset of it**. An identifier admits the joiner glyphs `+`, `*`, `%`, `<`, and `>`; a namespace segment admits only the hyphen, because the others are illegal or hazardous in a path component. `foo+bar` is therefore a legal identifier and an illegal segment. A parser must not reuse the identifier production here, or the restriction silently never applies.

The segment grammar is an allowlist rather than a list of forbidden characters, which is what makes it complete: `/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`, NUL, control characters, leading or trailing spaces, trailing dots, `.`, and `..` are all unformable rather than rejected. A blocklist would eventually omit one of them.

Segments are lowercase because they map to directory names, and a rule permitting both cases would allow two segments differing only by case — distinct on Linux, colliding through the Win32 layer on Windows. Restricting the set removes the problem rather than managing it, and matches the kebab-case identifiers used everywhere else.

Uppercase in a segment parses and is then rejected with a diagnostic naming the lowercase form, along with a formatter fixit. It is never silently folded: quietly rewriting `Foo` to `foo` would be reinterpreting input as a nearby construct, which the no-silent-repair principle forbids, and it would leave the corpus with many spellings for one namespace.

A small set of whole names is reserved because it is composed entirely of legal characters and therefore invisible to the allowlist:

```text
con  prn  aux  nul  com1..com9  lpt1..lpt9
```

These are Windows device names, reserved with any extension. They are excluded now even though version one targets Linux first, because adding the restriction later would break any project that had used one.

Namespace segments are ASCII permanently, independently of any future widening of the identifier set. A non-ASCII segment reaches the filesystem, where macOS normalises to NFD and Linux to NFC, so one identifier produces different bytes on disk depending on where the tree is checked out.

For a current namespace of:

```terrane
namespace my-app/http/handlers
```

the paths resolve as follows:

| Source path | Result |
|---|---|
| `helpers` | `my-app/http/handlers/helpers` |
| `../shared` | `my-app/http/shared` |
| `../../platform` | `my-app/platform` |
| `/core/output` | `core/output` from the root |

Resolution never silently falls back from a failed relative path to the root. Ambiguous convenience is not worth non-local behaviour.

`/` separates namespace components and anchors a path at the root when it leads. It is not an identifier character, so `from /network/ip import address` has components `network` and `ip`, and no component can itself contain a slash. A package name is a single component: `use ipv6-ipv4` names one package. The namespace tree corresponds to a directory tree under the manifest's declared mappings, so filesystem layout is checked against the declaration rather than being unrelated to it.

### 7.4 Namespaces as objects

A namespace has an object representation available to reflection and tooling. It can report:

- its parent;
- children;
- declarations;
- visibility;
- package ownership;
- source units;
- exported objects;
- ordinary bindings.

The `/` and `..` anchors remain grammar, not replaceable runtime objects.

### 7.5 One lookup view

Names resolve through a single lexical view. A `from ... import` binds the imported object under an ordinary name in the scope containing the import:

```terrane
from /image/codec import resize
```

`resize` is then usable directly. There is no second spelling for the same object and no separate declare-then-bind step.

Imports populate the scope containing them: block imports last to the end of the block, function imports to the end of the function, and namespace-top-level imports populate that exact namespace. Lookup proceeds from the current lexical scope outward through enclosing scopes, the current namespace, and parent namespaces nearest first. Namespace-level imports are inherited by descendant namespaces under the same visibility rules as ordinary namespace bindings.

A nearer binding shadows a farther one. Introducing two different objects under the same name in one scope is a compile-time collision; source order never chooses a winner. Reimporting the same export is idempotent, and `as` is required when both colliding objects must remain available:

```terrane
from /core/output import print as myprint
```

The alias binds the exported object under `myprint` in the current scope, preserving the object's identity and visibility checks.

Because an import now binds an ordinary name, an import cannot shadow a name and leave the original reachable under a second spelling. Where both are wanted, alias one of them.

### 7.6 Declarations and exports

A declaration creates one binding in its defining namespace, and that same object is what an import selects elsewhere.

```terrane
namespace text/formatters

class concat
```

defines `concat` in `text/formatters` and makes it importable under that name. Importing it elsewhere binds `concat` in the importing scope, subject to that scope's shadowing and collision rules.

Visibility governs whether a declaration is importable at all; it is a separate question from how the name is spelled.

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

Tiers four and five carry one restriction. Resolving from inside a function or method body, the
namespace tiers yield constants, descriptor constructs, imported names, functions, and types — never
a namespace variable. A variable's value depends on when it is read, so a body that could name one
would take execution order as an implicit input, which is the thing parameters and returns exist to
make explicit. Where program-wide mutable state is genuinely wanted, `global` says so; where the
value never varies, `constant` says that instead.

Shadowing is legal. Linters may report it according to project policy.

### 7.8 Namespace-local bindings

At namespace top level, ordinary assignment binds in that namespace:

```terrane
namespace my-output/formatters

from /core/output import print as emit
```

A name bound here — an import, a constant, a construct, a function, a type — is inherited by
descendant namespace resolution unless hidden by a nearer binding.

A namespace variable is scoped to the namespace tier itself. Other namespace-level declarations in
that namespace read and write it; nothing else can. Not a function body in the same namespace, not a
descendant namespace, not an importer. Its role is composition at the tier — the intermediate steps
that produce a value something else exposes:

```terrane
namespace app/config

base int = 5
derived int = base + 1          # namespace-level composition, visible here only

constant page-size int = 4096   # crosses into function bodies
global counter int = 0          # crosses, and may be replaced, because it says so
```

A value that must leave the tier is a `constant`, a `global`, or a function result.

### 7.9 Program-global bindings

`global` binds at the program assembly root:

```terrane
global log = logger
global database = database;
global page-size int = 4096
```

Program globals are ordinary bindings. They are not a privileged language-owned namespace.

`global` is required whenever source creates or replaces a program-global binding, including in a source unit assigned to the root namespace. A plain top-level assignment always remains namespace-local. Requiring the marker prevents moving a file or changing its namespace declaration from silently changing the reach of its bindings.

A global declaration still retains its lexical declaring namespace for visibility and name resolution. `global` controls program-wide identity and lifetime; it does not erase declaration provenance or imply public visibility. Therefore `private global max-threads int = 0` denotes one program-wide binding whose source-visible name is resolvable only inside its exact declaring namespace.

A package may not silently mutate the consuming program’s global bindings merely by being installed. Global composition belongs to the program entry configuration or explicitly evaluated program source.

### 7.10 The core prelude

The default prelude is a deliberately small set of ordinary program-global bindings selected from the `/core` implementation. `/core` is an ordinary, explicitly addressable root package namespace, so its objects remain directly importable:

```terrane
from /core/output import print
```

That creates a namespace-local `print` even though the default prelude already supplies the same core object globally. The explicit form is useful in a project that disables the prelude, under an alias, or when declaring exactly which implementation a namespace uses.

The version-one default ordinary bindings are:

- `print`, sourced from `/core/output`’s `print`;
- scalar type objects `int`, `float`, `bool`, `string`, `bytes`, and `none`, sourced from `/core/types`.

This is the complete default list. In particular, collections, filesystem access, concurrency, formatting helpers, and reflection helpers require imports. `import` remains structural syntax whose behaviour is supplied by the active importer object; it is not an ordinary prelude binding.

Prelude bindings are defaults, not reserved names. Explicit program composition may replace any of them:

```terrane
from mylib/tools import myprint
global print = myprint
```

After this declaration, ordinary lookup of `print` through the program-global tier resolves to `mylib/tools`’ `myprint`. The original remains available by explicitly importing `/core/output`’s `print`. A prelude replacement does not mutate `/core`, the scopes holding other imports, or namespace-local bindings that shadow the global.

A project may replace, extend, or disable the selected prelude through its build manifest. Packages cannot do so merely by being installed or imported; program-global composition remains an entry-project decision.

Documentation fragments may omit imports when the import itself is not under discussion. Such omissions are editorial only: the fragment's fixture supplies the explicit imports. In this document `list`, `map`, `set`, `tuple`, `range`, and `entry` come from `/core/collections`; `file` comes from `/system/files`; `shared-map` comes from `/concurrency`; fixed-width numeric descriptors `int8`, `int16`, `int32`, `int64`, `int128`, `uint8`, `uint16`, `uint32`, `uint64`, `uint128`, `float32`, and `float64` come from `/core/types`; and example-only objects such as `device-handle` come from the named example fixture. A complete source unit must write those imports. None of these objects belongs to the default prelude.

---

## 8. Imports

### 8.1 Basic import form

```terrane
from /image/codec import jpeg
```

imports an exported object from a namespace and binds it in the current scope.

Multiple objects may be imported:

```terrane
from /image/codec import jpeg, png, webp
```

`as` renames a selection, which is how a collision is retained:

```terrane
from /core/output import print as core-print
from /pretty/output import print as pretty-print
```

Both names are ordinary bindings in the importing scope. Without the aliases the second import would collide with the first, and source order would not choose a winner.

### 8.2 Import is a compile-time construct slot

The parser recognises the structural form:

```terrane
from path import objects
```

Its behaviour is supplied by the importer selected for the current compile-time construct scope. The standard importer is a precompiled `/core` host extension implementing the versioned compiler importer protocol; it is not an ordinary prelude object or runtime binding.

A namespace may select another importer for subsequent imports in that namespace and its descendants:

```terrane
namespace plugins

from /build/importers import sandboxed-import
import with sandboxed-import
```

A program entry source may select one at the program-global construct tier:

```terrane
from /build/importers import content-addressed-import
global import with content-addressed-import
```

`import with` and `global import with` are structural compile-time selection statements, not assignments. Their right operand must resolve through ordinary lexical scope to a declared, precompiled host extension implementing the importer protocol. Namespace selection applies after the statement to that namespace and descendants unless a nearer selection replaces it. Global selection applies after the statement wherever no nearer namespace selection exists. Lexical blocks and functions cannot replace the importer because their imports are resolved before runtime scope exists.

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

### 9.1 Name resolution to objects

An ordinary name resolves to whatever object it is bound to:

```terrane
print
concat
jpeg
database
```

The result is an object value: perhaps a function object, class object, singleton, prototype, namespace adapter, importer, or another callable object. A name alone never invokes — invocation always requires its own semicolon — so a bare name in argument position passes the object itself.

A dot lookup alone does not imply invocation:

```terrane
```

binds the object.

### 9.2 Default invocation

A semicolon invokes an object’s default behaviour:

```terrane
print; message
```

For a function object, the default behaviour executes the function.

For a class object, the default behaviour constructs an instance.

For an importer, it resolves an import request.

For an ordinary object, the class may define whatever default invocation means.

A zero-argument invocation is explicit:

```terrane
thing = thing;
```

### 9.3 Member lookup and member invocation

No whitespace before the dot means member access:

```text
print.concat
```

The result is the `concat` member object.

Invoking it is ordinary default invocation:

```terrane
print.concat; a, b, c
```

A zero-argument method invocation remains explicit:

```terrane
buffer.clear;
```

### 9.4 Objects as arguments

A bare name passes the object it is bound to, and never invokes the expression to its left through adjacency. Calls always retain the explicit semicolon:

```terrane
print; (render; report)
```

This invokes `render` with `report`, then passes its result to `print`. It differs from:

```terrane
print.render; report
```

which invokes the `render` member of the print object. The invalid spelling `print render; report` receives a diagnostic suggesting one of those two forms; whitespace is not general function application.

An uninvoked object is passed without grouping, since a name alone never invokes:

```terrane
configure; render
```

Invoking it instead requires its own semicolon, grouped whenever anything follows: `configure; (render;), other`.

### 9.5 Positional and named arguments

An invocation has one argument list:

```terrane
callable; arguments
```

Arguments may be positional or named. Positional arguments must precede named arguments:

```terrane
request; url, timeout=5, retries=2
```

Named arguments bind by parameter name rather than position. A call must not bind the same parameter both positionally and by name.

Because `-` has no call-specific role, subtraction remains an ordinary expression in an argument list:

```terrane
print; a - b
```

Parentheses group an expression; they never create an alternative call syntax. Because invocation is introduced by `;`, these are equivalent:

```terrane
if is-enabled; config-vmap-stack
  ...

if (is-enabled; config-vmap-stack)
  ...
```

The first is canonical when the call is the whole condition. Parentheses are useful only when they delimit a call inside a larger expression:

```terrane
if (flags & mask) != 0
  ...

result = (convert; uint64, pages) * page-size
```

`if (is-enabled; ...)` is therefore supported grouping, not C-style invocation. The formatter removes redundant whole-condition parentheses and preserves parentheses that determine expression structure.

A call clause extends to the end of its containing logical expression. Commas delimit its top-level arguments, but a semicolon inside an ungrouped argument does not start a nested call: `print; format; value` is invalid. A call used as an operand or argument inside a larger expression must be parenthesised, as in `print; (format; value)` or `result = (convert; uint64, pages) * page-size`.

Grouping keeps nesting unambiguous, but it is not an invitation to nest freely. A single parenthesised call in an argument list is ordinary and reads well. Two or more in the same argument list should be bound to intermediates and passed by name instead: the nesting obscures evaluation order for a reader, accumulates parentheses that carry no meaning of their own, and gives diagnostics, traces, and debuggers an anonymous subexpression to point at where a named binding would have identified the step. The rule is a style contract rather than a grammatical restriction — deeply nested calls remain legal — and the formatter is the practical enforcement point.

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

```terrane
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

```terrane
widget = widget; 100, 50
```

### 9.8 Functions and methods are objects

A function declaration creates a callable object:

```terrane
function greet; name string
  message = ' '.concat; 'hello', name
  print; message
```

It may be passed, stored, value-assigned, reflected, or invoked through its default behaviour:

```terrane
handler = greet
handler; 'alice'
```

A selected method is also an object:

```terrane
handler = server.handle
handler; request
```

### 9.9 Static/class behaviour

Functions declared inside a class are instance methods by default.

A `static` qualifier declares a function on the class object rather than on instances:

```terrane
class widget

  static function from-config widget; config
```

Static state is state on the class object and follows the same visibility and concurrency rules as other globals/shared objects.

### 9.10 Construction and destruction

`construct` is the conventional constructor method used by a class object’s default invocation.

`drop` is the conventional deterministic destruction hook:

```terrane
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

```terrane
class widget

  function render
```

The language gets out of the way where visibility does not matter.

Explicit visibility remains available and meaningful:

```terrane
public function render
private cache = map;
protected function update-layout
```

Writing `public` is permitted as documentation even though it matches the default.

A namespace variable is the one declaration visibility cannot describe. It does not leave its
namespace tier under any marker, so `public` on one is not redundant but meaningless, and is rejected
rather than accepted as documentation.

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

The fixed declaration grammar cannot grow a keyword for every ecosystem's storage, linkage, ABI, section, calling-convention, or code-generation requirement. A package may supply modifiers instead. An imported object implementing the constrained declaration-modifier protocol is applied through a `with` clause preceding a declaration:

```terrane
from /linux/kernel import per-cpu, cacheline-aligned, weak, syscall

with per-cpu global process-counts unsigned-long = 0
with cacheline-aligned global tasklist-lock rwlock = rwlock;
with weak function arch-release-task-struct void; tsk ref task-struct
with syscall function unshare long; unshare-flags unsigned-long
```

A `with` clause introduces one or more modifiers, separated by commas and applied left to right. Each resolves through ordinary lexical scope like any other imported name, and using one without importing it is an unresolved-name error.

The comma is what delimits the clause: it means another modifier follows, so the list ends at the first element not followed by one, and the declaration begins immediately after. No wrapping parentheses are needed, and the rule holds whatever the declaration is:

```terrane
with per-cpu totals int = 0                    # one modifier, then a typed local
with per-cpu, aligned totals int = 0           # two modifiers, then a typed local
with per-cpu, aligned totals = 0               # untyped local is equally determinate
with per-cpu, aligned global process-counts unsigned-long = 0
```

A modifier taking arguments is parenthesised, which needs no rule of its own: a declaration always follows the clause, so such a call is never in trailing position and the ordinary grouping requirement applies.

```terrane
with per-cpu, (aligned; 64) global process-counts unsigned-long = 0
with per-cpu, (aligned; 64), some-other-modifier global process-counts unsigned-long = 0
```

A trailing comma is therefore an error rather than a tolerated flourish: `with per-cpu, global x = 0` reads `global` as the next modifier element and fails on a reserved word, and the diagnostic can say exactly that.

The clause is available on any declaration, including a local binding inside a function. Comma delimitation is what makes that possible: without it, a bare run of names before a binding could not be distinguished from that binding's own name and type. A modified binding therefore need not declare a type merely to remain unambiguous.

**`with` marks package-supplied modifiers only.** Core declaration words — `global`, `constant`, the visibility words, and the function qualifiers `static`, `async`, `mutating`, and `throws` — remain bare keywords and never take `with`, even though several are conceptually modifier-like. The distinction is categorical rather than stylistic:

| | Structural | Decorative |
|---|---|---|
| Examples | `global`, `private`, `constant`, `async` | `per-cpu`, `packed`, `weak`, `syscall` |
| Who supplies them | the compiler | any package, first or third party |
| Set | closed | open |
| Spelling | bare keyword | `with` clause |

The test is whether the compiler's own model can be described without it. A structural word changes name resolution, visibility, mutability, or a callable's type contract, so the model cannot be stated without it and no package may redefine it. A decorative modifier changes only how a declaration the model already understands is realised — storage placement, layout, linkage, ABI, section, alignment.

That boundary is the one this protocol already enforces below: a modifier may not affect visibility, ownership, effects, or capability. Marking the two groups differently therefore reports a real difference rather than an inconsistency, and writing `with global` would falsely suggest `global` is one of the extensible ones.

Ordering follows the layering: `with` modifiers precede the structural keywords, because the package-supplied layer is the outer one.

**Why modifiers exist.** Declaration modifiers are not primarily a substitute for C attributes, annotations, or macros. Those are motivating examples, but defining the feature in terms of them would unnecessarily constrain what packages may eventually use it for.

The underlying observation is that a declaration answers two separable questions:

1. what is being declared?
2. how does that declaration exist, or become realised, on this target or within this domain?

Terrane owns the first question through its ordinary language semantics. Modifiers provide an open-ended vocabulary for the second.

A kernel package may therefore define `per-cpu`; another environment might define per-device, replicated, persistent, externally supplied, specially placed, instrumented, or other declaration behaviours the core language has no reason to know about. Future packages may discover useful forms the language designers cannot predict.

That open-endedness is intentional. Terrane should not require a grammar change whenever a domain discovers a new meaningful property of declarations, nor should it force such properties into textual macros merely because the core compiler did not anticipate them.

A modifier is therefore better understood as a **compile-time participant in declaration realisation** than as an attribute attached to syntax. It receives a declaration the compiler already understands and may participate through the modifier protocol while remaining subject to the language's normal type, ownership, visibility, effect, capability, safety, diagnostic, reflection, and reproducibility rules.

The important constraint is not that modifiers belong to a predetermined list of purposes. It is that an unfamiliar modifier remains understandable and inspectable: tooling should be able to answer what supplied it, what declaration contract it accepts, what guarantees or requirements it adds, how it composed with other modifiers, and what lowering consequences resulted.

Stated as a single rule: **the modifier protocol is closed in its guarantees, not closed in its intended vocabulary.**

This is deliberately an extensibility mechanism with room for uses this specification does not foresee. Its success should be judged partly by whether future packages can create useful declaration concepts without requiring either new Terrane syntax or opaque source rewriting.

`per-cpu` is the motivating example. In C that concern is expressed through a mixture of attributes, linker behaviour, specialised accessors, and convention. In Terrane it can describe something stronger: an ordinary object declaration whose realisation has one instance per CPU. That distinction is why modifiers exist even though ordinary object-oriented abstraction already removes much of the need for traditional macros — **objects abstract what things do; modifiers abstract how declarations exist.**

A declaration modifier receives the declaration's typed semantic descriptor during compilation and may return a constrained transformation or attach metadata consumed by lowering. Modifiers are deferred beyond version one, and the question still open is how a modifier is *declared*, not how it is applied.

The protocol may affect only declared compiler extension points, including storage placement, linkage, exported symbol names, ABI/calling convention, alignment, target sections, generated wrappers, and checked declaration constraints. It must not replace a declaration body with hidden runtime behaviour, weaken source-visible ownership or effects, capture undeclared inputs, perform unrestricted syntax rewriting, or evade safety, capability, visibility, or type checks.

Modifier resolution, order, provenance, effects, and emitted native attributes are recorded in reflection and build metadata. Versions and consulted build inputs participate in cache keys. Unsupported or conflicting modifiers are compile-time errors.

Core declaration words determine declaration shape, and the `with` clause precedes them. Modifiers accept arguments through the parenthesised element form above; an explicit compile-time descriptor operation after the declaration remains available for metadata that does not suit a prefix position. Failed calls and modifiers are never reinterpreted as one another.

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

These descriptors are exported from `/core/types` and are constructs available without import, so a fixed-width type may be named directly:

```terrane
count int64 = 42
```

The default prelude's ordinary bindings remain exactly `print`, `int`, `float`, `bool`, `string`, `bytes`, and `none`; descriptor constructs are a separate category rather than additions to that list. Explicit import is still available where a different name is wanted:

```terrane
from /core/types import int64 as word

count word = 42
```

A descriptor bound under another name represents the same type — identity is canonical and survives rebinding. The fixed-width spellings are therefore ordinary exported names, not reserved type keywords and not hidden compiler-only names.

`float` is a spelling of `float64`, not a separate type. Both names resolve to one canonical descriptor, so a value declared `float` and a value declared `float64` are the same type in every observable respect:

```terrane
measure float = 1.5

measure.type is float64      # true
measure is a float           # true
measure is a float64         # true
```

That equivalence is what makes the shortcut safe. A parallel descriptor that merely lowered the same way would make `.type`, `is a`, reflection, and diagnostics report two types where the program has one, and the shortcut would leak wherever a type is compared rather than merely declared.

The two names carry different meanings over time. `float` denotes the default precision, whatever the language currently defines that to be; `float64` denotes binary64, pinned. In this version they resolve to the same descriptor, so the distinction is invisible in any single program — but it is what makes the default repointable at all. Code written in `float` moves with the language; code written in `float64` stays where it is because it must, being a wire format, a foreign ABI, or a binary layout, where it belongs beside `float32`. Diagnostics name the resolved descriptor, so a mismatch involving either spelling reports `float64`.

Binary64 is the default because the failure modes are not symmetric. A program that would have been fine in `float32` merely uses more memory, which profiling finds and an annotation fixes locally. A program that needed binary64 and silently received `float32` computes wrong answers: integers stop round-tripping above 2^24, so millisecond timestamps, byte counts, and money in minor units all degrade quietly, and the common interop boundaries — JSON numbers, C `double`, SQL `DOUBLE` — are 64-bit besides.

Repointing therefore follows from what `float` denotes rather than being a separate promise. Should a later version make a wider type the default — IEEE 754 defines interchange formats for any width of 128 bits or more in multiples of 32, so `float128` and beyond are standard formats awaiting hardware rather than inventions — `float` may be repointed at a version boundary, where the change is visible, opt-in, and only ever gains precision. The width-suffixed names extend additively and need no grammar change.

It must not vary by target or profile. The same source computing different results in different builds is the defect `int` avoids by being semantically fixed rather than machine-sized, and precision is the last place to reintroduce it. Note also that the destination rule makes a repoint safer here than in a language with silent conversions: every crossing to a pinned width, a foreign ABI, or a narrower buffer carries a written destination type, and each such crossing preserves its value exactly or throws. A changed default therefore surfaces at those annotated sites — as a wider exact arrival, or as a failure that names the value and destination — rather than silently altering results.

`int` is one source type, not an alias for `int64` and not a union of source-visible width types. Its values have no language-level minimum or maximum. Ordinary `int` arithmetic produces the exact mathematical result; crossing a representation boundary is internal runtime control flow, not a throw, panic, type change, or observable conversion.

The standard runtime represents `int` values adaptively: a compact `i64` fast tier, an `i128` middle tier, and arbitrary-precision signed limb storage beyond that. The erased wrapper must keep an ordinary small integer machine-word-sized where the target permits; it must not inflate every `int` to an inline 128-bit payload merely because wider values are supported. A wide tier may therefore be boxed or share a wide/big allocation header. Statically proven values may lower directly to `i64`, `i128`, or specialised limb operations without constructing the erased wrapper.

Every completed `int` operation normalises its result to the smallest tier that represents it exactly: first `i64`, then `i128`, then arbitrary precision. Thus a widened value that later falls within `int64` range becomes compact again, and a big result that fits `i128` or `i64` does not remain unnecessarily large. Tier choice is not observable through equality, ordering, hashing, serialization, source reflection, ownership, or value semantics; profiling and generated-code inspection may report it as a physical cost.

The fixed-width integer names are distinct source types whose bounds and bit widths are contractual. Their ordinary arithmetic never promotes to `int` or another width. They exist for bounded storage, predictable machine operations, layout, and ABI contracts.

### 11.2 Literals become typed objects through context

```terrane
x = 42          # int
y = 3.14        # float
enabled = true  # bool
name = my rifle # string
empty = none    # none
```

A **constant expression** is a literal, unary `-` applied to one, a parenthesised constant expression, or a compile-time-evaluable arithmetic combination of these. Its spelling classifies it as **whole-number** (`42`, `-128`) or **decimal** (`1.5`, `4.0`), but does not by itself fix a runtime type.

A constant expression is unconstrained until a destination or numeric operand supplies a type. A destination context is an initializer or assignment to a typed binding, a parameter default, an argument matched to a declared parameter type, a return expression in a function with a declared return type, or an element or field whose type is fixed by its declared container or field. In such a context the destination reaches through the whole expression and selects its numeric domain and operators:

| Destination | Constant arithmetic and admission |
|---|---|
| `int` or a fixed-width integer | Exact integer arithmetic with unbounded intermediates, including Euclidean `/`; the final value must be an integer and lie in the finite destination range where one exists. |
| `float`, `float32`, or `float64` | Binary floating-point arithmetic performed operation by operation at the destination precision, using round-to-nearest with ties to even. The result must be finite; a whole-number constant whose floating result is integral must also preserve that integer exactly. |

The rule therefore selects constant *arithmetic*, not merely a constant's representation. The same text denotes different operations in different contexts:

```terrane
x = 1 / 3               # int - Euclidean quotient, 0
x float = 1 / 3         # float - floating division, 0.333...
takes-float; 1 / 3      # float - a parameter's declared type reaches the operator
```

Thus `count int = 4.0` is `4` and `count int = 4.2` is a compile-time error. Admission tests the value a constant denotes rather than how it was spelled: `4.0` denotes the integer four, while a decimal constant in a floating destination denotes the representable value nearest what was written, which is why `tiny float32 = 0.1` is ordinary and `budget float = 9007199254740993` is not. Integer folding may accept more than the corresponding runtime expression because its unbounded intermediates cannot overflow: `limit int8 = (1000 - 900)` is valid and materialises directly as `100`. Floating folding instead reproduces runtime arithmetic at the destination precision, so `ratio float = (0.1 + 0.2)` has the ordinary binary64 result rather than exact decimal `0.3` rounded once. A constant admitted by context is emitted in the destination representation with no conversion call, runtime check, or failure path.

Declared types and compile-time constant evaluation alone decide whether source is accepted. Additional range analysis may remove a runtime check but must never make an otherwise rejected program valid; proof changes generated code, not language semantics.

For a signed fixed-width destination, range checking applies to the signed mathematical value after unary negation rather than to the positive magnitude first. Thus `minimum int8 = -128` and the corresponding minimum of every signed width are valid, while `below int8 = -129` is rejected.

```terrane
large = 9223372036854775808
wide int128 = 9223372036854775808
too-large int64 = 9223372036854775808 # compile-time range error
```

Outside every destination and operand context, a whole-number constant expression is an `int` and a decimal constant expression is a `float`. The compiler may represent scalar objects as native Rust primitives when semantics permit.

### 11.3 Dynamic bindings

A binding without a type annotation may be rebound to another type:

```terrane
x = 42
x = forty two
```

This is dynamic binding, not an untyped value.

The compiler may still infer a concrete representation over regions where the type is stable.

### 11.4 Typed bindings and definite assignment

A type expression follows the binding name. An initializer is optional:

```terrane
count int = 42
ratio float = 0.5
name string = 'alice'

cpu int
result task-struct|none
```

An initialized typed binding is immediately available. A typed declaration without `=` creates a binding with no value; it does not construct a default value, contain `none`, zero storage, or invoke the type. Every control-flow path must definitely assign a compatible value before any read, reference creation, move, member access, argument passing, or capture of that binding. Failure is a compile-time error.

A declaration's initializer resolves names against the scope as it stands immediately before that declaration. The name being declared is therefore not in scope from its own initializer. Where nothing else binds that name, reading it — directly, or indirectly through a called function — is a compile-time error naming the absent binding, rather than a read of uninitialized storage. Namespace binding initialization dependencies, including dependencies reached through called functions and later namespace-level assignments folded into initialization, must be acyclic. The compiler rejects a statically provable cycle before lowering; it must not defer the cycle to backend initialization machinery.

Where the name *is* already bound in that same lexical scope, the initializer reads the earlier binding and the declaration replaces it:

```terrane
a int8 = 12
a int = a      # reads the int8, widens it exactly, then `a` is an int
```

One name means one thing at each point in a scope, read top to bottom, which is what makes this safe to read locally. The rule is lexical: a declaration at namespace top level may not replace another, because namespace initialization is ordered by dependency rather than by source position, and a replaced namespace name would have no single answer for the declarations that read it.

Replacement of an identical type is an assignment carrying a redundant annotation, and any outstanding reference observes the new value because the storage is the same. Where the type changes, the binding's type changes with it, and the replaced value is released at the point of replacement, after its initializer has been evaluated. That release is deterministic and earlier than scope exit: nothing can name the old value again, so holding it to the end of the scope would retain a resource — a file handle, a lock — that the program can no longer reach.

Version one rejects a type-changing replacement while an outstanding reference observes the binding. Retyping a value that another scope holds a reference to would change that scope's view of it without anything explicit appearing there. See §42 for the direction this leaves open.

Function bodies and every indented control-flow body create lexical scopes. A binding declared in an `if`, `else`, loop, or other nested body is visible from its declaration through the end of that body, including deeper scopes, but not in a sibling body or after the body exits. A `for` target belongs to the loop's lexical scope: it is visible in the loop body and unavailable after the loop. Declaring a nearer name shadows an enclosing binding only until the nested scope exits; an untyped assignment to a name already found in an enclosing scope assigns that binding rather than declaring a shadow. Values owned by a nested scope are released on each exit from that scope, so a loop-body local is released on every iteration.

```terrane
cpu int

if use-current-cpu
  cpu = current-cpu;
else
  cpu = fallback-cpu

print; cpu
```

The compiler performs flow-sensitive definite-assignment analysis across branches, loops, `try`/`catch`/`finally`, labels, and `goto`. A jump may not bypass required initialization. Leaving the scope of a never-initialized binding drops nothing; once initialized, its ordinary lifetime and cleanup rules apply.

Untyped declarations without assignment do not exist: `value` alone remains an expression, not a declaration. `var` is not a declaration keyword. Initialization never requires ceremony:

```terrane
total int = 0
```

Typed destinations admit numeric values by the exact-or-throw rule in §17.7. Unrelated categories remain strict:

```terrane
count int = 4.2        # compile-time error: the constant is not an integer
name string = 42       # type error: no numeric-to-string destination conversion
```

### 11.5 Explicit coercion

Coercion is a callable method family on the source value:

```terrane
x = 42
x = x.coerce; float
x = x.coerce.checked; int8
```

The bare invocation is its throwing default. `coerce.checked` returns an absence-aware result without a representability throw; `coerce.wrap` and `coerce.saturate` are available only where the source/destination policy table defines them. A receiver is evaluated exactly once before policy selection and arguments. The complete call, including its statically resolved destination descriptor, determines whether a policy exists; selecting a family alone does not make it a freely storable bound method value in version one.

`coerce` either returns an object compatible with the requested type or throws `coercion-error`, except that a numeric destination unable to preserve the exact source value uses the more specific `integer-conversion-overflow`.

There is no universal guarantee that every type can coerce to every other type. Numeric destination contexts have their own exact-or-throw rule in §17.7; writing `coerce` is not blanket permission for an undeclared pair, but a request for a declared policy or arithmetic interpretation that may differ from the destination default.

Coercion among integer types follows §17.7 exactly. Written coercion to a floating-point destination rounds to the nearest representable value using the IEEE 754 default round-to-nearest, ties-to-even rule; because that rounding is defined for every finite source magnitude, an inexact numeric-to-float coercion is a normal result rather than a failure, and precision loss is visible through the destination type rather than through an error. This differs deliberately from an implicit numeric destination, which accepts the value exactly or throws. A source magnitude beyond the destination's finite range throws `coercion-error`; it never yields an infinity, because a silent infinity is a lost error rather than a result. `checked` returns absence for exactly that overflow case.

Floating values expose the zero-argument members `round`, `floor`, `ceiling`, and `truncate`, each producing an integer before any later destination conversion. `round` uses round-to-nearest with ties to even; the other names state their direction. These members are how an author selects a policy for a fractional floating value before an integer destination applies §17.7's exact-or-throw rule.

No floating-to-integer pair is declared on `coerce`, because choosing an integer for a fractional value requires a rounding mode and `coerce` never takes one. `ratio.coerce; int` is therefore absent from the type, while `count int = ratio` is admitted under §17.7 and `ratio.round` states the policy. This is the one place where a destination admits a conversion the written family does not offer, and it is deliberate: the destination rule is exact-or-throw and needs no mode, whereas any written alternative would have to name one.

Conversions are declared rather than universal. A descriptor declares the source/destination pairs it supports, and `coerce` attaches exactly where a declaration exists, so an undeclared pair is absent from the type rather than a runtime failure. Declaration coherence — what happens when two protocols declare the same pair, and whether a declaration may be added for a type the author does not own — is part of the conversion-protocol contract. A caller-supplied conversion callback is admitted for pairs no descriptor declares, and therefore cannot precede first-class function values.

`bool` converts to integer destinations as a declared, total, lossless conversion: `false` is `0` and `true` is `1`. The reverse is not a conversion at all. Integer-to-`bool` is a predicate choice rather than a change of representation, and must be written as an explicit comparison.

Neither the default child nor `checked` substitutes a value for a failure: an unrepresentable, unparseable, or undeclared conversion throws under the default child and returns `none` under `checked`. A total conversion that yields a fixed value on failure — `0` for an unparseable string, in the style of PHP's `intval` — is permitted only as a separately named lenient child, so the substitution is visible at the call site rather than inherited by every plain `coerce`. Such a child is optional and unspecified in version one; if it is added, its name must state that it substitutes.

Parsing coercion from `string` to a numeric destination accepts the canonical text-display spelling of that destination and throws `coercion-error` when parsing fails. `coerce` takes no argument beyond its destination and must never acquire a radix or format option: acquiring one would absorb the interpretation role that belongs to `parse`, and the separation between the two would collapse. This is an invariant of the design rather than a description of the current surface.

Interpretation in a base other than ten is a distinct operation attached by receiver: `text.radix; 16` interprets base-sixteen text and yields an adaptive `int`, while `value.radix; 16` renders a number in that base as `string`. Narrowing after interpretation is ordinary coercion and follows the call-extent rule, as in `(text.radix; 16).coerce; int8`.

Locale-dependent parsing belongs to an imported formatting facility, never to `coerce`.

### 11.5.1 User-supplied interpretation: `parse`

`coerce` covers the conversions the language defines. Interpretation the language does not define is supplied by the program through `parse`, which always takes a callback as a required argument:

```terrane
function to-code int|string; input string
  if input == 'foobar'
    return 10
  return input

d string = 'foobar'
print; d.parse; to-code
```

There is no built-in destination-owned `parse`. The member exists to apply a program's own interpretation to a receiver, so a form without a callback would have no operation to perform.

`parse` differs from every other member in where its result type comes from: `coerce; int8` is typed by its destination descriptor, whereas `d.parse; to-code` is typed by the callback's declared return, here `int|string`. That union is then checked at the destination by ordinary union rules — `value int8 = d.parse; to-code` is rejected because the `string` alternative has no numeric destination conversion — and the diagnostic is available statically from the callback's declaration. No parse-specific runtime recheck exists. A return of `int|int8` would be admitted into an `int8` destination and checked at runtime for the `int` arm under §17.7.

The `checked` child catches a callback that throws and yields absence, which plain application of the same function cannot express:

```terrane
d.parse; to-code            # propagates a throw from the callback
d.parse.checked; to-code    # int|string|none
```

In version one the callback must be a statically resolvable function name rather than an arbitrary expression. The compiler then resolves and inlines it exactly as it resolves a coercion destination, with no runtime callable representation and no boxed value. The restriction lifts when first-class function values arrive.

### 11.6 Type objects

A type is a language construct backed by a canonical object, not an independently instantiated value. An ordinary binding may not name one:

```terrane
target-type = float             # rejected: a construct is not a value to bind
```

A construct is renamed where it enters the scope, which is the import:

```terrane
from /core/types import float as target-type

x = x.coerce; target-type
```

The renamed construct may appear anywhere the construct may appear — annotation position, a coercion destination, the right side of `is a` — and it may not appear where a runtime value is required. Passing it to `print`, using it in arithmetic, or handing it to a parameter expecting a value is rejected at the source span, because a descriptor has no display or value protocol in version one.

Renaming at the import and nowhere else keeps one spelling per name in a scope: the point where a name enters is the point where it may be given another. An ordinary binding whose value is a construct would be a second, weaker aliasing mechanism, and it would read as storing a type in a variable slot — the one thing this model forbids. Holding a type in a value, to dispatch or instantiate through it, is a distinct capability that belongs with reflection and needs its own construct rather than borrowing assignment syntax.

A statically resolved construct requires no runtime storage, and its use lowers to nothing. Erasure here is not an optimisation the compiler happens to apply; where the descriptor is statically known there is simply nothing to store.

That is a statement about *ordinary value storage*, not a claim that descriptors never exist at runtime. A descriptor is a semantic object with canonical identity, and reflection or a dynamic descriptor use may require that identity to be materialised — at which point the compiler emits the canonical descriptor object rather than a variable slot holding one. The rule is that a descriptor is never an ordinary runtime value, not that it can never have a runtime representation. What is always a defect is emitting a plain Rust binding for a descriptor as if it were an ordinary value, which is what makes `d = int` lowering to `d = int;` wrong.

A class name is usable as a type expression, and is renamed the same way:

```terrane
from /models import user as user-type

person user-type = user-type; data
```

The compiler resolves type compatibility through the object’s type protocol.

Alongside the concrete descriptors, `/core/types` exports abstract category descriptors: `number`, `integer`, `fixed-integer`, `signed-fixed-integer`, `unsigned-fixed-integer`, and `floating`, beneath the two identity roots `value` and `object`. `int` implements `integer` and `number` but no fixed-width contract; `int8` through `int128` implement `signed-fixed-integer`, `fixed-integer`, `integer`, and `number`; `uint8` through `uint128` implement `unsigned-fixed-integer` in place of the signed contract; `float`, `float32`, and `float64` implement `floating` and `number`. The roots `value` and `object` classify identity, copy, and ownership behaviour rather than numeric capability, so no arithmetic or conversion member attaches to them.

These are interface and category contracts used for member attachment, compatibility, reflection, and finite-union reasoning. None of them is a storage supertype. Like the concrete fixed-width descriptors, they are descriptor constructs available without import rather than prelude bindings: the default prelude's ordinary bindings are unchanged, and a construct name is usable in construct position directly while explicit import remains available for rebinding, aliasing, and shadowing. In particular, fixed-width integers are not subclasses of `int`: an exact destination conversion does not change the source value's concrete type or the differing arithmetic result contracts.

Type objects are canonical compiler-owned descriptors with stable type identity. They are semantic objects rather than ordinary values: the backing object is real — `.type` returns it, `is a` compares it, canonical identity survives rebinding under another name, and reflection exposes it — but it is never independently constructed by source and never occupies an ordinary variable slot. Source-observable behavior must remain the same as naming the descriptor directly: `.type`, identity, compatibility queries, and operations such as `coerce` all consult the same canonical descriptor. Version one does not accept an arbitrary runtime value as a type expression or coercion destination; the value must resolve to a finite, compiler-known descriptor alternative so lowering remains statically representable.

### 11.7 Union and parameterised types

Union types use `|`. `none` is an ordinary union member rather than a special generic wrapper:

```terrane
name string|none = none
value int|float = 42.5
function parse int|parse-error; source string
```

The spelling `optional<thing>` is not part of the language: write `thing|none`. `none` is not automatically admitted into every type.
Where a destination type is a union, an exact type match wins. Otherwise the compiler selects the unique arm that admits the value under the contextual-constant or numeric destination rules. If two or more arms admit it, the destination is ambiguous and compilation fails naming those arms; source order never breaks the tie. Thus an `int8` value selects `int8` from `int8|int`, while the constant `5` is ambiguous in `int8|int32`.
A `T|none` destination is valid wherever a declared source type is valid, including binding declarations, parameter types, and function return types. It is not restricted to inferred results or compiler-owned checked operations.

The word `of` applies a parameterised type constructor using the language's fixed constructor-application grammar:

```terrane
items list of string = list;
stacks array of vm-struct|none, nr-cached-stacks
callback function from int, borrowed-ref of opaque to int
```

Packages may supply type-constructor objects, but they cannot add type-expression grammar. Every constructor argument is parsed into the same unified constructor-argument syntax node; the parser does not guess whether an identifier denotes a type or a compile-time value. Semantic analysis resolves each argument against the constructor's declared signature and reports whether a type, constant value, or other permitted compile-time object was required. Thus `array of vm-struct|none, nr-cached-stacks` can accept a type followed by a constant extent without lexer or parser knowledge of `array`.

Comma-separated arguments after `of` belong to the same type application. `|` forms a union within the current constructor argument; grouping may override the resulting structure. Angle-bracket generic spelling such as `list<string>`, `array<thing, 4>`, or `function-reference<int, void>` is not Terrane syntax.

Functions have one core type shape because functions are core objects:

```terrane
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
- weak or undeclared implicit coercion remains forbidden; numeric destination conversion follows §17.7;
- dynamic locals may still be permitted when explicitly marked or inferred under a project policy.

Strictness is local and composable. A strict package may call a dynamic package through generated checked boundaries.

### 11.10 Type checking time

A type violation should be reported at compile time when provable.

When a dynamic value crosses a typed boundary and its concrete type is not known until runtime, the generated program performs a runtime check and throws a source-language type error.

Version one reaches that runtime path in no ordinary program. Because it admits a dynamic binding only when its alternatives form a finite compiler-known set, as stated in §9.6, protocol availability and typed-boundary compatibility are decided statically across every alternative and incompatibility is a compile-time error. The runtime check exists for later erased or foreign dynamic values whose complete alternatives are unavailable at compilation.

### 11.11 Truth

Conditions use an object’s truth protocol:

```terrane
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

```terrane
false
0
''
list;
```

A typed binding rejects `none` unless its type expression includes it.

### 11.13 Equality, identity, and type membership

The language keeps three different questions separate:

- `a == b` asks whether the values are equal;
- `a is b` asks whether both expressions denote the same source-visible identity;
- `a is a type` asks whether the left operand is a value of, subtype of, or interface-compatible with the type expression.

`==` performs value equality with no unrelated implicit coercion. A type may explicitly define meaningful cross-type equality through its equality protocol, but equality never performs a hidden general conversion merely to make operands comparable.

`is` observes semantic identity only. Copy-on-write backing storage, compiler boxing, interning, and other representation sharing are not observable through it. If either evaluated operand has no source-visible identity, the result is false, even for `x is x` or two evaluations of `items[0]`. Obtaining an explicit `ref` creates or preserves source-visible identity; comparing aliases of that identity is true.

```terrane
a = list; 1, 2
b = a
c = ref a
d = c

a == b  # true
a is b  # false under value assignment
a is c  # true
c is d  # true: value assignment of a ref value preserves the referenced identity
42 is 42 # false
```

`is a` is a contextual two-word operator whose right operand is a type expression.

The parser treats `is a` as type membership only when the contextual `a` is followed by a complete type expression. At the end of an expression, or whenever no type expression follows, `a` remains an ordinary identifier and `left is a` is identity comparison against that binding. Thus `value is a serializable` is membership while `c is a` is identity. Formatters preserve the two-word membership spelling and do not rewrite identity comparisons.

```terrane
if value is a serializable
  print; value
```

For a typed left operand, `is a` asks about the type the value already has: exact concrete type, subclass, implemented interface or category, or an arm of the queried union. Destination convertibility is not membership. Therefore an `int8` value satisfies `int8`, `integer`, `fixed-integer`, and `number`, but not `int`, even though an `int` destination admits it exactly.

A numeric constant has no type before context, so `is a` supplies that context and answers whether the constant can become the named type. `42 is a int8` is true; `7828748 is a int8` and `42.5 is a int` are false rather than compile-time range errors. This asking context is the one exception that answers instead of rejecting an inadmissible constant. `isa` is not an operator: it remains available as an ordinary identifier and is less readable than the separated phrase.

The following values carry source-visible identity without requiring a new `ref` at the comparison site:

- every value participating in an explicit `ref` identity group, including the original logical value from which the reference was obtained;
- linear and other uniquely owned resource objects, such as device handles, capabilities, guards, and foreign-runtime proxies;
- canonical semantic descriptor objects whose contract defines one identity, including type, namespace, package, and declared-function descriptors.

Other ordinary values—including scalars, strings, collections, non-linear class instances, closures, and bound methods—have no source-visible identity merely because an implementation boxes, interns, caches, or shares them. Their type may expose identity only through `ref` or by declaring an inherently identity-bearing linear/resource/descriptor contract. Whether a type is inherently identity-bearing is reflected in its public type metadata and cannot vary secretly by representation or instance.

Exact runtime type is expressed through the value’s `type` descriptor. Requiring both exact type and value equality remains an explicit conjunction:

```terrane
left == right and left.type is right.type
```

The language does not define `===`. Because `==` already forbids unrelated coercion, a “strict equality” spelling would be redundant; making it secretly combine type equality and value equality would hide two independent predicates and leave subclass/interface semantics unclear.

Mutable values used as hash keys must either be rejected or use a stable immutable key projection.

---

## 12. Assignment, copying, references, and ownership

### 12.1 The central rule

> Assignment creates an independently mutable value using copy-on-write. `ref` shares mutable identity. `move` transfers ownership.

### 12.2 Value assignment

```terrane
b = a
```

has value semantics. After the assignment, mutations to `b` must not become visible through `a`, and mutations to `a` must not become visible through `b`.

This guarantee applies uniformly to ordinary scalars, strings, collections, class instances, functions, and other non-linear values. Assignment already provides independently mutable value semantics, so the source language needs no separate operation for eager duplication.

### 12.3 Universal copy-on-write

The normal implementation should share a value’s backing representation until mutation requires separation:

```terrane
a = list; 1, 2, 3
b = a
```

At this point `a` and `b` may share the same storage. Neither binding has shared mutable identity.

```terrane
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

```terrane
b = ref a
```

creates shared mutable identity for the logical value currently held by `a`.

Mutations through either identity are visible through the other:

```terrane
a = thing;
b = ref a

b.value = 10
print; a.value  # 10
```

If `a` previously received its value through ordinary assignment, creating or mutating an explicit reference must not pull other independently mutable values into the reference group:

```terrane
original = thing;
copy = original
alias = ref copy

alias.value = 10
print; copy.value      # 10
print; original.value  # unchanged
```

The implementation separates `copy` from `original` when required, while `copy` and `alias` intentionally retain shared identity.

In this draft, `ref` aliases the logical value identity, not the lexical binding slot. Rebinding `a` later does not retarget `b`:

```terrane
a = other;
```

`b` still refers to the original value.

Binding-slot aliases are deliberately not part of the core draft because they complicate closures, concurrency, and source reasoning. They may be revisited only if a compelling use case survives those costs.

### 12.5 Reference type contracts

`ref` is also a prefix type constructor for a binding whose contract requires source-visible shared identity:

```terrane
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

```terrane
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

```terrane
constant answer = 42
```

This does not necessarily make the referenced object deeply immutable.

Binding constancy is independent of the identity tier that supplied the name. A `constant`
declaration rejects every later assignment to that declaration, whether it is namespace-local,
program-global, or lexically local. A plain namespace-local declaration may shadow a distinct
program-global identity of the same name; assigning the local binding does not rebind the global
constant.

Function parameters and `for` targets are ordinary value-semantic lexical bindings. They may be
reassigned within their lexical scope, without mutating the caller's argument or the iterated
collection. Lowering therefore makes their target storage mutable only when a resolved write
actually occurs; this is an implementation detail and does not add reference semantics.

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

```terrane
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

```terrane
function main
  ...
```

Parameters follow a semicolon:

```terrane
function add; a, b
  return a + b
```

Return types follow function names, and parameter types follow parameter names:

```terrane
function add int; a int, b int
  return a + b
```

### 13.2 Optional parameters

A parameter with a default value is optional:

```terrane
function connect; host string, port int, timeout float = 5, retries int = 2
  ...
```

Calls may provide optional parameters positionally:

```terrane
connect; host, port, 10, 3
```

Named arguments are clearer when selected optional values are overridden:

```terrane
connect; host, port, timeout=10, retries=3
```

### 13.3 Variadic parameters

A parameter followed by `...` collects remaining values:

```terrane
function collect; values ...
```

Variadic values are exposed as a list-like object.

Only one variadic parameter is permitted.

### 13.4 Default values

Defaults use ordinary assignment syntax:

```terrane
function request; url string, timeout float = 5, retries int = 0
```

Default expressions are evaluated according to a declared policy:

- immutable compile-time values may be shared;
- mutable defaults must be freshly value-copied for each call;
- expressions with side effects are evaluated at call time.

This avoids Python-style shared mutable default behaviour.

### 13.5 Named arguments

Arguments may be named when the function exposes stable parameter names:

```terrane
resize; width=100, height=50
```

A call must not bind the same parameter both positionally and by name.

### 13.6 Return types

A return type follows the function name:

```terrane
function area int
  return this.width * this.height
```

A function without a return type is dynamically returning. A function with several possible return types uses a union:

```terrane
function parse int|parse-error; source string
```

A function may return `none` explicitly or implicitly at the end of its body.

Multiple logical results should normally be returned as an object. A homogeneous tuple is also
appropriate when every result has the same item type:

```terrane
return tuple; minimum, maximum
```

rather than inventing a second assignment protocol.

### 13.7 Early return

```terrane
if invalid
  return none
```

`return` without a value returns `none`.

### 13.8 Anonymous functions and closures

An anonymous function omits the name:

```terrane
handler = function; request
  return process; request
```

Closures capture outer values by value by default, following ordinary assignment semantics.

To share mutable identity with a closure, capture or assign an explicit reference before creating it:

```terrane
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

```terrane
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

```terrane
if condition
  ...

else
  ...
```

Else-if is written plainly:

```terrane
if first
  ...

else if second
  ...

else
  ...
```

No trailing colon or parentheses are required.
A direct presence guard narrows a named `T|none` binding to `T` within the guarded block. The recognized guard forms are `value != none`, `none != value`, and `not (value is a none)`, with parentheses permitted around the complete test or its operands. Narrowing is structural rather than inferred from arbitrary Boolean equivalence: combining a presence test with another condition using `and` or `or` does not establish narrowing. The fact is scoped to the guarded block and its nested scopes; assigning that name within the block invalidates the fact from that assignment onward.

### 14.2 `while`

```terrane
while condition
  ...
```

### 14.3 Collection iteration

```terrane
for item in things
  print; item
```

Destructuring is permitted when the iterator yields a matching tuple/object shape:

```terrane
for key, value in mapping
  message = ': '.concat; key, value
  print; message
```

### 14.4 Three-clause `for`

The same `for` construct supports explicit initialisation, condition, and update clauses:

```terrane
for i = 0; i < 10; i++
  print; i
```

The update may be written without `++`:

```terrane
for i = 0; i < 10; i = i + 1
  print; i
```

The parser distinguishes the two forms by `in` versus semicolon-separated clauses.

### 14.5 Increment and decrement

Postfix `++` and `--` are statement/update operations on compatible mutable numeric bindings.

They are statements, not expressions, and produce no value. `value++` updates the binding; it cannot appear where a value is required, and there is no expression form yielding the previous or the updated value.

This is deliberate. Expression-valued increment is the origin of C's read-modify-write sequencing problems, and it buys almost nothing: a program that wants the old value alongside the update writes the two operations, which is clearer than any evaluation-order rule the language could specify for the fused form. Postfix update selects the default `add` or `subtract` child of the arithmetic families (§17.6); a non-default overflow policy is written as an ordinary assignment.

The forms lower through numeric increment/decrement protocols. For fixed-width receivers they retain checked overflow behaviour unless an explicitly wrapping operation is selected; for `int` they compute the exact mathematical successor or predecessor and promote representation as necessary.

### 14.6 Loop control

```text
break
continue
```

A value-returning `break` may be considered for expression loops later, but is not required in the first implementation.

### 14.7 Labels and `goto`

Low-level control flow may name a statement position and jump to it:

```terrane
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

```terrane
match value

  case success as result
    ...

  case failure as error
    ...

  else
    ...
```

The final grammar should be validated against ordinary object/type matching before implementation.

---

## 15. Errors and exceptional control flow

### 15.1 Throwing

```terrane
throw error
```

A constructed error may be thrown directly:

```terrane
throw file-error; path
```

Any object may technically be thrown in dynamic mode. Standard tooling expects thrown objects to implement the error protocol.

All catchable failures implement a structural `error` interface carrying a stable `kind`, a human-readable `message`, an optional `cause`, and a source-context chain. `kind` is the matchable identity and is stable across releases; `message` is for humans and is not a matching key. Throwing uses a compiler-owned result propagation representation rather than native unwinding, so lowering stays deterministic and readable.

Strict mode may require an error-compatible object.

### 15.2 Catching

```terrane
try
  file = file; path
  data = file.read;

catch file-error as error
  print; error.message

catch error as error
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

The `/core/errors` namespace defines the standard error protocol and the following language-mandated error objects:

| Object | Meaning | Operations that raise it | Required information |
|---|---|---|---|
| `arithmetic-overflow` | A checked fixed-width arithmetic result is outside the receiver type's range. | Ordinary checked fixed-width addition, subtraction, multiplication, signed negation, increment/decrement, and signed `MIN / -1`. | operation and fixed-width type |
| `division-by-zero` | An integer division or remainder operation has a zero divisor. | `/`, `%`, and `div-rem` for every integer type and arithmetic mode. | operation and numeric type |
| `integer-conversion-overflow` | An exact-or-throw numeric destination cannot preserve the mathematical source value. | Implicit assignment, argument, return, element, or field conversion across numeric types; throwing `coerce` to a fixed-width integer destination; floating-to-integer conversion for a fractional, NaN, or infinite value. | source value/type, destination type, and failed exactness condition |
| `negative-shift-count` | An integer shift count is negative. | Unbounded-`int` `<<` and `>>`. | attempted count and shift operation |
| `coercion-error` | An explicit coercion has no result compatible with the requested destination, outside the integer-overflow case above. | `coerce` where the source value or text cannot be represented in the destination type, including parsing coercion from `string` and an out-of-range floating-point destination whose protocol does not declare infinity. | source value/type and destination type |

Each is a subtype or conforming instance of `error`, is catchable through the ordinary `throw`/`catch` model, and has the standard `message` plus the structured information listed above. Implementations may attach additional diagnostic fields without changing program-visible matching. Names such as `file-error`, `not-found`, `config-error`, and `python-error` used elsewhere are package- or adapter-defined error objects, not additional implicit core errors.



### 15.6 Panic

A standard panic object or operation should exist separately from `throw`.

```terrane
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
list
map
set
tuple
range
entry
```

These remain objects and are not compiler-only species.

### 16.2 Lists

A list may be constructed with ordinary invocation:

```terrane
items = list; a, b, c
```

Square-bracket syntax is recommended as compact sugar:

```terrane
items = [a, b, c]
```

On a standard UK keyboard, brackets do not violate the ordinary no-Shift ergonomic goal.

### 16.3 Maps

A map with simple textual keys may use named construction arguments:

```terrane
users = map; alice=user-a, bob=user-b
```

Computed keys use entries:

```terrane
users = map;
users.set; key-a, user-a
users.set; key-b, user-b
```

or:

```terrane
users = map; entry; key-a, user-a
```

The exact multiline entry sugar may be refined by prototype use; the object and method semantics are fixed.

### 16.4 Sets and tuples

```terrane
unique = set; a, b, c
pair = tuple; first, second
```

Tuples are fixed-length homogeneous value objects. Their applied type is `tuple of Item`; the
runtime length is not part of the type, so tuples of the same item type may cross the same binding,
parameter, and return boundaries even when their lengths differ. Tuple length cannot change after
construction.

Lists, maps, and sets are value-semantic copy-on-write objects by default.

### 16.5 Indexing

Indexing uses brackets:

```terrane
first = items[0]
value = mapping[key]
```

Assignment through an index is mutation and therefore triggers copy-on-write separation where required:

```text
items[0] = replacement
```

Lookup and indexing follow the same family convention as every other fallible operation: the default child throws and `checked` returns absence.

```terrane
value = mapping[key]                 # throws missing-key when absent
maybe = mapping.get.checked; key     # V|none
first = items[0]                     # throws index-error when out of range
```

Absence is always the `checked` spelling. No lookup returns absence by default, and there is no separately named required-lookup operation: a default that throws and a child that does not is the same shape used by `coerce` and the arithmetic families, and introducing a second mechanism for one container would make the convention unreliable everywhere else.

### 16.6 Slices and ranges

Ranges are objects:

```terrane
range = range; 0, 10
```

A concise range form such as `0..10` may be supported.

Ranges are half-open: the start is included and the end is not, so `range; 0, 10` covers `0` through `9`. An inclusive end uses an explicitly named constructor rather than a second punctuation form:

```terrane
inclusive = range.through; 0, 10     # 0 through 10
```

The step defaults to `1` and must be non-zero. A step whose direction is inconsistent with the endpoints yields an empty range rather than an error or an unbounded sequence, so a computed step cannot accidentally produce a non-terminating loop.

Slicing should use range objects rather than accumulating multiple special colon grammars:

```terrane
part = items[range; 10, 20]
```

### 16.7 Collection contracts

These contracts apply across the collection types above.

**Ordering.** Maps and sets preserve insertion order, and that order is an observable part of their contract rather than an implementation accident. Iteration, rendering, and serialisation are therefore reproducible without the program sorting defensively.

A separate unordered map and set type exists for cases where the index-map layout costs more than the guarantee is worth. It does not preserve insertion order, but it remains deterministic: a fixed hash seed means the same insertions iterate the same way on every run, in every process, and across builds. The performance option must never be the nondeterministic option, because reproducible output and comparable test evidence depend on it. Choosing it is a type choice rather than a flag, so the weaker guarantee stays visible in signatures and at every boundary the value crosses.

**Mutation results.** A mutator returns the resulting collection for value and copy-on-write collections, and `none` for an in-place mutator on a resource, unless the operation has a meaningful removed or replaced value to report.

**Copy-on-write separation.** Separation occurs at the first mutation visible through a non-unique handle, which is what preserves value-assignment semantics without copying on every binding.

**Element type inference.** A homogeneous literal infers the narrowest common declared type of its elements. A heterogeneous literal requires an explicit finite union or an annotation; the compiler does not widen silently to a dynamic element type.

**Hash keys.** Mutable values and identity-bearing resources cannot be hash keys. A key must satisfy stable equality and hash protocols for its entire lifetime, so a type whose hash could change while it is stored is rejected at compile time rather than corrupting lookup at run time.

### 16.8 Iteration protocol

`for ... in ...` invokes the iteration protocol.

An iterator's advancing operation returns a dedicated finite result, `iteration-step of Item`, with `item of Item` and `end` alternatives. The item may itself be a tuple or destructurable object.

Exhaustion is `end`, never `none`, because `none` may be a legitimate item. Iterators are stateful linear objects; `end` is sticky, and advancing after `end` returns `end` without consulting the source again. `for` desugars through this protocol and neither exposes nor synthesises a sentinel value.

The compiler may statically lower standard iterators to native Rust iterator chains.

### 16.9 String iteration

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

### 16.10 String search and transformation

String search uses literal Unicode text, not regular expressions. `contains` returns `bool`; its default child searches anywhere, while `contains.start` and `contains.end` test the logical start and end of the stored sequence. Those names are independent of writing direction: right-to-left text still starts at logical index zero. An empty pattern is contained everywhere, including at both ends.

`find` is a separate family because it returns `text-range | none` rather than a boolean. Its default child returns the first non-overlapping match, `find.all` returns a `list of text-range`, and `find.count` returns that list's length. Each range retains its immutable source and exposes checked byte, scalar, and grapheme views. For an empty pattern, `find` returns the zero-width range at the first grapheme boundary; `find.all` returns every grapheme boundary, including both ends, so `find.count` is the grapheme count plus one.
Non-empty literal search compares the stored Unicode scalar sequence and is not constrained to grapheme boundaries. For example, searching decomposed `e` plus U+0301 for `e` succeeds and returns a range ending inside that grapheme cluster; the range's checked grapheme view consequently counts the partial cluster as one. Callers that require whole-grapheme matching must compare grapheme views explicitly.

`trim` removes Unicode whitespace from both ends by default; `trim.start` and `trim.end` select one logical end. When supplied a literal argument, the selected operation removes exactly one matching prefix or suffix and otherwise returns the receiver unchanged.

`upper` and `lower` are locale-independent Unicode mappings. Their `first` children change the first cased scalar, and `upper.words` changes the first cased scalar in each Unicode word-boundary segment. Locale-sensitive casing requires an explicit policy object and never consults process locale. `case-fold` is the explicitly named locale-independent Unicode case-folding operation; search has no hidden case-insensitive child. `normalise.nfc`, `.nfd`, `.nfkc`, and `.nfkd` apply the named Unicode normalization form.

`split` and `replace` use literal patterns and return new values. A non-empty pattern is matched left to right and the next search begins after the complete preceding match, so matches do not overlap. An empty `split` pattern returns one string per extended grapheme cluster, with no synthetic empty elements. An empty `replace` pattern inserts the replacement at every extended-grapheme boundary, including both ends. These grapheme-boundary rules prevent decomposed text from being split inside a user-perceived character.

### 16.11 String composition

Two distinct members compose text. They share a subject but are not modes of one operation, so they are separate members rather than a family with children.

```terrane
'a'.concat; 'b', 'c'                    # 'abc'
': '.join; 'a', 'b', 'c'                # 'a: b: c'
```

`concat` appends its arguments to the receiver in order, with nothing between them. `join` treats the receiver as the separator and its arguments as the parts, placing one separator between each adjacent pair. This is the shape of Python's `str.join` and PHP's `implode`: the separator supplies the member rather than being passed as an argument, which reads naturally because the separator is usually a literal and the parts usually are not.

Both accept any number of arguments, and every argument is converted through canonical text display before composition. An argument whose type has no display protocol is a typed error under the ordinary display rules, not a silent rendering.

Boundary cases are specified rather than left to implementation:

| Call | Result |
|---|---|
| `x.concat;` | the receiver unchanged |
| `x.join;` | the empty string |
| `x.join; a` | `a`, with no separator |
| `x.join; a, b` | `a`, separator, `b` |

The separator never appears before the first part or after the last. `join` with a single argument therefore returns that argument's display text exactly, which makes it safe to build a list incrementally without special-casing the first element.

Neither member mutates its receiver; both return a new `string`.

### 16.12 Bytes

`bytes` is a distinct immutable sequence of octets, not a text view. A bytes literal uses the familiar `b'...'` form. Printable ASCII characters denote their octets directly; `\\`, `\'`, `\n`, `\r`, `\t`, `\0`, and `\xHH` are the only escapes. Every other escape is rejected at its source span. Bytes iterate as `uint8`, and `bytes.length` reports the octet count.

No operation silently treats arbitrary bytes as valid text. Decoding and encoding are explicit object operations:

```terrane
text = data.decode; utf8
data = text.encode; utf8
```

Version one provides the explicitly named `utf8`, `utf16-le`, `utf16-be`, `utf32-le`, and `utf32-be` encoding descriptors. `encode` is total for valid Terrane strings. `decode` validates the complete input and throws `decode-error` with a source-oriented message when an octet sequence is malformed; it never inserts replacement characters or accepts a valid prefix while discarding an invalid remainder.

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

Known numeric values lower according to their source contract rather than through a universal conversion helper. A contextual constant is emitted directly in its selected representation. An exact widening is a representation change with no representability check or conversion-error path. A checked conversion between statically known integer types narrows, widens back, compares, and branches to the failure path; implicit narrowing and equivalent written `coerce` use the same check. Range proof may remove that check. Widening to adaptive `int` chooses the smallest sufficient physical tier and may allocate only for a value that requires Big storage.

Dynamic dispatch occurs only where required by source semantics.

### 17.3 Numeric operand context and promotion

Where one operand of a binary numeric operator has a statically known type and the other is a constant expression, the constant takes the known operand's type under §11.2 and the operation uses that type's ordinary contract. Thus `iteration < 50` with `iteration int32` compares two `int32` values, and `scale * 2` with `scale float32` performs `float32` multiplication. An inadmissible constant is a compile-time error. Shift counts are exempt: they remain non-negative counts governed by §17.6 rather than values of the receiver's type.

Where both operands are integer values of different concrete types, the operation promotes them to the smallest integer type whose range contains both source-type ranges; if no fixed width does, it uses `int`. This promotion is exact and cannot throw. `int8` with `int` therefore operates in `int`; `int8` with `uint8` in `int16`; `uint64` with `int64` in `int128`; and `uint128` with a signed type in `int`. The result reaches any later destination through §17.7.

Constants remain contextual rather than pre-typed, while runtime values carry their declared types. The resulting seam is deliberate:

```terrane
counter int8 = 127
right int = 1
counter + 1          # int8 arithmetic; throws arithmetic-overflow
counter + right      # promoted int arithmetic; produces 128
counter = counter + right # promoted arithmetic, then checked int8 destination; throws integer-conversion-overflow
```

The check moves with the semantics: a constant adapts to the typed operand, two integer values promote, and a later destination checks the promoted result.

This rule does not permit arithmetic across unrelated numeric categories or other types. Integer/floating value mixtures remain rejected because they would choose a quiet approximation without a destination, as do integer/string, integer/`bool`, and integer/`none` mixtures. Such an operation requires an explicit conversion that states the intended policy:

```terrane
integer + text.coerce; int
count.coerce; float + ratio
```

### 17.4 Unbounded `int` arithmetic

Ordinary `int` arithmetic is exact and promotes only when required. Addition, subtraction, and unary negation first use checked operations in the current representation tier and continue in the next tier on representation overflow. Negating the value represented as `i64::MIN`, for example, produces positive `2^63` in the `i128` tier.

Multiplication uses an exact wider intermediate rather than losing the operands and retrying a source-level operation. The product of two `i64` values is computed exactly in `i128`; multiplication involving `i128` values uses an exact 256-bit/two-limb or arbitrary-precision intermediate; operations involving a big value use the arbitrary-precision backend. The result is then normalised. Implementations may specialise multiplication by `0`, `1`, and `-1` only when the same exactness and normalisation rules remain true.

Promotion is not implemented as a thrown `arithmetic-overflow` followed by retry. It is part of the integer operation's normal runtime path. A promotion that requires storage has an allocation effect and must be transactional: compute and normalise the new value before publishing it, leave value-semantic aliases unchanged, and leave the destination unchanged if allocation fails. Allocation failure follows the ordinary allocation-failure contract, never the `arithmetic-overflow` contract.

Bitwise operations on `int` use the mathematical infinite two's-complement model. Conceptually, nonnegative values have infinitely many leading zero bits and negative values infinitely many leading one bits; `&`, `|`, `^`, and `~` operate pointwise on that representation and return the corresponding mathematical integer. Consequently `~x == -x - 1`, `-1 & x == x`, and no finite runtime limb width is source-observable.

For `int`, `x << n` is exact multiplication by `2^n`, and `x >> n` is arithmetic right shift, equal to floor division by `2^n`, for a nonnegative `int` count `n`. A negative shift count throws `negative-shift-count`. A count that cannot be represented by the target's indexing/allocation machinery, or a left shift whose exact result cannot be materialised, follows the ordinary resource/capability failure contract rather than wrapping the count or reporting `arithmetic-overflow`. Right shift by a count at least the represented significant width yields `0` for nonnegative values and `-1` for negative values without requiring proportional allocation.

The implementation may perform these operations in `i64`, `i128`, or limb storage, but it must normalise the result and preserve the same value across representation tiers. Fixed-width bitwise operations instead operate on exactly `N` two's-complement bits and retain their declared type. Their shift-count policy must be selected explicitly by the fixed-width protocol and must never inherit host debug/release behaviour; it is not the unbounded-`int` rule above.

### 17.5 Division and remainder

Integer `/` and `%` use Euclidean division. For divisor `b != 0`, quotient `q` and remainder `r` satisfy:

```terrane
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

The standard integer protocol exposes `div-rem; divisor`, returning a named immutable `div-rem-result of T` with `quotient: T` and `remainder: T`, so an implementation need not divide twice. Both operands evaluate once and one backend operation is performed. A tuple is deliberately not used: named fields give a stable reflected result contract. `div-rem` exposes only its throwing default and `checked` — `wrap` and `saturate` are absent even on fixed-width receivers, because a wrapped or clamped quotient no longer satisfies the quotient/remainder identity the result object exists to guarantee. `/` selects the quotient and `%` selects the remainder. Division by zero throws `division-by-zero` for every integer type and arithmetic mode.

For `int`, a representation minimum divided by `-1` promotes and then normalises; it is not overflow. For a signed fixed-width type, `MIN / -1` is arithmetic overflow because the mathematical quotient is outside that type.

### 17.6 Fixed-width overflow modes

Ordinary arithmetic on `int8` through `int128` and `uint8` through `uint128` is checked. Its result has the same fixed-width type, and an exact mathematical result outside that type's range throws the standard catchable `arithmetic-overflow` error through `Result`-like control flow rather than platform unwinding. This includes addition, subtraction, multiplication, signed negation, `MIN / -1`, and any increment or decrement expressed through those operations. Unsigned negation is rejected.

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

For signed `MIN / -1`, `divide.wrap` returns `MIN`, `divide.saturate` returns `MAX`, and `divide.checked` returns `none`; `divide.overflowing` returns `MIN` with `overflowed = true`. Division by zero still throws `division-by-zero` under every policy because it is not overflow, and it is never converted into a wrapped or saturated value.

A shift count is not a numeric operand in the sense of §17.3 and never takes the receiver's type merely because it is a constant. Shifts accept a non-negative count. On a fixed-width receiver, the default and `checked` reject counts outside the width, and `wrap` reduces the count modulo the width; `saturate` is absent, because saturating a shift *count* has no coherent value contract. On `int`, `shift-left` is unbounded and total and `shift-right` is an arithmetic shift, with no count-policy children. Shift behaviour never inherits host-language debug/release behaviour.

Postfix `++` and `--` remain statements selecting the default `add`/`subtract` child only. A non-default policy is written as an ordinary assignment, `value = value.add.wrap; 1`.

The profiler and debugger identify the selected overflow mode in lowered Rust. An explicitly selected panic-on-overflow operation, if supplied by a package, is a panic and follows the target panic policy; it is not an ordinary core arithmetic mode.

### 17.7 Numeric destination conversion

Every numeric value is admitted to a single numeric destination in assignment, argument, return, declared element, and declared field contexts. The value either arrives exactly or the operation throws; it never silently truncates, rounds, wraps, saturates, changes signedness interpretation, or promotes the destination.

The compiler classifies a source/destination pair by one mechanical test: whether every value of the source type is exactly representable in the destination type. If yes, the conversion is an exact widening and lowers to a representation change with no representability check or conversion-error path. This includes fixed-width integer to `int`, range-contained fixed-width pairs such as `int8` to `int32` and `uint8` to `int16`, and integer types whose full range is exactly representable by the floating destination. Range containment, not width or signedness, decides it.

If the source type has values the destination cannot represent, the conversion remains admitted but checks the runtime value. Integer narrowing throws `integer-conversion-overflow` when out of range. Integer to floating succeeds only when that integer is exactly representable and otherwise throws `integer-conversion-overflow`: binary32 exactly covers every integer through $2^{24}$ and binary64 through $2^{53}$, but larger exactly representable multiples remain valid, so $2^{53}+1$ fails a `float64` destination while $2^{54}$ arrives. Floating narrowing likewise succeeds only when widening the narrowed result reproduces the source value; signed zero and signed infinity therefore preserve their sign and arrive, while a finite value changed by rounding and every NaN throw `integer-conversion-overflow` because narrowing cannot preserve a NaN value and payload exactly. A floating value reaches an integer destination only when finite, integral, and in range; fractional values, NaN, and infinities throw `integer-conversion-overflow`. No optimiser proof is required for acceptance, though range analysis may remove a check whose outcome is statically known.

None of these conversions creates a subtype relation. A destination supplies one unambiguous target type; mixed-operand arithmetic without one follows §17.3 instead. A union destination follows §11.7.

Written coercion remains available to request a policy different from the destination default:

```terrane
value.coerce; T             # canonical declared conversion; numeric-to-float may round
value.coerce.checked; T     # T|none
value.coerce.wrap; T        # destination-width modular reduction
value.coerce.saturate; T    # clamp to destination bounds
```

For integer destinations, bare `coerce` has the same exact-or-throw result as an implicit destination conversion. `checked` returns `T|none`; `wrap` reduces the mathematical value modulo `2^N` and interprets the resulting bits using the destination signedness; `saturate` clamps to the destination bounds. Therefore `-1.coerce.wrap; uint8` is `255`, `255.coerce.wrap; int8` is `-1`, and `300.coerce.saturate; uint8` is `255`. Wrapping and saturation exist only for fixed-width destinations.

Written integer-to-floating `coerce` deliberately requests IEEE round-to-nearest, ties-to-even, as specified in §11.5; the implicit destination form is exact-or-throw. Floating values expose `round`, `floor`, `ceiling`, and `truncate`, each yielding an integer; `round` uses ties-to-even. These members state how a fractional value should become integral before an integer destination receives it.

Lowering materialises a contextual constant directly in the destination representation, emits an unchecked representation change for exact widening, and emits a direct narrow/widen-back comparison for a checked conversion whose source and destination are statically known. An implicit narrowing and an equivalent written `coerce` must generate equivalent checks. Exact widening from a fixed-width integer to adaptive `int` selects storage from the source range: `int8` through `int64` and `uint8` through `uint32` fit the Small tier; `uint64` through `int128` fit the Wide tier; and `uint128` uses Wide below $2^{127}$ or Big otherwise. A Big conversion may allocate, whose failure follows the ordinary allocation contract, but it cannot throw a conversion error. Flat spellings such as `checked-coerce`, `wrapping-coerce`, and `saturating-coerce` are not language syntax.

---

## 18. Classes, interfaces, traits, and inheritance

### 18.1 Fields

Fields are ordinary object bindings declared in class scope:

```terrane
class request

  method string = 'GET'
  path string = '/'
  body bytes|none = none
```

Fields are public by default and may be narrowed:

```terrane
private cache = map;
protected state = none
```

### 18.2 Inheritance

Single class inheritance is supported:

```terrane
class secure-request extends request
```

Multiple class inheritance is not part of the core language.

The compiler may lower inheritance through generated composition, enums, trait objects, or static specialisation. Source semantics must not depend on Rust having class inheritance.

Assigning a subclass instance to a superclass-typed binding preserves the complete dynamic object and its subclass state. Subsequent value assignment copies that complete dynamic value under the ordinary COW contract; Terrane never slices to the statically named superclass fields. A superclass annotation constrains the visible interface and accepted dynamic classes, not storage layout. Targets unable to represent the permitted dynamic class set without an unavailable capability reject the boundary at compile time rather than changing this rule.

### 18.3 Interfaces

Interfaces describe required object protocols:

```terrane
interface serializable

  function serialize bytes
```

A class declares implementation:

```terrane
class message implements serializable
```

Interfaces are type objects and can be used in annotations.

### 18.4 Traits

Traits provide reusable behaviour:

```terrane
trait timestamped

  created-at = none

  function touch
    this.created-at = clock.now;
```

A class may use traits:

```terrane
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

```terrane
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

```terrane
global cache = shared-map;
```

or explicitly wrap one.

### 20.4 Thread-local globals

A standard thread-local object/facility should be provided rather than special-casing a second global declaration grammar.

Target profiles without threads reject it.

### 20.5 Build-time selection

`when build` selects declarations or statements from immutable build configuration:

```terrane
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

```terrane
async function fetch response; url string
  return await client.get; url
```

`async` marks a function whose invocation returns a task/future object.

### 21.2 Await

```terrane
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

```terrane
error: this value requires heap allocation

  buffer = dynamic-list;

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

```terrane
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

```terrane
pointer = pointer; address, type=int32
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

### 23.1 The dependency principle

One rule governs every ecosystem below, and each subsequent subsection is a specialisation of it rather than a separate model:

> **Dependency declarations name ecosystems and packages, not APIs. The build resolves the exact package and generates only the boundary machinery that Terrane source actually crosses. Tooling projects an advisory Terrane-visible surface, which is never compiler-authoritative.**

Three consequences follow, and they apply uniformly to Rust crates, system libraries, and foreign runtimes.

**Resolution is the source of truth.** A declaration names `serde` or `numpy`; it does not describe what those packages contain. The manifest, lockfile, selected features, target, and toolchain determine the interface that exists for a given build. Nothing in the language predefines it, because a predefined surface would be a second, weaker copy of the ecosystem's own type system, guaranteed to drift.

**The build bridges only what is crossed.** Boundary machinery is generated for the specific calls, types, and values that Terrane source actually touches. A dependency is not projected wholesale into Terrane objects. This keeps generated output proportional to use, keeps compile times bounded by the program rather than by the dependency, and avoids committing the language to representing constructs it has no equivalent for.

**Tooling advises; it does not define.** The language server may read package metadata, rustdoc output, or runtime introspection to offer completion, signature help, hover text, and documentation. That projection is advisory: it never alters compiler output, never invents members, and is never the authority on whether a program compiles. The authority is the ecosystem's own toolchain — Cargo and rustc for Rust, the C compiler and linker for system libraries, the runtime for a hosted language.

Tooling must also not execute arbitrary package code merely to inspect it, and its cache identity must include everything that can change the resolved interface: manifest contents, lock checksum, enabled features, target triple, toolchain version, and source checksums.

### 23.2 Four dependency origins

The package system supports:

1. native Terrane packages;
2. Rust crates;
3. system libraries, ordinarily exposed through C ABI metadata or a wrapper;
4. foreign-runtime packages hosted through an explicit runtime adapter.

Example manifest/source declarations:

```terrane
use image-tools
use rust serde
use system libjpeg
use runtime python
```

A native package is the default dependency kind.

### 23.3 `use` versus `from ... import`

`use` declares a build dependency.

`from ... import` brings object symbols from an available namespace into source scope.

```terrane
use image-tools

from /image/tools import resize
```

The distinction is intentional:

- dependency graph composition is not the same operation as name binding;
- installing a package must not automatically pollute source names.

### 23.4 Package contents

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

[namespaces]
"example/tools" = "src"
"example/generated" = "generated"
```

`package` is a required non-empty package identity. `namespaces` is a required,
non-empty table from canonical namespace roots to distinct relative directory
roots; absolute paths, paths containing `..`, duplicate directory roots, and
roots containing no `.trn` source are invalid. `prelude` is an optional boolean
and defaults to `true`. Unknown fields are rejected. The loader recursively
discovers `.trn` files only beneath those roots and gives source units stable
file identities in sorted package-relative path order. Every source declaration
must match the namespace derived from the longest directory mapping and the
file's relative parent directory. A direct `.trn` CLI input is instead an
implicit one-unit package with identity `single-file`, the default prelude, and
no directory-correspondence check.

A package may expose one coherent object namespace regardless of which implementation language supplies each object.

Consumers should not need to know whether `resize` is implemented in source, generated Rust, handwritten Rust, or a C library wrapper.

### 23.5 Locking and reproducibility

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

### 23.6 Generated Cargo project

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

### 23.7 Build scripts

Declarative build metadata is preferred.

Arbitrary build scripts are powerful and therefore capability-gated.

The build report must identify packages that executed code during compilation.

### 23.8 Rust crates

A Rust crate dependency is declared with:

```terrane
use rust crate-name
```

This adds a locked dependency to the generated Cargo graph. It does not describe the crate, and the compiler does not predefine an API surface for it.

The resolved package, version, selected features, target, and lockfile determine the native interface available to a given build. Inline Rust and maintained Rust modules use that interface directly, in Rust, with Rust's own types. Cargo and rustc type-check it, and build diagnostics are source-mapped back to the dependency declaration or the native Rust span that produced them.

The compiler does **not** project a crate into high-level Terrane objects. There is no generated adapter layer, no translation of Rust generics into instantiations, and no mapping of Rust traits, lifetimes, or error types into the Terrane model. Those constructs remain in Rust, where they are already well defined, and Terrane source touches them only inside a native Rust body. A projection layer would be a second, weaker copy of Rust's type system that drifts with every crate release, and it would make the compiler responsible for representing constructs the language has no equivalent for.

A Terrane-visible wrapper is therefore something an author writes deliberately when they want one, not something the build produces automatically. Writing it is ordinary work: a Rust module that exposes a boundary the Terrane side crosses, with the ownership and error contracts stated at that boundary.

Editor intelligence follows §23.1: the language server reads Cargo metadata and lock data for resolved package, version, feature, and target facts, and delegates Rust semantic questions to rust-analyzer over the exact generated and native module graph. Rustdoc output is a documentation fallback, never a second type checker. Everything it offers is advisory — lowering, Cargo, and rustc remain the authority on what compiles.

### 23.9 System and C libraries

```terrane
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

### 23.10 C++

Arbitrary C++ ABI integration is not a version-one goal.

C++ libraries should initially be consumed through:

- an existing C API;
- a small C-compatible shim;
- a handwritten Rust bridge.

### 23.11 Exporting back to Rust and C

Language packages should be able to expose stable Rust and C APIs.

The compiler generates:

- Rust modules/types/functions;
- C ABI wrappers where requested;
- headers;
- ownership and error conventions;
- symbol metadata;
- versioned ABI descriptors.

This makes the language embeddable rather than a one-way consumer.

### 23.12 Native interop versus foreign runtimes

Rust is Terrane’s canonical lowering language. Inline Rust and maintained Rust modules inhabit the generated program and may use its documented native representations directly. System/C libraries cross an ABI boundary but do not introduce another language runtime.

A foreign runtime is different:

```terrane
use runtime python
```

declares that the program hosts a subordinate runtime with its own object model, allocator or garbage collector, exceptions, module loader, concurrency rules, and deployment requirements.

The distinction is constitutional:

- `rust` is a native lowering escape hatch;
- `python` is foreign-runtime execution;
- a runtime adapter must not silently replace Terrane typing, assignment, error, thread, or ownership semantics with the foreign language’s semantics.

Python is the first foreign runtime and the first adapter implementation. The initial adapter targets the CPython `libpython3` embedding API. Other adapters, such as Lua or JavaScript, may be added later through the same contracts; they are not version-one requirements.

### 23.13 Runtime imports and Python objects

After declaring the runtime dependency, Python modules may expose bindings:

```terrane
use runtime python
from python/numpy import array

values = array; 1, 2, 3, 4
mean = values.mean;
```

`from python/...` names a crossing point rather than importing an API. It does not project the module into Terrane, and the compiler holds no model of what `numpy` contains. The build generates boundary machinery for exactly the members the program crosses, and the language server projects an advisory surface from runtime introspection so an author can discover what is available — advisory in the §23.1 sense, never the authority on whether the program builds.

Attribute lookup and invocation use ordinary Terrane member syntax, but the semantic descriptor records a foreign transition, and resolution of those members happens against the runtime that is actually present at build time rather than against a static declaration in the language.

A Python object proxy is a Terrane object whose implementation, mutable identity, and lifetime belong to CPython. Reflection must identify that fact rather than presenting it as a native value:

```text
foreign
runtime python
foreign-type numpy.ndarray
```

Foreign proxies are identity-bearing resources, not ordinary COW values. They cannot be value-assigned unless an adapter exposes a specific value-copy contract. Sharing one requires explicit `ref`; transferring an exclusive proxy uses `move`. Calls may borrow a proxy without transferring it. This prevents Python aliasing from silently weakening Terrane’s ordinary assignment rule.

An adapter may expose a native Terrane wrapper with normal COW semantics when it can genuinely preserve those semantics—for example, through a verified immutable value or buffer-backed representation.

### 23.14 Embedded foreign source

An indented runtime block executes foreign source:

```terrane
python
  import numpy as np

  x = np.array([1, 2, 3])
  print(x.mean())
```

The compiler preserves and source-maps the block, while the Python adapter compiles and executes it through `libpython3`. Values enter or leave only through an explicit adapter interface; lexical bindings are not implicitly shared with the block.

Unlike inline Rust, an embedded Python block is never inserted into generated Rust as native code. Tooling must label it as foreign runtime execution and account for every transition.

### 23.15 Conversion and zero-copy data

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

```terrane
py-values = values.coerce; python.list
```

Large data must have standard zero-copy paths where representation and lifetime permit them. The Python adapter should support the Python buffer protocol first and may add DLPack and Arrow adapters. A zero-copy bridge must pin or otherwise preserve the producer’s storage, declare mutability and element layout, and reject incompatible lifetimes rather than copying silently.

Build explanations and profiling must report whether a boundary conversion borrowed, wrapped, pinned, or copied data.

### 23.16 Errors, lifetime, and threads

A Python exception becomes a Terrane `python-error` preserving:

- the Python exception type and message;
- the formatted Python traceback;
- the original Python exception object while its runtime remains alive;
- the Terrane source location and boundary operation;
- a causal chain when wrapped or rethrown.

```terrane
try
  result = python-object.do-thing;

catch python-error as error
  print; error.message
  print; error.python-trace
```

Crossing the boundary may acquire the CPython GIL, allocate in Python’s managed heap, execute arbitrary Python code, and trigger Python finalisers. The adapter owns reference-count transitions and interpreter shutdown ordering. Foreign finalisation must not be described as deterministic Terrane destruction when CPython cannot provide that guarantee.

The compiler and runtime must reject unsupported cross-thread use rather than silently adding locks or moving a proxy between interpreters.

### 23.17 Runtime adapter contract

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

An adapter is boundary machinery, not a translation of the foreign ecosystem into Terrane. It defines how a crossing behaves — representation, lifetime, conversion, error and thread rules — and the build instantiates only the crossings a program actually contains. It does not enumerate, mirror, or typecheck the foreign package's API, which remains the responsibility of the foreign runtime and its own tooling. This is the same division §23.1 states for Rust: the ecosystem owns its interface, the build owns the boundary, and tooling advises across it.

### 23.18 Deployment contract

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

```terrane
function checksum uint64; data bytes
  rust
    checksum_impl(data)
```

The indented block is preserved as Rust after stripping its common source indentation.

The compiler inserts it into the generated Rust function and maps its spans back to the source block.

### 24.3 Inline Rust expression

A Rust block used as an expression returns its final Rust expression:

```terrane
result int = rust
  native_calculation()
```

The compiler checks that the Rust result can cross back into the declared/source object type.

### 24.4 Inline Rust in classes

```terrane
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
ipv4-ipv6   -> __terrane_ipv4_x2d_ipv6
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
terrane eject-rust /image/codec resize
```

This copies a generated implementation into a maintained native Rust module, adds the appropriate bridge metadata, and replaces source generation for that object.

The operation must be explicit, reviewable, and reversible only through source control or a deliberate migration.

---

## 25. Reflection

### 25.1 Reflection is a core service

A language in which everything is an object requires a coherent way to inspect those objects.

Reflection should be accessed through a normal object:

```terrane
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

```terrane
info = reflect; my-function
print; info.compile.rust
```

returns the generated Rust corresponding to that build.

A live frame may be inspected:

```terrane
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

```terrane
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
terrane explain /image/codec resize
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
4. namespace and import resolution;
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

```terrane
x = 42
x = x + 1
```

may lower directly to `i64` where range analysis proves the fast representation sufficient. Where runtime widening is possible, generated code uses an `i64` hot path with a cold exact-promotion path into the `i128` tier and then arbitrary-precision storage. It must not model promotion as a source throw or re-evaluate operands after detecting representation overflow.

The erased `int` representation keeps the small case compact and boxes or otherwise out-of-lines wider payloads rather than imposing an inline `i128` size on every value. Arithmetic helpers normalise completed results back through `i128` to `i64` whenever exact bounds permit. Equality, ordering, and hashing operate on the mathematical value across all tiers and must produce identical answers for equal values reached through different representations.

**Known finite alternatives**

```terrane
if condition
  x = 42
else
  x = 'unknown'
```

may lower to a generated enum.

**Open dynamic value**

A value crossing an open plugin/reflection boundary may lower to a boxed/tagged dynamic object.

**Typed contract**

```terrane
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

```terrane
message = ' '.concat; a, b, c
print; message
```

have a stable semantic lowering:

```terrane
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
Terrane source warnings are non-blocking diagnostics: they are reported by the compiler but do not
change the success of `check`, `rust`, `build`, or `run`. Backend warnings remain denied for
compiler-owned and generated Rust. Conformance manifests may name an expected-warning file; when
they do, warning code, source-relative span, severity, message, order, and multiplicity are matched
exactly.

Binding-use analysis is resolved by declaration identity and recorded once per semantic unit, so
shadowing does not merge unrelated bindings and later lowering does not repeatedly scan whole
syntax trees. `W4001` reports an initialized local binding whose value is never read. `W4002`
reports an initial or later assignment whose stored value cannot reach a subsequent read before a
definite replacement. Conditional stores do not by themselves kill the incoming value. Parameters
do not receive unused-binding warnings: an unused parameter can be required by a callable contract,
and parameter-name linting belongs to a later explicit policy rather than these local-store
diagnostics. Loop targets likewise remain outside `W4001`; generated Rust explicitly consumes unused
loop targets, dead stores, and other warning-only locals so source-level warnings do not leak into
opaque `rustc` warning failures.


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

```terrane
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

```terrane
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

```terrane
print.concat
print; concat
foo+bar
foo + bar
count - 1
```

It must canonicalise every parsed infix expression to one space around its operator and must never insert spaces inside an identifier. One-sided operator spacing is rejected rather than guessed. Formatting `x=foo+bar` produces `x = foo+bar`; `x=count-1` is rejected and may be fixed explicitly to `x = count - 1`.

The formatter must reject or loudly expose ambiguous/non-canonical spacing.

### 31.3 Language server

The language server should expose:

- completion over the single lookup view, including imported and prelude names;
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

```terrane
error: whitespace does not invoke print

did you mean:
  print.concat
to select print's concat member?

or:
  print; (concat; value)
to invoke concat and pass its result to print?
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

This is normative EBNF for the covered core forms. Names in capitals are layout tokens emitted by the lexer. Lexical terminals such as `letter`, `lowercase-letter`, `digit`, `literal`, and the opaque foreign/Rust/text bodies are defined by their dedicated sections. `namespace-segment` is given inline below because it is a strict subset of `identifier` rather than a reuse of it. Semantic restrictions and layout notes remain outside the machine-readable fence.

```terrane
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
  = "+" | "-" | "*" | "%" | "<" | ">"

comment
  = line-comment
  | block-comment

line-comment
  = ( "#" | "//" ) { non-newline-character }

block-comment
  = "/*" { character except the terminating sequence "*/" } "*/"

namespace-declaration
  = "namespace" namespace-segment { "/" namespace-segment }

namespace-path
  = [ "/" | relative-prefix ]
    namespace-segment { "/" namespace-segment }

relative-prefix
  = "../" { "../" }

namespace-segment
  = lowercase-letter { [ "-" ] ( lowercase-letter | digit ) }

dependency-declaration
  = "use" package-name
  | "use" ( "rust" | "system" ) package-name
  | "use" "runtime" runtime-name

package-name
  = namespace-segment

runtime-name
  = identifier

foreign-source-block
  = runtime-name indented-foreign-body

from-import
  = "from" namespace-path "import"
    object-import { "," object-import }

object-import
  = identifier [ "as" identifier ]

importer-selection
  = [ "global" ] "import" "with" identifier

visibility
  = "public" | "private" | "protected"

modifier-clause
  = "with" modifier-element { "," modifier-element }

modifier-element
  = identifier
  | "(" identifier ";" argument-list ")"

binding
  = [ modifier-clause ]
    [ visibility ] ( "global" | "constant" )
    identifier [ type-expression ] [ "=" expression ]
  | [ modifier-clause ] visibility
    identifier [ type-expression ] [ "=" expression ]
  | [ modifier-clause ]
    identifier [ type-expression ] [ "=" expression ]

class-declaration
  = [ modifier-clause ]
    [ visibility ] [ "linear" ] "class" identifier
    [ "extends" type-expression ]
    [ "implements" type-expression { "," type-expression } ]
    indented-body

function-declaration
  = [ modifier-clause ]
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
  | update-statement
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

update-statement
  = assignment-target ( "++" | "--" )

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
  | update-statement
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
    { "." identifier | "[" expression "]" }
    [ call-clause ]

primary-expression
  = identifier
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

`assignment-target` is syntactically a primary followed only by member or index operations. Semantic analysis accepts a mutable bare binding, or a member/index path whose final operation implements assignable storage. It rejects literals, calls, postfix updates, temporary values without assignable storage, and any path forbidden by ownership, borrow, visibility, or COW-pinning rules. Every receiver and index is evaluated exactly once.

A bare `identifier = expression` is the ordinary assignment form: it initializes a new binding when declaration is permitted and no binding resolves, otherwise it rebinds the resolved mutable binding. Visibility, declaration modifiers, `global`, `constant`, and an uninitialised declaration always use `binding`, so `private cache = map;` is structurally unambiguous.

Each function qualifier may appear at most once, and incompatible combinations are rejected semantically. The recursive operator production permits conventional combinations such as `not -value`, while `ref`, `move`, and `await` consume a postfix operand and therefore reject accidental forms such as `ref ref value` and `move move value`. Unary `+` is not a core operation.

The `is a` alternative is selected only when `a` is followed by a complete `type-expression`; otherwise the comparison alternative treats `a` as an ordinary identifier. `call-free-expression` is the expression grammar instantiated with the optional `call-clause` on `postfix-expression` disabled. This parameterisation avoids duplicating every precedence production; parser-generator sources must expand it mechanically. A parenthesised `expression` re-enables calls, which is why nested invocation requires grouping.

The semantic resolver checks the first component of a `from` path against declared runtime names before native namespace resolution. Thus `from python/numpy import array` is syntactically an ordinary `from-import`, but resolves through the adapter introduced by `use runtime python`. A runtime name at statement position begins an opaque, indentation-delimited `foreign-source-block`; its adapter owns the nested grammar and source map.

The parser emits every `constructor-argument` as one unified syntax-node kind because identifiers and other forms may resolve as types or compile-time values. Constructor signatures classify those nodes during semantic analysis. Function types associate to the right; grouping overrides that association.

Postfix/member operations bind most tightly, followed from high to low by prefix operators, multiplicative, additive, shifts, bitwise AND, XOR, OR, comparisons, identity/type membership, logical AND, and logical OR. Binary arithmetic, shift, bitwise, `and`, and `or` operators associate left. Comparisons are non-associative: `a < b < c` is invalid and must be written as `a < b and b < c`. Prefix operators associate right. A postfix call clause applies to the complete postfix expression immediately to its left.

Operands and call arguments evaluate strictly left to right. Member receivers evaluate before member selection; an assignment target's receiver and indices evaluate once, left to right, before the assigned value; `and` and `or` short-circuit; all other listed binary operators evaluate both operands. Default argument expressions evaluate at the call site after supplied arguments have been evaluated, in parameter order. The compiler may reorder only when it proves that source-observable values, effects, throws, mutation, destruction, and reference/COW separation are unchanged.

`print.concat` is a member expression. `print concat` is invalid because adjacency is not invocation. `>native executable` is a tail string when `>` appears in expression-start position. An exact `>>` followed by a newline opens an indented block string. Neither text form is admitted as a non-final ungrouped subexpression.

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

```terrane
call; a - b, (convert; value)
```

The semicolons separating a three-clause `for` are owned by that statement, so calls within its clauses are likewise grouped.

---

## 35. End-to-end examples

### 35.1 Namespace-local output override

```terrane
namespace my-app

from /my-output import print

function main
  print; >Hello! From, "Terrane"!
```

Only `my-app` and descendants see this `print` unless it is promoted globally.

### 35.2 Program-global output override

```terrane
from mylib/tools import myprint
global print = myprint
```

Ordinary global lookup of `print` now resolves to `mylib/tools`’ function. The core implementation has no sacred claim on the binding and remains available through `from /core/output import print`.

### 35.3 Custom importer

```terrane
namespace plugins

from /build/importers import sandboxed-import
import with sandboxed-import

from third-party/plugin import plugin
```

The final import is resolved by `sandboxed-import`.

### 35.4 Exact-or-throw numeric destination

```terrane
function read-count int; ratio float
  count int = ratio
  return count
```

An integral finite value arrives exactly; a fractional, infinite, or NaN value throws `integer-conversion-overflow`. Writing `ratio.round` states a rounding policy instead.

### 35.5 Value, ref, and move

```terrane
a = list; 1, 2, 3

b = a
# b is independently mutable; storage may initially be shared by cow

c = ref a
# c shares a's identity

handle = device-handle;
worker-handle = move handle
# handle is unavailable
```

### 35.6 Both forms of `for`

```terrane
for item in things
  print; item

for i = 0; i < 10; i++
  print; i
```

### 35.7 Error handling

```terrane
function load bytes; path string
  try
    file = file; path
    return file.read;

  catch not-found as error
    throw config-error; error

  finally
    trace; load complete
```

### 35.8 Inline Rust hot path

```terrane
function checksum uint64; data bytes
  rust
    fast_checksum(data.as_slice())
```

Callers do not care that the implementation is handwritten Rust.

### 35.9 Reflection and Rust inspection

```terrane
info = reflect; checksum

print; info.source
print; info.compile.rust
print; info.compile.rust-type
```

### 35.10 Embedded Python

```terrane
use runtime python

from python/numpy import array

values = array; 1, 2, 3, 4
print; values.mean;

python
  import torch
  tensor = torch.tensor([1, 2, 3])
  print(tensor.sum())
```

The imported NumPy array is a foreign proxy with explicit runtime reflection. The embedded block crosses the same visible Python boundary and retains Python source locations for errors and debugging.

### 35.11 Kernel-oriented code

```terrane
namespace kernel/memory

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

```terrane
count int = 42
```

Possible Rust:

```rust
let mut count: i64 = 42;
```

### 36.2 Potentially widening `int`

Source:

```terrane
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

```terrane
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

```terrane
message = ' '.concat; a, b, c
print; message
```

Possible conceptual Rust:

```rust
let message = " "concat([
    a.as_text(),
    b.as_text(),
    c.as_text(),
])?;
print.call(message)?;
```

The real compiler may inline the concatenation or stream directly when source-observable behaviour permits.

### 36.5 Value assignment with COW

Source:

```terrane
b = a
```

Possible Rust:

```rust
let mut b = CowValue::share(&a);
```

The profiler still reports one semantic assignment and zero physical copies until separation.

### 36.6 Explicit reference

Source:

```terrane
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
/core/types
/core/output
/core/errors
/core/reflection
/core/collections
/text/formatters
/text/encoding
/system/files
/system/process
/system/memory
/system/time
/system/observability
/network
/concurrency
/testing
```

The prelude imports or binds only a very small subset.

Standard APIs should follow the same object conventions as user packages. Compiler magic must be limited to facilities that cannot be expressed otherwise.

This section fixes the shape only. The object contracts of individual version-one facilities — document formats, URLs, paths and filesystem access, streams, process and environment access, date and time, networking and TLS, randomness and digests, compression, and logging — are mapped in `docs/surface-v1.md`, which carries them at the level of members, return types, and failure modes. That document is a proposal rather than normative prose; where it and this specification disagree, this specification governs.

Those facilities are written in Terrane over the minimal Rust core, per §5.7. The standard library is therefore an ordinary set of packages rather than a privileged layer, and it uses the same dependency mechanism as any other package when it needs a Rust crate beneath it.

---

## 38. Implementation sequencing

The normative language design does not duplicate the compiler's operational roadmap. Implementation milestones, ordering, deliverables, and validation commands live in [the compiler plan](compiler-plan.md). This specification constrains that plan through the semantics and invariants stated here; changing milestone order does not change the language contract.

---

## 39. Prototype acceptance tests

The first serious prototype should prove all of these:

Unless a snippet explicitly tests unresolved lookup, the conformance harness supplies the imports named by that snippet's fixture. Prose examples outside the harness must either show their imports or state the standard namespace from which omitted objects come; imported names are never implicitly added to the prelude.

1. `value.concat` parses as member lookup, while `value .concat` is rejected; a leading `.` is always an error, since `.` appears only between a receiver and its member.
2. `namespace my-output/formatters` and `from /my-output/formatters` resolve symmetrically.
3. `/` anchors the root and separates every segment; `../` and `../../` ascend one and two tiers; an uppercase or reserved segment is rejected with a diagnostic naming the correction.
4. `/` is the namespace separator and never an identifier character; `ipv4 / ipv6` is division and `ipv4-ipv6` is the identifier form.
5. `a+b`, `a + b`, `a+ b`, and `a +b` respectively tokenise as an identifier, an addition, an undeclared-postfix error, and an addition; `count-1` is a lexical error suggesting `count - 1`, while `sha256` remains an identifier.
6. `foo+bar`, `foobar`, and `fooplusbar` resolve independently and map injectively to distinct valid Rust identifiers.
7. `../foo` resolves one tier upward.
8. importing `print` binds `print` in the scope containing the import, and `as` binds it under a different name.
9. `from /core/output import print as emit` binds `emit` namespace-locally.
10. `global print = my-print` replaces the program-global binding.
11. `import with custom-import` changes subsequent import resolution in its namespace, `global import with custom-import` selects the program fallback, and an ordinary binding named `import` changes neither.
12. `#`, `//`, and `/* ... */` comments lex and format without changing indentation structure.
13. an unterminated block comment fails at its opening delimiter, and an unused string is never treated as a comment.
14. quoted, tail, and indented block strings preserve their specified content deterministically.
15. typed scalars lower to native Rust primitives.
16. dynamic finite alternatives lower without a universal heap object.
17. contextual constants select the arithmetic of typed destinations and operands; numeric destination conversion widens exactly or checks and throws, while explicit checked, wrapping, saturating, and rounding policies obey their contracts.
18. value assignment prevents mutation leakage.
19. COW avoids a physical copy until mutation.
20. `ref` preserves shared identity.
21. nested COW values separate on mutation without leaking changes.
22. a foreign proxy requires explicit `ref` or `move` rather than weakening value assignment.
23. a Python import resolves through `libpython3` and exposes a reflected foreign proxy.
24. Python exceptions retain their traceback in a `python-error`.
25. both `for` forms compile.
26. throw/catch lowers without ordinary panic.
27. a Rust error is mapped back to the source span.
28. inline Rust sees source values through documented generated names.
29. a function’s generated Rust is retrievable in a development build.
30. profiling distinguishes semantic assignment, shared storage, physical copy, COW split, ref, and move.
31. profiling exposes Python transitions and data copies.
32. a simple allocator-free target rejects hosted-only capabilities at source level.
33. `==`, `is`, and `is a` respectively test value equality, source-visible identity, and type membership; a numeric constant uses the queried type as context and returns false rather than failing when inadmissible, while a typed numeric value is not a member of a merely convertible concrete type; exact type-and-value comparison uses an explicit conjunction and `===` is rejected.
34. labels are function-local; `goto` cannot enter a deeper lexical scope or cross initialisation/lifetime transitions unsafely, and every accepted jump lowers to sound Rust with identical cleanup order.
35. `when build` selects namespace declarations and function statements deterministically, excludes inactive branches from the current build, and records every selection input in the build cache key.
36. `ref T`, `borrowed-ref of T`, `user-ref of T`, `raw-address of T`, `array-ref of T`, `c-pointer of T`, and `function from ... to ...` enforce distinct identity, lifetime, address-space, provenance, extent, and ABI contracts without implicit conversion between them.
37. `with per-cpu, (aligned; 64) global x int = 0` applies two package-supplied modifiers resolved through ordinary lexical scope; the comma delimits the clause, an argument-taking modifier is parenthesised, a trailing comma is an error, and `with global` is rejected because core declaration words never take `with`.
38. `constant` declarations parse in every binding position and `const` is rejected as a declaration word.
39. `array of vm-struct|none, nr-cached-stacks` parses as one constructor application whose signature classifies its first argument as a type and its second as a compile-time integer.
40. `function from int, c-pointer of opaque to int` associates to the right; nested callable parameters format with grouping whenever the ungrouped form would be difficult to scan.
41. `void` is accepted only as the no-produced-value contract, while `opaque` is accepted as a type with hidden representation; neither substitutes for the other.
42. a borrow derived through member access or collection iteration retains the origin borrow's anonymous provenance and cannot escape or widen its inferred lifetime.
43. reflection reports source name, generated Rust name, and native symbol independently, and `native-name; mmdrop, "__mmdrop"` changes only the last.
44. lexical ownership and acyclic strong references destroy deterministically, while a provable strong cycle is rejected and an uncollectable runtime cycle is diagnosed or documented as a leak rather than promised deterministic reclamation.
45. imports obey lexical and namespace scope, nearer imports shadow farther ones, same-scope collisions are rejected, and `as` retains both objects when two exports collide.
46. plain top-level assignment remains namespace-local even in the root namespace; creating or replacing a program-global binding without `global` is rejected.
47. the default prelude contains exactly `print`, `int`, `float`, `bool`, `string`, `bytes`, and `none`; disabling it removes those defaults while explicit `/core` imports still work.
48. a call owns its remaining logical expression, nested calls require grouping, zero-argument calls require `;`, and three-clause `for` semicolons cannot be consumed as call delimiters.
49. source type parameters are rejected; strict code uses concrete types, unions, interfaces, or generated concrete declarations rather than silently becoming dynamic.
50. `c is a` parses as identity against the binding `a`, `c is a widget` parses as type membership, ordinary identity-less values compare false even to themselves, explicit refs alias one identity, and linear resources preserve identity across moves.
51. core text display renders supported scalar values canonically, `print` consumes that protocol and appends a newline, arbitrary `bytes` and values without text display are rejected rather than guessed, and locale-sensitive or styled formatting remains explicitly imported.
52. an interior `ref` separates COW storage, remains attached to its original logical owner, pins the referenced path, and rejects removal, replacement, escape, or lifetime widening while live.
53. exported may-throw functions expose `throws`, non-throwing callable contracts reject may-throw implementations, fixed-width checked arithmetic throws a catchable `arithmetic-overflow`, `int` representation promotion does not throw, and explicit wrapping operations do not.
54. assigning a subclass value to a base-typed binding preserves the complete dynamic value and dispatch; implementations that would slice are rejected.
55. protocols express structural capabilities, interfaces define typed dispatch boundaries, traits reuse implementation without becoming types, and single inheritance preserves value and dynamic-type semantics.
56. only declared precompiled host extensions execute as importers or modifiers; `when build` accepts only its restricted deterministic query subset, records inputs and plans in cache keys, and never recursively executes ordinary Terrane source.
57. an `async function` has an async callable type, `await` is rejected outside async context, sync and async callables are incompatible without an explicit adapter, and no borrow crosses suspension unless its contract proves that lifetime.
58. default `string.length` requires grapheme segmentation capability; a target lacking it diagnoses the operation instead of substituting scalar or byte length, while explicit scalar/byte views remain available.
59. representation specialisation may inspect only a package compilation unit and declared dependency metadata; downstream packages consume the published representation contract rather than changing upstream layout.
60. precedence, associativity, comparison non-associativity, short-circuiting, receiver/index evaluation, assignment-target evaluation, argument order, and default-argument order match §34 exactly under both interpreted tooling and generated Rust.
61. `private cache = map;`, `protected state = none`, bare rebinding, member assignment, and index assignment parse; literals, calls, postfix updates, non-assignable temporaries, and ownership-invalid paths are rejected as assignment targets.
62. every statement form in §34 parses with empty and non-empty bodies where allowed; `else`, `catch`, `finally`, and `case` bind only to their owning constructs, and `return`, loop control, throw, yield, labels, and jumps preserve required cleanup.
63. unary `-`, `~`, and `not` compose according to precedence; unary `+`, `ref ref value`, and `move move value` are rejected.
64. unconstrained integer literals beyond `int64` and `int128` range remain `int`; runtime addition, subtraction, and negation promote exactly from the compact tier through `i128` to arbitrary precision without a source-visible overflow.
65. completed `int` operations normalise back to the smallest exact tier, including an `i128`-tier value crossing into `int64` range and a big value producing a small result; equality and hashing remain identical across every tier.
66. multiplying two small `int` values uses an exact `i128` intermediate, wider multiplication produces the exact arbitrary-precision result, and multiplication by `0`, `1`, or `-1` preserves promotion and normalisation edge cases.
67. signed `/`, `%`, and `div-rem` obey the Euclidean quotient/remainder invariant for every sign combination; division by zero throws `division-by-zero`, `int` division promotes for a representation `MIN / -1`, and fixed-width `MIN / -1` follows its selected overflow mode.
68. every signed and unsigned fixed width through 128 bits keeps its declared type under arithmetic and implements throwing ordinary, checked, wrapping, saturating, and overflowing operation contracts without build-mode-dependent behaviour.
69. contextual constant expressions are evaluated in destination or typed-operand arithmetic, admitted by mathematical value rather than literal spelling, materialised directly in the selected representation, and rejected at compile time when the selected domain cannot represent the result.
70. mixed integer values promote exactly to the smallest integer type containing both source ranges, while integer/floating value mixtures and unrelated categories remain rejected without an explicit policy conversion.
71. numeric destination contexts admit exact widening without a representability check or conversion-error path and checked narrowing with `integer-conversion-overflow`; floating/integer crossings succeed implicitly only for exactly representable values, optimiser range knowledge may remove checks but never decide source validity, and widening to adaptive `int` may retain an ordinary allocation effect.
72. `coerce`, `coerce.checked`, `coerce.wrap`, and `coerce.saturate` handle signedness and every `int`/fixed-width boundary exactly; written integer-to-float `coerce` rounds ties-to-even while an implicit float destination is exact-or-throw; flat spellings such as `checked-coerce` are rejected.
73. `int` bitwise operations behave as infinite two's-complement arithmetic across positive and negative operands and every representation tier; `~x == -x - 1`, left shift is exact, right shift is arithmetic/flooring, negative counts throw `negative-shift-count`, and very large right shifts produce `0` or `-1` without count wrapping or proportional allocation.
74. contextual signed fixed-width destinations accept each type's syntactically negated minimum literal, including `-128` as `int8` and `-2^127` as `int128`, reject the next lower value, and do not first reject the unsigned positive magnitude.
75. fixed-width numeric descriptors are constructs available without import, distinct from the seven prelude ordinary bindings; explicit import remains available for aliasing and shadowing, and they are not reserved type words.
76. canonical type descriptors are semantic objects with stable identity rather than ordinary values, requiring no runtime storage when statically resolved and materialising only where reflection or dynamic descriptor use demands it, while a first-version type expression or coercion destination must resolve to a finite compiler-known descriptor alternative.
77. numeric-to-float coercion rounds to nearest with ties to even and reports precision loss through the destination type rather than an error, unrepresentable float destinations and unparseable text throw `coercion-error`, and parsing coercion accepts exactly the destination's canonical text-display spelling.

---

## 40. Deliberate validation points

The architecture is coherent enough to implement, but these details should be tested in real code before being frozen.

### 40.1 Zero-argument invocation shorthand

The current draft treats:

```text
thing
```

as object lookup and:

```terrane
thing;
```

as zero-argument default invocation/construction.

A prototype should test whether zero-argument class construction deserves a safe shorthand without making imported singleton/function objects ambiguous.

### 40.2 Map literal syntax


`map` construction and methods are semantically sufficient.

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

### 40.9 Numeric arrival diagnostics

The exact-or-throw destination rule creates two useful diagnostics whose semantics are fixed but whose final surface should be tested against real code. A typed numeric value needs a predicate asking whether this value would arrive exactly in a destination; the proposal spelling is `value.fits; Destination`, but whether this remains a member or becomes a contextual operator is not frozen. A statically false `is a` on a typed numeric operand should lint with the categories that operand does implement, and constant integer division that discards a nonzero remainder should lint at the operator.

The name `integer-conversion-overflow` should also be revisited there. It now covers the whole exact-or-throw rule, including a fractional or non-finite floating value reaching an integer destination and an integer too precise for a floating one — neither an integer destination nor an overflow. One error for one rule is the right shape, so the semantics are settled and only the spelling is open; a rename is a coordinated change across this document, the compact reference, and existing conformance cases.

The wording, severity defaults, and stable `T00xx` codes for contextual-constant rejection and these lints should be assigned with their first conformance cases rather than guessed in advance.

---

## 41. Core invariants

The following are the design’s constitutional layer. They govern the entire document and override conflicting illustrative prose, examples, lowering sketches, or implementation plans:

1. Everything is an object semantically.
2. Runtime representation is free to be non-object-shaped when behaviour remains identical.
3. Values have types even when bindings are dynamic.
4. Dynamic typing never implies weak or unrelated implicit coercion.
5. Type constraints are optional, local, and real.
6. Numeric constants take the arithmetic of their destination or typed operand; numeric values cross a single declared destination exactly or throw, while written coercion remains object-driven and selects alternative policies.
7. Ordinary assignment has value semantics.
8. Ordinary values may share backing storage, but mutation separates them before changes become observable elsewhere.
9. `ref` is the visible shared-identity operation.
10. `move` is the visible ownership-transfer operation.
11. Imports bind ordinary names in the scope containing them; lexical scope, not a second spelling, is what keeps them from leaking.
12. There is one lookup view. `.` appears only between a receiver and its member, never as a name prefix.
13. Namespace segments are `/`-separated, lowercase `[a-z]([a-z0-9]|-[a-z0-9])*`, and a strict subset of `identifier`.
14. `/` is both the root anchor and the segment separator, and is never an identifier character.
15. Operator-bearing identifiers and spaced infix expressions are lexically distinct and formatter-protected.
16. `foo.bar` is member lookup and `foo; bar` passes `bar` as an argument; whitespace adjacency never invokes, and a leading `.` is not a name form.
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
40. Package-supplied modifiers are introduced by `with` and are available on any declaration including a local binding; core structural words remain bare keywords.
41. Package-defined type constructors classify a common constructor-argument syntax as type or compile-time value without extending the parser grammar.
42. `void` means no produced value and never acts as erased storage; `opaque` names unavailable representation, whose reference contract must still identify ownership, lifetime, address space, and operations.
43. Every derived borrow retains compiler-assigned provenance and may preserve or narrow, but never widen, the origin lifetime.
44. Source names, generated Rust names, and native ABI/link symbols are independent reflected identities.
45. Deterministic destruction is guaranteed by lexical ownership and acyclic final strong-reference release, not by arbitrary strong-cycle reachability.
46. `int` denotes an exact arbitrary-precision signed value with compact adaptive representation; representation overflow promotes and completed results normalise, fixed-width arithmetic alone exposes width overflow, and numeric destination conversions preserve the exact mathematical value or throw.

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
- additional foreign-runtime adapters governed by the same explicit boundary contracts as Python;
- type-changing replacement of an observed binding. Version one rejects replacing a binding's type while an outstanding reference observes it, and the same exclusion covers a closure capture once closures exist. What a later version might offer is an explicit way for the holder to accept the new type, so the retyping appears in the scope that is affected by it rather than only in the scope that caused it. Silently retyping through a reference is not a candidate.

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
