# Strata first-version compiler plan

## 1. Purpose

Build the first usable Strata compiler as a source-to-Rust toolchain. The compiler must accept a deliberately bounded, coherent subset of Strata, produce readable and deterministic Rust, invoke Cargo/rustc, and return diagnostics in Strata source terms.

This plan is for an executable compiler, not another language-design prototype. Every milestone must finish with source programs that are compiled and run through the real pipeline.

`demos/` is explicitly outside the compiler conformance contract. Files there are exploratory pressure tests and may intentionally combine unfinished syntax, package adapters, kernel facilities, unsafe contracts, or speculative semantics. They must not be used as smoke tests, parser fixtures, acceptance criteria, or examples of what the current compiler is expected to build. Compiler development will create purpose-built test programs under `tests/` and runnable examples under `examples/` instead.

## 2. First-version outcome

The first version is complete when a user can:

```text
strata check path/to/program.strata
strata run path/to/program.strata -- program-arguments
strata build path/to/program.strata
strata rust path/to/program.strata
```

and the toolchain can compile and run a small but nontrivial command-line program using:

- a namespace declaration;
- namespace-local bindings and functions;
- core scalar values;
- quoted, tail, and indented block strings;
- arithmetic, comparisons, and Boolean conditions;
- explicit semicolon calls, member invocation, and dot-objects passed as ordinary argument values;
- positional and named arguments, including optional parameters;
- `if`/`else`, `while`, collection and three-clause `for`, and `return`;
- the exact version-one default prelude and standard output facility;
- deterministic generated Rust and a Cargo project;
- source-oriented lexer, parser, resolver, type, and backend diagnostics.

The first version does not need classes, universal dynamic values, copy-on-write collections, `ref`/`move`, exceptions, custom importers, third-party packages, foreign runtimes, async, reflection, inline Rust, `no_std`, embedded targets, or kernel targets. Syntax for deferred features may be recognized only when doing so enables a precise “not supported in this compiler version” diagnostic; it must never be accepted and lowered incorrectly.

## 3. Delivery principles

1. **Tests define implemented behavior.** The design draft informs the implementation, but an executable conformance case is required before a feature is considered supported.
2. **No dependency on `demos/`.** CI must neither compile nor parse files from `demos/` unless a future, explicitly named demo-specific job is introduced.
3. **Vertical slices before breadth.** Establish `source -> Rust -> Cargo -> executable` early, then expand the language through end-to-end slices.
4. **One semantic path.** `check`, `run`, `build`, and `rust` share the same frontend and semantic pipeline. Commands must not grow separate parsers or validators.
5. **No silent repair.** Invalid Strata is rejected at its source span. The compiler must not reinterpret failed syntax as a nearby construct merely to continue.
6. **Deterministic output.** The same source, compiler version, target, and declared inputs produce byte-identical generated source and manifests.
7. **Readable lowering.** Generated Rust is a public debugging surface, not opaque compiler debris.
8. **Narrow runtime.** Statically known fixed-width scalars and functions lower directly to Rust types and calls where Rust preserves the complete Strata contract; core `int` uses the narrowest exact representation required by its adaptive semantics. The first compiler must not introduce a universal boxed `Value` as a shortcut.

## 4. Proposed repository layout

```text
compiler/
  Cargo.toml
  crates/
    strata-cli/
      Cargo.toml
      src/
        main.rs
    strata-compiler/
      Cargo.toml
      src/
        diagnostics.rs
        source.rs
        lexer.rs
        tokens.rs
        syntax.rs
        parser.rs
        ast.rs
        names.rs
        resolver.rs
        types.rs
        semantics.rs
        lower.rs
        rust_names.rs
        rust_emit.rs
        cargo.rs
        source_map.rs
        prelude/
      tests/
        unit/
        conformance/
          accept/
          reject/
          parse/
          resolve/
          lower/
          run/
        fixtures/
examples/
  hello.strata
  word-count.strata
  build-report.strata
```

The first compiler and its CLI should be implemented in Rust. This gives the project one distributable executable, exhaustive phase models, direct integration with Cargo diagnostics and any support crates, and no later frontend rewrite boundary.

Use mature Rust parsing tooling rather than treating Rust as a requirement to hand-write every frontend component. Chumsky is a strong initial candidate: it supports separate character and token parsers, token-associated spans, recursive combinators, Pratt expression parsing, rich errors, and recovery. Prototype Strata's hardest lexical and grammatical boundaries with it before freezing the parser architecture. Keep Strata tokens, syntax nodes, spans, and diagnostics compiler-owned so replacing or selectively bypassing the parsing library would not change the language model.

