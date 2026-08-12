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
case.strata
case.toml             # phase, expected status, entrypoint, arguments
stdin.txt             # optional
stdout.txt            # optional exact output
stderr.txt            # optional exact user-visible diagnostic
exit-code.txt         # optional, defaults to zero for accepted runs
parse.json            # optional normalized syntax shape
resolve.json          # optional symbol-resolution facts
lower.rs               # optional canonical generated Rust
```

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
2. **build-report:** kebab-case bindings, typed integers and strings, receiver-based string concatenation, named arguments, and output.
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

- UTF-8 source validation;
- tokens with exact spans and retained trivia;
- `NEWLINE`, `INDENT`, and `DEDENT` generation;
- blank lines and comment-only lines that do not perturb indentation;
- `#`, `//`, and `/* ... */` comments;
- quoted, tail, and indented block strings plus numeric literals;
- identifiers with operator-bearing joiners, while a terminal joiner followed by a digits-only unit is rejected;
- structural punctuation and spacing-sensitive operator attachment, including `++`/`--` as declared postfix tokens and `<`/`>` as punctuation rather than angle-generic delimiters;
- lexical diagnostics for mixed tab/space indentation styles, invalid characters, unterminated strings/comments, inconsistent dedents, illegal attached operators, attached joiner-plus-digits forms such as `count-1` with a spaced-expression fix, and angle-bracket generic spellings such as `list<string>` with a canonical `list of string` fix;

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
```

Indentation cases must cover consistently space-indented and consistently tab-indented files, a mixture within one indentation prefix, and a style change between different code lines in one file.

Exit criterion: lexer corpus covers every token class and malformed boundary; all diagnostics point to the originating bytes and remain correct for multibyte UTF-8.

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
- explicit unsupported-feature nodes or diagnostics for reserved/deferred constructs, including source-declared type parameters and angle-bracket generic spelling.

Highest-risk ambiguities must receive dedicated tests before broad grammar work:

- `print.concat` versus invalid `print .concat` adjacency;
- `.thing` versus the explicit zero-argument call `.thing;`;
- tail-string markers versus comparison and shift operators;
- operator-bearing identifiers, prefix negation, postfix `++`/`--`, and spaced operators;
- namespace whitespace tiers versus expressions;
- call semicolon precedence, named arguments, and grouping of nested calls;
- grouping calls inside the clauses of a three-clause `for`;
- `is a` as identity against an ordinary binding versus type membership when `a` is followed by a complete type expression.

Invalid adjacency and missing grouping must produce source-oriented diagnostics with valid explicit-semicolon and parenthesized-call fixes; the parser must never repair them silently.

The parser must implement the normative §34 precedence and associativity table, including non-associative comparisons, and mechanically expand the call-free-expression variant used by argument grammar rather than maintaining a second expression grammar.

Exit criterion: every first-version construct has accepted and rejected parse cases; no semantic decision is required merely to recover the intended tree shape.

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
- fixed bootstrap importer limited to compiler-owned core facilities and processed as a structural compilation phase rather than an ordinary runtime call;
- the exact default prelude bindings `print`, `int`, `float`, `bool`, `string`, `bytes`, and `none`;
- import resolution that does not create an ordinary binding automatically, and proof that an ordinary binding named `import` cannot alter structural import syntax or importer selection.

Defer custom importer execution and package acquisition. The initial bootstrap environment may resolve compiler-owned modules from a fixed, versioned table.

Exit criterion: a purpose-built manifest-enumerated multi-file test proves manifest loading, complete source-unit assembly, implicit single-file package identity, symmetric namespace declaration/import resolution, explicit object-to-ordinary binding, lexical object-form lookup, collision and idempotent-reimport rules, prelude enablement and disablement, `global` versus namespace-local assignment, visibility, shadowing, root/parent lookup, and structural import independence from ordinary bindings.

### Milestone 4 — Types, calls, and control-flow semantics

Deliver:

- direct native lowering types for `bool`, fixed-width signed and unsigned integers through 128 bits, `float`, `string`, and `none`, where the Rust representation preserves the complete Strata contract;
- core `int` as an exact signed integer with adaptive `i64`, `i128`, and arbitrary-precision tiers, including normalization to the smallest exact tier;
- the initial integer support component and lowering hooks for checked tier promotion, exact wide operations, normalization, and capability rejection where arbitrary-precision promotion is unavailable;
- typed literals and inferred local bindings, including signed fixed-width minimum literals handled without first rejecting their positive magnitude;
- typed parameters, optional parameters with defaults, and return contracts;
- initialized and uninitialized typed bindings, with definite-assignment analysis rejecting reads before assignment across control flow;
- assignment compatibility without implicit cross-type coercion;
- unary, arithmetic, shift, bitwise, comparison, Boolean, equality, identity/type-membership, and type-appropriate operator checking;
- exact `int` arithmetic, infinite two's-complement bitwise behavior, exact/arithmetic shifts, and Euclidean division/remainder without inheriting Rust overflow, shift, or signed division behavior;
- fixed-width checked ordinary arithmetic and explicit checked, wrapping, saturating, and overflowing operation families without host debug/release dependence;
- an interim uncaught-runtime-failure contract for division by zero, fixed-width overflow, and invalid shifts: preserve the normative error identity and source location, render it deterministically, and exit nonzero while source `throw`/`try`/`catch` remains deferred;
- positional and named argument binding, arity and default checks, explicit zero-argument `;`, and duplicate-argument errors;
- semantic distinction among calls, member access, and dot-objects passed explicitly as ordinary argument values;
- strict left-to-right operand and argument evaluation, receiver-before-selection, exactly-once assignment receiver/index evaluation, `and`/`or` short-circuiting, and call-site defaults after supplied arguments in parameter order;
- truth protocol initially implemented for core types only;
- branch and loop checking, postfix-update placement and integer-family semantics, loop-control placement, unreachable-code facts, and definite return analysis;
- an explicit version-one string iteration/length choice among graphemes, scalars, or bytes, with required capability diagnostics rather than silent substitution;
- an explicit minimal collection subset for iteration, with mutation accepted only where ordinary assignment cannot expose aliasing that violates deferred universal COW semantics;
- explicit unsupported-feature diagnostics for source-declared type parameters rather than accidental parser or type-checker failures;
- finite dynamic bindings only where all alternatives lower soundly without a universal box.

Receiver-based text behavior must be tested using the canonical object model, for example:

```text
message = ': '.concat; project-name, build-target, build-status
print; message
```

Exit criterion: semantic and lowering conformance for `fizz-buzz` and `build-report` proves the specified integer, type, call, evaluation-order, and control-flow contracts; generated crates compile and run through the existing pipeline, while plausible type, call, definite-assignment, arithmetic-failure, shift/bitwise, and capability mistakes fail at Strata source spans.

### Milestone 5 — Rust IR, readable emission, and Cargo builds

Deliver:

- explicit Rust-oriented lowering IR;
- deterministic module and item ordering;
- injective source-name-to-Rust-name encoding;
- direct fixed-width scalar and function lowering where Rust preserves the source contract;
- integration of the adaptive core-`int` support component into the explicit Rust IR, preserving checked tier promotion, exact wide operations, result normalization, normative runtime failures, and target capability diagnostics;
- structured expression/block emission with a pinned formatter policy;
- generated `Cargo.toml`, source tree, compiler metadata, and entrypoint;
- content-addressed build directory keyed by compiler version, source inputs, target, and relevant options;
- `cargo check`, build, and run process wrappers with captured structured output;
- `strata rust` output or path display suitable for inspection.

Generated artifacts should be organized under a project-local ignored directory or a user cache, never mixed with authored source. A `--keep-generated` or stable development path may expose them intentionally.

Exit criterion: identical inputs produce byte-identical generated files; all accepted compile cases pass Cargo; the generated Rust for representative fixtures is readable and has reviewed goldens.

### Milestone 6 — Source diagnostics across Rust

Deliver:

- basic source associations from semantic nodes to generated Rust spans;
- JSON-formatted Cargo/rustc diagnostic ingestion;
- projection of backend errors to the most relevant Strata span;
- raw Rust diagnostic retained as a note or opt-in detail;
- stable diagnostic codes and CLI rendering with color policy;
- distinction among source errors, compiler defects, Rust toolchain failures, and user-program exit failures;
- internal-error reports that preserve generated artifacts and reproduction metadata.

The frontend should prevent ordinary type/name errors from reaching rustc. Backend translation exists for missed constraints, target failures, generated-code defects, and handwritten/toolchain boundaries—not as a substitute for semantic analysis.

Exit criterion: at least one deliberately induced backend error is mapped to its Strata source location, and raw rustc information remains available.

### Milestone 7 — First-version hardening and release gate

Deliver:

- complete CLI help and documented exit codes;
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
- exact identifier character/joiner policy, spacing-sensitive operators, prefix negation, postfix `++`/`--`, and angle-generic rejection;
- normative §34 precedence, associativity, non-associative comparisons, call-free arguments, explicit semicolon calls, and grouping rules;
- a minimal package manifest, manifest-enumerated source units, implicit single-file package identity, namespace declarations, and fixed bootstrap imports;
- ordinary/object-form lexical lookup distinction, collision/idempotent-import rules, and structural imports unaffected by ordinary bindings;
- locals, namespace bindings, visibility, explicit `global` bindings, definite assignment, and the exact default prelude;
- core literals and static scalar types, including adaptive exact `int`, fixed-width integer contracts, normative arithmetic failures, and target capability diagnostics;
- functions, required/optional parameters, positional/named arguments, calls, and return values;
- basic expressions, identity/type-membership predicates, assignment, shifts, bitwise operators, and specified evaluation order;
- `if`/`else`, `while`, collection and three-clause `for`, `break`, `continue`, and `return`;
- a selected string iteration/length contract and minimal collection/output surface sufficient for real CLI programs without violating deferred universal COW semantics;
- deterministic Rust lowering, the first integer support component, Cargo build/run, source maps, and diagnostics.

### Explicitly deferred

- classes, inheritance, interfaces, traits, and constructors;
- universal COW semantics and mutable collection aliasing;
- `ref`, borrow families, `move`, linear values, and deterministic user-defined destruction;
- `throw`/`try`/`catch`/`finally`;
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
- whether collection iteration requires an initial support crate;
- representation of finite dynamic alternatives in generated Rust;
- generated module boundaries for multiple namespaces in one package;
- source-map encoding between Strata byte spans and generated Rust spans;
- stable CLI project discovery versus explicit single-file entrypoints.

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