Do not create a general runtime crate before an implemented feature requires one. Core `int` is the first such feature: introduce a small support crate with its first semantic/lowering slice for adaptive exact integers and their normative failures. Keep other statically known values on direct Rust lowering and add support only for behavior that generated code cannot express cleanly.

## 5. Test corpus design

### 5.1 Fixture contract

Each conformance case is a directory or manifest entry containing only the artifacts relevant to its assertion:

```text
case.strata     # single-source input
package.toml    # optional package manifest for multi-source cases
case.toml       # phase, expected status, entrypoint, arguments
stdin.txt       # optional exact input
stdout.txt      # optional exact output
stderr.txt      # optional exact diagnostic or uncaught source-runtime error
exit-code.txt   # optional exact exit code; defaults to zero for accepted runs
parse.json      # optional normalized syntax shape
resolve.json    # optional symbol-resolution facts
lower.rs        # optional canonical generated Rust
```

`package.toml` is the authored package contract exercised by milestone 3; `case.toml` remains test-harness metadata and points to it when present. Runtime-failure fixtures must provide both `stderr.txt` and `exit-code.txt`.

Golden files must be reviewed output, not snapshots accepted blindly. Unstable data such as temporary paths is normalized by the test harness before comparison.

### 5.2 Test layers

- **Lexer unit tests:** tokens, trivia, indentation transitions, spans, UTF-8 boundaries, comments, strings, identifier/operator attachment, and malformed input.
- **Parser conformance tests:** accepted and rejected syntax plus compact normalized trees.
- **Resolver tests:** namespace paths, root/parent anchors, ordinary versus object-form lookup, scopes, duplicate names, and unresolved names.
- **Semantic tests:** type compatibility, call binding, control-flow validity, definite return, and unsupported-feature diagnostics.
- **Lowering goldens:** readable Rust for small constructs, including exact source identity comments or map entries where applicable.
- **Corpus scale:** expect hundreds or thousands of minimal snippets, each isolating one lowering decision and comparing canonical Rust byte for byte.
- **Compile tests:** batch independent accepted snippets into deterministic generated crates for `cargo check`; compile cases individually when crate structure, linking, or diagnostics are part of the contract.
- **Run tests:** purpose-built Strata programs execute and produce exact output and exit status.
- **CLI integration tests:** command arguments, exit codes, output locations, and diagnostic behavior.
- **Differential invariants:** `check` and `build` accept or reject the same source; `rust` uses the same semantic model; formatting or comments do not alter runtime behavior.

### 5.3 Initial real programs

Create these incrementally rather than borrowing from `demos/`:

1. **hello:** import/bind output, define `main`, print exact text.
2. **build-report:** kebab-case bindings, typed integers and strings, receiver-based string concatenation, named arguments, and output. Because fixed-width names are `/core types` descriptor objects rather than prelude bindings, the authored file either uses `int` or writes the explicit descriptor import and ordinary binding for each width it names.
3. **fizz-buzz:** arithmetic, comparisons, `if`/`else`, a loop, function calls, and return.
4. **word-count:** command-line arguments, string iteration or splitting, a standard collection, mutation, and deterministic formatted output. Add only after milestone 4 selects and implements an explicit grapheme/scalar/byte iteration contract and a collection subset whose mutation preserves version-one value semantics.
5. **multi-file greeting:** two namespaces, explicit object import, ordinary binding, and deterministic module lowering.

Each program becomes a permanent end-to-end regression test. Examples should demonstrate only released behavior and must build in CI.

## 6. Architecture contracts to freeze early

### 6.1 Source and spans

- Assign every source file a stable file ID within a compilation.
- Store byte offsets as the canonical span representation and derive line/column lazily.
- Preserve trivia in the lossless syntax layer even if the semantic AST discards it.
- A diagnostic owns a primary span, message, stable diagnostic code, optional labels, notes, and help.

### 6.2 Syntax model

Use three layers:

1. immutable token stream with trivia and indentation tokens;
2. lossless concrete syntax tree for formatting/tooling;
3. compact semantic AST with source spans for resolution and lowering.

The parser must recover at statement and dedent boundaries so one error does not turn a file into noise, but recovered nodes may never reach lowering as valid constructs.

### 6.3 Names

- Preserve exact Strata spelling as symbol identity.
- Maintain ordinary names and object-form names as distinct lookup views.
- Encode Rust identifiers with one deterministic, injective algorithm shared by declarations, references, source maps, and tests.
- Never normalize punctuation away: `foo+bar`, `foobar`, and `fooplusbar` remain distinct.

### 6.4 Semantic model

Every resolved expression records:

- source span and stable node ID;
- resolved symbol or builtin operation;
- static type or finite dynamic alternatives;
- value category needed by lowering;
- selected call target and argument binding;
- control-flow facts relevant to reachability and returns.

The first version may reject a dynamic construct whose finite representation cannot yet be proven. It must explain that limitation rather than emit a universal runtime representation silently.

### 6.5 Backend boundary

Lower the semantic model to a small Rust-oriented IR before rendering text. The IR should represent modules, items, blocks, expressions, types, calls, and source associations without containing formatting decisions. Rust emission then becomes deterministic pretty-printing rather than semantic analysis embedded in string concatenation.

## 7. Milestones

### Milestone 0 — Toolchain skeleton and executable corpus

Deliver:

- Rust workspace, `strata` CLI executable, and compiler library;
- compiler version reporting and structured exit codes;
- isolated temporary/build directories;
- conformance harness supporting accept, reject, Rust golden, compile, and run cases;
- automatic Rust toolchain prerequisite check;
- initial `hello` accepted fixture and several rejected placeholders that fail with an explicit unsupported-stage diagnostic;
- CI commands that run compiler tests without traversing `demos/`.

End-to-end proof:

```text
strata rust tests/conformance/run/hello/case.strata
strata build tests/conformance/run/hello/case.strata
<generated executable>
```

At this milestone the frontend may support only the exact constructs needed by `hello`, but the source must travel through the real token, syntax, semantic, lowering, Cargo, and execution boundaries. Do not implement `hello` by source-text substitution.

Exit criterion: one purpose-built Strata file produces a real executable and exact expected output; malformed input fails through the diagnostic framework.

Implementation note: milestone zero names the intended pipeline boundaries, but its bootstrap frontend is deliberately not yet structurally separated. Its `lex` stage records logical lines rather than tokens, import and binding forms are recognized as exact supported lines, unresolved-object detection remains parser-local, and the current resolve/lower boundaries mostly transfer fields. Milestone one therefore builds the real tokenizing lexer rather than extending a complete lexer, and later milestones make resolution and typed lowering substantive.

### Milestone 1 — Lexer and indentation correctness

Deliver:

- UTF-8 source validation with an explicitly versioned ASCII-only identifier character policy for the first compiler;
- tokens with exact spans and retained trivia;
- `NEWLINE`, `INDENT`, and `DEDENT` generation;
- blank lines and comment-only lines that do not perturb indentation;
- `#`, `//`, and `/* ... */` comments;
- quoted, tail, and indented block strings plus numeric literals;
- identifiers with operator-bearing joiners, including `<` and `>`, while a terminal joiner followed by a digits-only unit is rejected;
- comparison and shift operators using `<`, `>`, `<<`, and `>>`, with `>` and `>>` additionally opening tail and block strings in expression-start position; these tokens never delimit generic arguments;
- structural punctuation and spacing-sensitive operator attachment, including `++`/`--` as declared postfix tokens;
- lexical diagnostics for mixed tab/space indentation styles, invalid characters, unterminated strings/comments, inconsistent dedents, illegal attached operators, and attached joiner-plus-digits forms such as `count-1` with a spaced-expression fix.

Required conformance boundaries include:

```text
ipv4/ipv6
ipv4 / ipv6
a+b
a + b
a+ b
a +b
print.concat
print .concat
count-1
-einval
list<string>
list<string>= x
value===other
```

The lexer must tokenize `value === other` and `value===other` consistently as `==` followed by structural `=`, and must tokenize `list<string>` without treating angle brackets as generic delimiters. Both angle spellings must tokenize deterministically even though they produce different trailing tokens: a bare trailing `>` where whitespace or a delimiter follows, and a single `>=` token where `=` follows immediately. Milestone 2 owns the contextual rejection and fixes for every one of these spellings.

Indentation cases must cover consistently space-indented and consistently tab-indented files, a mixture within one indentation prefix, and a style change between different code lines in one file.

Exit criterion: lexer corpus covers every token class and malformed boundary; all diagnostics point to the originating bytes and remain correct for multibyte UTF-8.

Implementation status (completed on the `indentation-lexer` capability branch):

- the shared compiler pipeline uses compiler-owned tokens, trivia, byte spans, and lexical diagnostics before the bootstrap parser;
- the lexer emits structural newline and indentation transitions, retains whitespace and all three comment forms, and decides text markers, comparisons, and shifts from the preceding token rather than from line text;
- tokens, trivia, and indentation transitions cover every source byte exactly once: a block string token spans its marker and body, and one terminator ends the statement it completes;
- only lines carrying source outside comments participate in indentation, so blank lines, comment-only lines, and multiline comment terminators never open or close a block;
- §6.8 numeric literals, `&`/`^`/`~`, and the identifier joiner set are lexed as declared, and a malformed literal is reported across its whole run instead of splitting into a name;
- lexer contracts cover every token class, each required boundary spelling, all four indentation cases, and byte-accurate diagnostics including multibyte input;
- the milestone-zero logical-line parser remains only as a temporary semantic projection for the runnable hello slice; milestone 2 replaces it as the authoritative syntax parser.

Lexical diagnostics own the `L` code range and are the sole reporter of every condition listed here; the bootstrap parser keeps the `S` range for the value-level rules it still owns:

```text
L0001 invalid source character        L0006 illegal left-attached operator
L0002 unterminated block comment      L0007 unterminated string literal
L0003 indentation style               L0008 block string marker not final
L0004 inconsistent dedent             L0009 invalid numeric literal
L0005 joiner-introduced digit unit
```

The parser now owns grammar-defined continuation and recovery decisions. Blank and comment-only lines continue to emit terminators as part of the lossless lexical contract.

### Milestone 2 — Lossless parser and formatter-ready tree

Deliver:

- namespace declarations;
- namespace-local bindings, typed bindings with and without initializers, visibility modifiers, `global`, and `constant`;
- function declarations and parameter lists;
- block statements and legal empty blocks;
- literals, names, object-form lookup, member access, calls, unary/binary expressions, assignment, grouping, and postfix `++`/`--`;
- `if`/`else`, `while`, collection and three-clause `for`, `return`, `break`, and `continue` syntax;
- parser recovery at newline and dedent boundaries;
- normalized parse-tree serializer for goldens;
- explicit unsupported-feature nodes or diagnostics for reserved/deferred constructs, including source-declared type parameters, `===` with an explicit equality/type-identity fix, and angle-bracket generic intent in type position with a canonical `list of string` fix, recognized from both the bare trailing `>` and the `>=` spelling.

Highest-risk ambiguities must receive dedicated tests before broad grammar work:

- `print.concat` versus invalid `print .concat` adjacency;
- `.thing` versus the explicit zero-argument call `.thing;`;
- tail-string markers versus comparison and shift operators;
- operator-bearing identifiers, prefix negation, postfix `++`/`--`, and spaced operators;
- namespace whitespace tiers versus expressions;
- call semicolon precedence, named arguments, and grouping of nested calls;
- grouping calls inside the clauses of a three-clause `for`;
- `is a` as identity against an ordinary binding versus type membership when `a` is followed by a complete type expression.

The delivered tree must also preserve sufficient type-expression structure for later resolution of core names, explicitly imported descriptor bindings, union members, constructors, and finite descriptor alternatives without treating fixed-width names as parser keywords.

Invalid adjacency and missing grouping must produce source-oriented diagnostics with valid explicit-semicolon and parenthesized-call fixes; the parser must never repair them silently.

The parser must implement the normative §34 precedence and associativity table, including non-associative comparisons, and mechanically expand the call-free-expression variant used by argument grammar rather than maintaining a second expression grammar.

Exit criterion: every first-version construct has accepted and rejected parse cases; no semantic decision is required merely to recover the intended tree shape.

Implementation status (completed on the `lossless-parser` capability branch):

- the shared `check`, `rust`, `build`, and `run` pipeline now parses authoritative lexer output through one recursive-descent parser before the temporary hello semantic projection;
- compiler-owned syntax nodes retain byte spans, token ranges, child structure, the complete token stream, and trivia, with a deterministic normalized serializer suitable for reviewed goldens;
- declarations, imports, bindings, functions, parameters, legal empty blocks, control flow, assignment clauses, names, object lookup, literals, member/index/postfix expressions, calls, grouping, and the normative unary/binary precedence ladder have dedicated tree shapes;
- the same expression parser implements call-permitted and call-free contexts, including named arguments and grouped nested calls, without a second grammar;
- type nodes preserve unions, prefix forms, constructor application, function types, and ordinary descriptor names for later semantic resolution;
- newline and dedent recovery keeps subsequent statements structurally available, while invalid member adjacency, chained comparisons, ungrouped nested calls, `===`, and angle-bracket generic intent produce source-oriented `S` diagnostics;
- focused accepted and rejected cases cover the milestone grammar, highest-risk ambiguities, structural imports, malformed declarations, recovery boundaries, and exact normalized output; the full workspace suite and a real CLI hello run verify the shared pipeline.

The temporary semantic projection below the syntax tree remains deliberately limited to the milestone-zero runnable hello program. It does not parse independently or bypass syntax diagnostics, and milestone 3 replaces it while adding package, namespace, import, and scope semantics.

Parser diagnostics own the stable `S1xxx` range:

```text
S1001 unexpected layout token             S1017 malformed object lookup
S1002 malformed namespace declaration     S1018 unclosed grouped expression
S1003 missing binding name                S1019 missing expression
S1004 missing binding initializer         S1020 malformed function type
S1005 missing `function` keyword          S1021 unclosed grouped type
S1006 invalid function header content     S1022 missing type expression
S1007 malformed parameter                 S1023 missing block newline
S1008 malformed three-clause `for`        S1024 unterminated indented block
S1009 malformed collection `for`          S1025 trailing statement content
S1011 value on a value-free statement     S1026 malformed `from` import
S1012 chained non-associative test         S1027 malformed importer selection
S1013 invalid member adjacency            S1028 malformed collection target
S1014 missing member name                 S1029 invalid declaration modifier
S1015 unclosed index expression           S1030 assignment in condition
S1016 unparenthesized nested call         S1090 reserved unsupported syntax
S1091 unsupported `===`                   S1092 unsupported angle generic
```

`S1010` is intentionally unassigned. Diagnostics whose correction is not
fully expressed by the primary message carry structured help; CLI rendering
prints that help separately from the stable code and message.

The original milestone branch routed the pipeline through the parser before
its broad coverage commit. Review follow-ups added focused regression cases
alongside each correction. Later language work must continue to introduce its
accepted and rejected cases in the same vertical work unit as the behavior.

### Milestone 3 — Namespaces, scopes, and bootstrap environment

Deliver:

- a minimal package manifest contract and loader that enumerate the complete source-unit set and select whether the default prelude is enabled;
- a single-file CLI input modeled as an implicit one-unit package with a stable package identity and the default prelude, without filename-to-namespace inference or on-demand namespace search;
- namespace tree assembled from the complete manifest-enumerated set of package source units before resolution;
- deterministic multi-file discovery and source-unit assembly order;
- exact root `/` and parent `..` anchoring;
- separate ordinary and object-form symbol tables, with lexical object-form lookup;
- namespace-local, function-local, parameter, and program-global scopes needed by the first version;
- explicit `global` handling for program-global creation/replacement and rejection of plain top-level assignment where a global operation is required;
- duplicate, shadowing, visibility/inaccessibility, unresolved-name, and same-scope object-form collision diagnostics;
- idempotent reimport of the same object-form export, with aliases required for distinct colliding exports;
- fixed bootstrap importer whose milestone-3 module table registers versioned `/core output`, `/core types`, `/core errors`, and `/collections` namespaces as structural compiler-owned modules rather than runtime calls; milestone 3 populates the first three, including all fixed-width numeric descriptor objects under `/core types`, while milestone 4 populates `/collections` with its selected collection subset;
- the exact default prelude bindings `print`, `int`, `float`, `bool`, `string`, `bytes`, and `none`;
- import resolution that does not create an ordinary binding automatically, and proof that an ordinary binding named `import` cannot alter structural import syntax or importer selection.

Defer custom importer execution and package acquisition. The initial bootstrap environment may resolve compiler-owned modules from a fixed, versioned table.

Exit criterion: a purpose-built manifest-enumerated multi-file test proves manifest loading, complete source-unit assembly, implicit single-file package identity, symmetric namespace declaration/import resolution, explicit object-to-ordinary binding, lexical object-form lookup, collision and idempotent-reimport rules, prelude enablement and disablement, `global` versus namespace-local assignment, visibility, shadowing, root/parent lookup, and structural import independence from ordinary bindings.

### Milestone 4 — Types, calls, and control-flow semantics

Deliver:

- direct native lowering types for `bool`, fixed-width signed and unsigned integers through 128 bits, `float`, the explicit widths `float32` and `float64`, `string`, and `none`, where the Rust representation preserves the complete Strata contract;
- core `int` as an exact signed integer with adaptive `i64`, `i128`, and arbitrary-precision tiers, including normalization to the smallest exact tier;
- the initial integer support component and lowering hooks for checked tier promotion, exact wide operations, normalization, and capability rejection where arbitrary-precision promotion is unavailable;
- explicit `/core types` resolution for fixed-width descriptor objects: programs import dot-object descriptors and bind ordinary type names, while the exact default prelude remains unchanged;
- typed literals and inferred local bindings, with destination-range checking applied to every compile-time constant expression and signed fixed-width minima accepted without first rejecting their positive magnitude;
- typed parameters, optional parameters with defaults, and return contracts;
- initialized and uninitialized typed bindings, with definite-assignment analysis rejecting reads before assignment across control flow;
- assignment compatibility without implicit cross-type coercion;
- explicit throwing `coerce` plus `checked-coerce`, `wrapping-coerce`, and `saturating-coerce` for integer destinations only, covering `int` and every fixed width, including fixed-width-to-`int` widening only when requested explicitly; a floating-point or `string` destination is rejected with an explicit unsupported-destination diagnostic rather than partially implemented, so `.integer-conversion-overflow` remains the only conversion failure version one can raise;
- unary, arithmetic, shift, bitwise, comparison, Boolean, equality, identity/type-membership, and type-appropriate operator checking;
- exact `int` arithmetic, infinite two's-complement bitwise behavior, exact/arithmetic shifts, and Euclidean division/remainder without inheriting Rust overflow, shift, or signed division behavior;
- fixed-width checked ordinary arithmetic and explicit checked, wrapping, saturating, and overflowing operation families without host debug/release dependence; fixed-width shift counts receive an explicit source-language operation contract rather than inheriting host behavior;
- an interim uncaught-runtime-failure contract for division by zero, fixed-width overflow, integer-conversion overflow, and invalid shifts: preserve the normative error identity and source location, render it deterministically, and exit nonzero while source `throw`/`try`/`catch` remains deferred;
- positional and named argument binding, arity and default checks, explicit zero-argument `;`, and duplicate-argument errors;
- semantic distinction among calls, member access, and dot-objects passed explicitly as ordinary argument values;
- strict left-to-right operand and argument evaluation, receiver-before-selection, exactly-once assignment receiver/index evaluation, `and`/`or` short-circuiting, and call-site defaults after supplied arguments in parameter order;
- truth and core text-display protocols implemented for the supported core types, with `print` consuming canonical scalar display left to right and appending a newline; arbitrary `bytes`, unsupported values, and locale/styled formatting are not guessed; float display must explicitly normalize non-finite spellings to `inf`, `-inf`, and `nan` rather than inherit Rust's `NaN`, while preserving negative zero and shortest round-trippable finite output;
- branch and loop checking, postfix-update placement and integer-family semantics, loop-control placement, unreachable-code facts, and definite return analysis;
- default `string.length` measured in grapheme clusters, either backed by the required segmentation capability or rejected with a capability diagnostic suggesting explicit implemented `bytes`, `scalars`, or `graphemes` views; another unit must never be substituted silently;
- an explicit minimal collection subset for iteration, with mutation accepted only where ordinary assignment cannot expose aliasing that violates deferred universal COW semantics;
- version-one identity restricted to canonical compiler-owned descriptor objects, including type descriptors exposed by `.type`; ordinary scalars, strings, and collections are identity-less, so even `x is x` is false for them, while `===` is rejected with the explicit `left == right and left.type is right.type` spelling;
- canonical type descriptors as source-observable values with stable identity, while version-one type expressions and coercion destinations must resolve to finite compiler-known descriptor alternatives and may be erased only when source behavior is preserved;
- explicit unsupported-feature diagnostics for source-declared type parameters rather than accidental parser or type-checker failures;
- finite dynamic bindings only where all alternatives lower soundly without a universal box; because version one knows every alternative in such a binding, protocol availability and typed-boundary compatibility are checked statically, so unsupported text display or argument compatibility is rejected at compile time rather than entering the interim runtime-failure contract.

Core text display and receiver-based text behavior must be exercised through the canonical object model, including integer output:

```text
message = ': '.concat; project-name, build-target, build-status
print; message
print; completed-count
```

Exit criterion: semantic and lowering conformance for a program that exercises the same contracts as `fizz-buzz` and `build-report` proves the specified integer, canonical scalar text-display, type-descriptor, call, evaluation-order, and control-flow behavior; generated crates compile and run through the existing pipeline, while plausible type, call, definite-assignment, arithmetic-failure, shift/bitwise, display, descriptor-resolution, and capability mistakes fail at Strata source spans. If the text-display protocol is not yet implemented when milestone 4 begins, the initial executable fixture may print literal strings only, but integer-rendering conformance is required before the milestone exits.

### Milestone 5 — Rust IR, readable emission, and Cargo builds

Deliver:

- explicit Rust-oriented lowering IR;
- deterministic module and item ordering;
- injective source-name-to-Rust-name encoding;
- direct fixed-width scalar and function lowering where Rust preserves the source contract;
- integration of the adaptive core-`int` support component into the explicit Rust IR, preserving checked tier promotion, exact wide operations, result normalization, normative runtime failures, and target capability diagnostics;
- structured expression/block emission with a pinned formatter policy;
- generated `Cargo.toml`, source tree, compiler metadata, and entrypoint;
- deterministic inclusion of the integer support crate by copying compiler-bundled, content-addressed source into the generated build directory and referring to it by a generated-project-relative Cargo path, without registry, network, or install-location paths; the bundled source content identity enters the build key, and the same vendoring mechanism applies to any authored third-party dependency admitted later;
- content-addressed build directory keyed by compiler version, source inputs, target, and relevant options;
- `cargo check`, build, and run process wrappers with captured structured output;
- `strata rust` output or path display suitable for inspection, clearly distinguishing authored generated modules from vendored support source.

Generated artifacts should be organized under a project-local ignored directory or a user cache, never mixed with authored source. A `--keep-generated` or stable development path may expose them intentionally.

Exit criterion: identical inputs produce byte-identical generated files; all accepted compile cases pass Cargo; the generated Rust for representative fixtures is readable and has reviewed goldens. Goldens pin canonical float display at both `float32` and `float64` width for `nan`, `inf`, `-inf`, negative zero, and shortest round-trippable finite values; they pin one multi-argument `print` call proving that arguments render adjacently with no inserted separator and exactly one trailing newline, alongside adjacent `print` calls proving record separation; and generated-project goldens include both authored lowered modules and the vendored support copy.

### Milestone 6 — Source diagnostics across Rust

Deliver:

- basic source associations from semantic nodes to generated Rust spans;
- JSON-formatted Cargo/rustc diagnostic ingestion;
- projection of backend errors to the most relevant Strata span;
- raw Rust diagnostic retained as a note or opt-in detail;
- stable diagnostic codes and CLI rendering with color policy;
- distinction among source errors, uncaught source-language runtime failures, compiler defects, Rust toolchain failures, and ordinary user-program exits; normative runtime failures render Strata namespace/function frames and source spans, retain generated Rust frames only as expandable detail, and never surface as raw Rust panics or backtraces;
- internal-error reports that preserve generated artifacts and reproduction metadata.

The frontend should prevent ordinary type/name errors from reaching rustc. Backend translation exists for missed constraints, target failures, generated-code defects, and handwritten/toolchain boundaries—not as a substitute for semantic analysis.

Exit criterion: at least one deliberately induced backend error is mapped to its Strata source location, and raw rustc information remains available.

### Milestone 7 — First-version hardening and release gate

Deliver:

- complete CLI help and documented exit codes, including a stable distinct code for uncaught source-language runtime failures;
- stable build-directory and cache behavior;
- interruption and subprocess cleanup;
- Windows/macOS/Linux path handling where CI is available;
- deterministic tests and generated artifacts;
- parser/lexer fuzz targets seeded from conformance cases;
- performance baselines for cold check, warm check, build, and run;
- compiler self-diagnostics for unsupported draft features;
- a release manifest listing the exact implemented language subset;
- runnable `examples/` that all compile in CI;
- no test or release command that treats `demos/` as supported source.

Exit criterion: the clean-checkout release scenario below passes on supported platforms and the implemented-subset document agrees with executable conformance tests.

## 8. Clean-checkout release scenario

The release pipeline must prove, from a clean checkout:

1. build the Rust compiler workspace;
2. report `strata --version`;
3. run unit and conformance tests;
4. verify rejected fixtures and diagnostic goldens;
5. compile every accepted compile fixture with Cargo;
6. execute every run fixture and compare exact stdout, stderr, and exit code;
7. build every file under `examples/`;
8. run `strata rust` twice for selected cases and compare generated artifacts byte-for-byte;
9. verify no test enumerated, parsed, or built anything under `demos/`;
10. package the `strata` executable and install that artifact into a second clean environment;
11. compile and run `examples/build-report.strata` using only the installed artifact and Rust toolchain prerequisites.

## 9. Initial feature boundary

### Required in first version

- UTF-8, indentation, all three comment forms, exact spans, and legal empty blocks;
- exact ASCII-only version-one identifier character/joiner policy, spacing-sensitive operators, prefix negation, postfix `++`/`--`, and layered angle-generic rejection;
- normative §34 precedence, associativity, non-associative comparisons, call-free arguments, explicit semicolon calls, and grouping rules;
- a minimal package manifest, manifest-enumerated source units, implicit single-file package identity, namespace declarations, and the fixed version-one bootstrap module table;
- ordinary/object-form lexical lookup distinction, collision/idempotent-import rules, and structural imports unaffected by ordinary bindings;
- locals, namespace bindings, visibility, explicit `global` bindings, definite assignment, and the exact default prelude;
- core literals and static scalar types, including adaptive exact `int`, fixed-width integer contracts, `float` with its explicit `float32`/`float64` widths, integer-destination coercion families, normative arithmetic/conversion failures, and target capability diagnostics;
- functions, required/optional parameters, positional/named arguments, calls, and return values;
- basic expressions, descriptor-object identity and `.type`, type-membership predicates, assignment, shifts, bitwise operators, and specified evaluation order; ordinary values remain identity-less and `===` is rejected;
- `if`/`else`, `while`, collection and three-clause `for`, `break`, `continue`, and `return`;
- grapheme-defined default string length with capability diagnostics, explicit implemented string views, and a minimal collection/output surface sufficient for real CLI programs without violating deferred universal COW semantics;
- deterministic Rust lowering, a compiler-bundled integer support component usable offline, Cargo build/run, source maps, and diagnostics.

### Explicitly deferred

- classes, inheritance, interfaces, traits, and constructors;
- universal COW semantics and mutable collection aliasing;
- `ref`, borrow families, `move`, linear values, and deterministic user-defined destruction;
- `throw`/`try`/`catch`/`finally`;
- floating-point and `string` coercion destinations, including numeric-to-float rounding and text parsing;
- variadics, generators, closures if they delay the core pipeline;
- custom declaration modifiers and package-defined type constructors;
- custom importers, registries, lockfiles, Rust/system/runtime dependencies;
- inline/full-file Rust and C ABI export;
- reflection, debugger integration, tracing, and profiling;
- async/concurrency;
- build-time selection, labels, and `goto` unless needed before systems profiles;
- `no_std`, embedded, firmware, and kernel compilation;
- parsing or compiling `demos/fork.strata` as an acceptance goal.

Deferral means “diagnose as unsupported,” not “leave behavior accidental.”

## 10. Work sequencing inside each milestone

For every language feature:

1. write at least one accepted and one plausible rejected source fixture;
2. add the smallest lexer/parser support needed;
3. add resolution and semantic rules with source diagnostics;
4. add lowering and a reviewed Rust golden where the output contract changes;
5. compile the generated crate;
6. run the source program when the feature has runtime behavior;
7. add the case to the permanent conformance suite;
8. update the implemented-feature manifest only after the end-to-end case passes.

A feature is not complete when it merely parses or emits plausible Rust.

## 11. Early design decisions that require prototype evidence

Resolve these through small conformance branches before their dependent milestones are frozen:

- representation and implementation strategy for exact adaptive core `int`, including its arbitrary-precision support dependency and target-capability boundary;
- whether the selected collection subset needs facilities beyond the already-required integer support component;
- representation of finite dynamic alternatives in generated Rust;
- generated module boundaries for multiple namespaces in one package;
- source-map encoding between Strata byte spans and generated Rust spans;
- manifest location and deterministic discovery for multi-unit projects.

Each decision should leave behind executable accepted/rejected cases. Do not use `demos/` to settle these questions because their surrounding unsupported constructs would confound the result.

## 12. Immediate implementation backlog

1. Create the Rust workspace, CLI crate, compiler crate, and diagnostic model.
2. Define the conformance case manifest and test runner.
3. Add a tiny authored `tests/conformance/run/hello/case.strata` fixture.
4. Implement source files, spans, tokens, and indentation lexing for that fixture.
5. Implement the minimal lossless tree and semantic AST.
6. Parse namespace, import/binding, `function main`, string value, and invocation.
7. Resolve a compiler-owned `.print` object and its explicit ordinary binding.
8. Lower the program through a Rust IR into a generated Cargo binary.
9. Make `strata rust`, `build`, `run`, and `check` use that shared pipeline.
10. Add malformed indentation, unterminated string, unresolved object, and wrong-call rejected cases.
11. Expand lexer conformance around identifier/operator attachment before adding arithmetic.
12. Proceed milestone by milestone, adding real programs only when every construct they contain is supported.

## 13. Definition of done

The first-version compiler is done only when:

- its supported subset is explicit and executable;
- accepted programs are checked, lowered, compiled, and run through one pipeline;
- rejected programs fail at the correct Strata spans with stable diagnostics;
- generated Rust and Cargo files are deterministic and readable;
- a nontrivial purpose-built CLI program builds from a clean installed compiler;
- tests cover parsing, semantics, lowering, Cargo integration, runtime behavior, and backend diagnostic projection;
- `examples/` contains only programs guaranteed to build;
- `demos/` remains clearly excluded from all support and conformance claims;
- unsupported draft features fail clearly rather than being silently miscompiled.
