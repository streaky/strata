# Terrane first-version compiler plan

## 1. Purpose

Build the first usable Terrane compiler as a source-to-Rust toolchain. The compiler must accept a deliberately bounded, coherent subset of Terrane, produce readable and deterministic Rust, invoke Cargo/rustc, and return diagnostics in Terrane source terms.

This plan is for an executable compiler, not another language-design prototype. Every milestone must finish with source programs that are compiled and run through the real pipeline.

`demos/` is explicitly outside the compiler conformance contract. Files there are exploratory pressure tests and may intentionally combine unfinished syntax, package adapters, kernel facilities, unsafe contracts, or speculative semantics. They must not be used as smoke tests, parser fixtures, acceptance criteria, or examples of what the current compiler is expected to build. Compiler development will create purpose-built test programs under `tests/` and runnable examples under `examples/` instead.

## 2. First-version outcome

The first version is complete when a user can:

```text
terrane check path/to/program.trn
terrane run path/to/program.trn -- program-arguments
terrane build path/to/program.trn
terrane rust path/to/program.trn
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

That list is the **first runnable outcome**, reached at milestone 6, not the boundary of version one. Version one as now specified also requires structured errors, the callable-family and descriptor models, collections and iteration, classes and interfaces, ownership and resources, capabilities and effects, async with structured concurrency, and the standard facilities in milestones 20 through 26. Milestones 7 onward deliver those.

Version one still does not need universal dynamic values, source-declared generics, general pattern matching, multiple class inheritance, generators, labels and `goto`, hot-code replacement, `no_std`, embedded targets, or kernel targets. Syntax for deferred features may be recognized only when doing so enables a precise “not supported in this compiler version” diagnostic; it must never be accepted and lowered incorrectly.

## 3. Delivery principles

1. **Tests define implemented behavior.** The design draft informs the implementation, but an executable conformance case is required before a feature is considered supported.
2. **No dependency on `demos/`.** CI must neither compile nor parse files from `demos/` unless a future, explicitly named demo-specific job is introduced.
3. **Vertical slices before breadth.** Establish `source -> Rust -> Cargo -> executable` early, then expand the language through end-to-end slices.
4. **One semantic path.** `check`, `run`, `build`, and `rust` share the same frontend and semantic pipeline. Commands must not grow separate parsers or validators.
5. **No silent repair.** Invalid Terrane is rejected at its source span. The compiler must not reinterpret failed syntax as a nearby construct merely to continue.
6. **Deterministic output.** The same source, compiler version, target, and declared inputs produce byte-identical generated source and manifests.
7. **Readable lowering.** Generated Rust is a public debugging surface, not opaque compiler debris.
8. **Narrow runtime.** Statically known fixed-width scalars and functions lower directly to Rust types and calls where Rust preserves the complete Terrane contract; core `int` uses the narrowest exact representation required by its adaptive semantics. The first compiler must not introduce a universal boxed `Value` as a shortcut.
9. **Standard facilities are written in Terrane.** The Rust core stays deliberately minimal. Document formats, networking protocols, compression framing, date and time arithmetic, paths, CLI parsing, and logging are Terrane packages over that core, not Rust support crates. A Rust support crate is permanently opaque to the compiler, so implementing a facility in Rust forecloses inlining, specialisation, and whole-program analysis for it forever; it also loses the readable Terrane frames the diagnostics contract requires. The boundary runs per layer rather than per facility: Rust owns the layer that is a syscall or ABI boundary, carries a guarantee the optimiser would destroy, is a large audited security-critical implementation, or is generated data — and a layer claiming to be Rust states which of the four applies. Everything above it is Terrane. Core libraries reach Rust through the ordinary dependency mechanism, so they carry no privileged path and double as worked examples. Two consequences are load-bearing rather than incidental: package-level artifact caching, because a source-form standard library would otherwise be recompiled by every build, and capability profiles expressed as which packages are present rather than which crates were compiled in.

## 4. Proposed repository layout

```text
compiler/
  Cargo.toml
  crates/
    terrane-cli/
      Cargo.toml
      src/
        main.rs
    terrane-compiler/
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
  hello.trn
  word-count.trn
  build-report.trn
```

The first compiler and its CLI should be implemented in Rust. This gives the project one distributable executable, exhaustive phase models, direct integration with Cargo diagnostics and any support crates, and no later frontend rewrite boundary.

Use mature Rust parsing tooling rather than treating Rust as a requirement to hand-write every frontend component. Chumsky is a strong initial candidate: it supports separate character and token parsers, token-associated spans, recursive combinators, Pratt expression parsing, rich errors, and recovery. Prototype Terrane's hardest lexical and grammatical boundaries with it before freezing the parser architecture. Keep Terrane tokens, syntax nodes, spans, and diagnostics compiler-owned so replacing or selectively bypassing the parsing library would not change the language model.

Do not create a general runtime crate before an implemented feature requires one. Core `int` is the first such feature: introduce a small support crate with its first semantic/lowering slice for adaptive exact integers and their normative failures. Keep other statically known values on direct Rust lowering and add support only for behavior that generated code cannot express cleanly.

## 5. Test corpus design

### 5.1 Fixture contract

Each conformance case is a directory or manifest entry containing only the artifacts relevant to its assertion:

```terrane
case.trn     # single-source input
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
- **Run tests:** purpose-built Terrane programs execute and produce exact output and exit status.
- **CLI integration tests:** command arguments, exit codes, output locations, and diagnostic behavior.
- **Differential invariants:** `check` and `build` accept or reject the same source; `rust` uses the same semantic model; formatting or comments do not alter runtime behavior.

### 5.3 Initial real programs

Create these incrementally rather than borrowing from `demos/`:

1. **hello:** import/bind output, define `main`, print exact text.
2. **build-report:** kebab-case bindings, typed integers and strings, receiver-based string joining, named arguments, and output. Because fixed-width names are `/core/types` descriptor objects rather than prelude bindings, the authored file either uses `int` or writes the explicit descriptor import and ordinary binding for each width it names.
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

- Preserve exact Terrane spelling as symbol identity.
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

- Rust workspace, `terrane` CLI executable, and compiler library;
- compiler version reporting and structured exit codes;
- isolated temporary/build directories;
- conformance harness supporting accept, reject, Rust golden, compile, and run cases;
- automatic Rust toolchain prerequisite check;
- initial `hello` accepted fixture and several rejected placeholders that fail with an explicit unsupported-stage diagnostic;
- CI commands that run compiler tests without traversing `demos/`.

End-to-end proof:

```text
terrane rust tests/conformance/run/hello/case.trn
terrane build tests/conformance/run/hello/case.trn
<generated executable>
```

At this milestone the frontend may support only the exact constructs needed by `hello`, but the source must travel through the real token, syntax, semantic, lowering, Cargo, and execution boundaries. Do not implement `hello` by source-text substitution.

Exit criterion: one purpose-built Terrane file produces a real executable and exact expected output; malformed input fails through the diagnostic framework.

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

```terrane
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
- literals, names, object-form lookup, member access, calls, unary/binary expressions, assignment, grouping, and postfix `++`/`--` as an update *statement* rather than an expression, so a value-position use fails to parse;
- `if`/`else`, `while`, collection and three-clause `for`, `return`, `break`, and `continue` syntax;
- parser recovery at newline and dedent boundaries;
- normalized parse-tree serializer for goldens;
- explicit unsupported-feature nodes or diagnostics for reserved/deferred constructs, including source-declared type parameters, `===` with an explicit equality/type-identity fix, and angle-bracket generic intent in type position with a canonical `list of string` fix, recognized from both the bare trailing `>` and the `>=` spelling.

Highest-risk ambiguities must receive dedicated tests before broad grammar work:

- `print.concat` versus invalid `print .concat` adjacency;
- `.thing` versus the explicit zero-argument call `.thing;`;
- tail-string markers versus comparison and shift operators;
- operator-bearing identifiers, prefix negation, postfix `++`/`--`, and spaced operators;
- namespace `/` separators versus division, and namespace segments versus identifiers;
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
S1012 chained non-associative test        S1027 malformed importer selection
S1013 invalid member adjacency            S1028 malformed collection target
S1014 missing member name                 S1029 invalid declaration prefix
S1015 unclosed index expression           S1030 assignment in condition
S1016 unparenthesized nested call         S1090 reserved unsupported syntax
S1091 unsupported `===`                   S1092 unsupported angle generic
```

`S1010` is intentionally unassigned. Diagnostics whose correction is not
fully expressed by the primary message carry structured help; CLI rendering
prints that help separately from the stable code and message.

Language work must introduce its accepted and rejected cases in the same
vertical work unit as the behavior.

### Milestone 3 — Namespaces, scopes, and bootstrap environment

Deliver:

- a minimal package manifest contract and loader that enumerate the complete source-unit set and select whether the default prelude is enabled;
- a single-file CLI input modeled as an implicit one-unit package with a stable package identity and the default prelude, without filename-to-namespace inference or on-demand namespace search (**superseded by milestone 4.7**, which introduces checked namespace-to-directory correspondence; on-demand search remains excluded);
- namespace tree assembled from the complete manifest-enumerated set of package source units before resolution;
- deterministic multi-file discovery and source-unit assembly order;
- exact root `/` and parent `..` anchoring;
- separate ordinary and object-form symbol tables, with lexical object-form lookup;
- namespace-local, function-local, parameter, and program-global scopes needed by the first version;
- explicit `global` handling for program-global creation/replacement and rejection of plain top-level assignment where a global operation is required;
- duplicate, shadowing, visibility/inaccessibility, unresolved-name, and same-scope object-form collision diagnostics;
- idempotent reimport of the same object-form export, with aliases required for distinct colliding exports;
- fixed bootstrap importer whose milestone-3 module table registers versioned `/core/output`, `/core/types`, `/core/errors`, and `/collections` namespaces as structural compiler-owned modules rather than runtime calls; milestone 3 populates the first three, including all fixed-width numeric descriptor objects under `/core/types`, while milestone 4 populates `/collections` with its selected collection subset;
- the exact default prelude bindings `print`, `int`, `float`, `bool`, `string`, `bytes`, and `none`, requiring no import at any use site;
- import resolution that does not create an ordinary binding automatically, and proof that an ordinary binding named `import` cannot alter structural import syntax or importer selection.

Defer custom importer execution and package acquisition. The initial bootstrap environment may resolve compiler-owned modules from a fixed, versioned table.

Exit criterion: a purpose-built manifest-enumerated multi-file test proves manifest loading, complete source-unit assembly, implicit single-file package identity, symmetric namespace declaration/import resolution, explicit object-to-ordinary binding, lexical object-form lookup, collision and idempotent-reimport rules, prelude enablement and disablement, `global` versus namespace-local assignment, visibility, shadowing, root/parent lookup, and structural import independence from ordinary bindings.

Implemented evidence: package input now uses the authored `package.toml` contract and
deterministically loads its complete enumerated source set before analysis. The shared
semantic pass assembles symmetric namespace declarations, resolves exact root and parent
imports, keeps ordinary and object-form namespaces separate, and records lexical scopes
for parameters, local bindings, assignments, and block-local object imports. Its fixed
bootstrap table and exact default prelude are versioned compiler-owned data. Focused
accepted and rejected cases cover explicit object-to-ordinary binding, visibility,
duplicate and collision rules, idempotent reimports, `global` assignment, legal
namespace-local shadowing of program globals, unresolved references, and ordinary
bindings named `import`; a manifest-driven multi-source contract test exercises package
assembly and cross-unit resolution. Semantic phases report the first failure in
deterministic package and source order because subsequent resolution failures can depend
on declarations or imports that the first failure prevented from assembling; manifest
loading instead accumulates its independently discoverable diagnostics.

### Milestone 4 — Types, calls, and control-flow semantics

Deliver:

- direct native lowering types for `bool`, fixed-width signed and unsigned integers through 128 bits, `float`, the explicit widths `float32` and `float64`, `string`, and `none`, where the Rust representation preserves the complete Terrane contract;
- core `int` as an exact signed integer with adaptive `i64`, `i128`, and arbitrary-precision tiers, including normalization to the smallest exact tier;
- the initial integer support component and lowering hooks for checked tier promotion, exact wide operations, normalization, and capability rejection where arbitrary-precision promotion is unavailable;
- explicit `/core/types` resolution for fixed-width descriptor objects: programs import dot-object descriptors and bind ordinary type names, while the exact default prelude remains unchanged;
- typed literals and inferred local bindings, with destination-range checking applied to every compile-time constant expression and signed fixed-width minima accepted without first rejecting their positive magnitude;
- typed parameters, optional parameters with defaults, and return contracts;
- initialized and uninitialized typed bindings, with definite-assignment analysis rejecting reads before assignment across control flow;
- assignment compatibility without implicit cross-type coercion;
- explicit throwing integer coercion plus checked, wrapping, and saturating policies for `int` and fixed-width integer sources and fixed-width integer destinations, now exposed through the canonical `.coerce`, `.coerce.checked`, `.coerce.wrap`, and `.coerce.saturate` callable family; floating-point and `string` destinations remain rejected with an explicit unsupported-destination diagnostic rather than partially implemented, so `.integer-conversion-overflow` remains the only conversion failure version one can raise.
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

```terrane
message = ': '.join; project-name, build-target, build-status
print; message
print; completed-count
```

Exit criterion: semantic and lowering conformance for a program that exercises the same contracts as `fizz-buzz` and `build-report` proves the specified integer, canonical scalar text-display, type-descriptor, call, evaluation-order, and control-flow behavior; generated crates compile and run through the existing pipeline, while plausible type, call, definite-assignment, arithmetic-failure, shift/bitwise, display, descriptor-resolution, and capability mistakes fail at Terrane source spans. If the text-display protocol is not yet implemented when milestone 4 begins, the initial executable fixture may print literal strings only, but integer-rendering conformance is required before the milestone exits.

Implemented evidence: the compiler now resolves compiler-owned and imported scalar
descriptors, infers and checks typed bindings, parameters, defaults, returns, calls,
operators, assignments, branches, loops, updates, and finite descriptor alternatives.
Native fixed-width values lower directly to Rust with checked default arithmetic and
shift operations; adaptive `int` operations use the dedicated exact-integer support
crate, and deterministic source-oriented failures cover unsupported or invalid arithmetic
and coercion paths. Canonical scalar display, grapheme-counted string length, descriptor
and value-type identity, and the supported collection iteration slice are checked before
lowering. Manifest-driven accepted cases and focused rejected cases exercise these semantic
boundaries, while semantic unit tests cover the broader diagnostic set. The conformance
corpus includes adaptive logical comparisons, destination-aware returns, assignments, and
lengths, fixed-width overflow, invalid member receivers, compound membership and identity,
and identity operand evaluation order. The `fizz-buzz`, `build-report`,
`grouped-precedence`, and focused regression run cases compile generated crates with
warnings denied and verify their observable output, stderr, and exit status.

### Milestone 4.5 — Canonical coercion object model

Milestone 4 delivered the integer contracts but its flat `checked-coerce`,
`wrapping-coerce`, and `saturating-coerce` spellings contradict the canonical callable
family specified by `docs/surface-v1.md`. This is release-blocking semantic debt, not a
compatibility layer or a deferred additive feature.

Deliver:

- a compiler-owned, statically resolved `.coerce` callable family with throwing default,
  `.checked`, `.wrap`, and `.saturate` policies, using the complete
  `(source type, destination type, policy)` lookup table;
- member-chain resolution that evaluates the receiver exactly once and checks availability
  only after the statically known destination is supplied;
- clean migration of every integer coercion callsite, fixture, diagnostic, and golden to
  `value.coerce; T`, `value.coerce.checked; T`, `value.coerce.wrap; T`, or
  `value.coerce.saturate; T`; the flat spellings are rejected, with no aliases;
- source-oriented rejections for unsupported policy/type pairs and for extracting a
  coercion family as a value before bound-method values exist;
- deterministic lowering that erases the statically resolved family while preserving the
  distinct throwing, partial, wrapping, and saturating result contracts;
- accepted and rejected conformance cases covering every integer policy, receiver
  single-evaluation, canonical generated Rust, and the absence of the obsolete spellings.

Exit criterion: canonical grouped coercion cases compile and run with warnings denied,
flat spellings fail at Terrane source spans, and no compiler-facing documentation or
fixture presents them as valid syntax.

Implemented evidence: the compiler resolves the canonical `.coerce` callable family and
its `.checked`, `.wrap`, and `.saturate` children through one typed policy resolver shared
by semantic analysis and lowering. This is compiler-owned semantic object metadata rather
than four independent direct-invocation paths: lowering erases the resolved family and
policy to backend support calls without exposing those helper names as Terrane members.
Availability is keyed by source type, destination type, and policy; unsupported pairs,
unknown policy chains, escaped family values, and obsolete flat spellings fail at Terrane
source spans. Accepted conformance cases cover every policy, including both absent and
present membership for checked results, a package-level adaptive integer receiver,
parameter-sourced coercion inside a typed function, and a side-effecting function-call
receiver proving exactly-once evaluation. Reviewed generated-Rust goldens preserve the
throwing, partial, wrapping, and saturating result contracts without redundant references,
and every accepted generated crate compiles with warnings denied.

### Milestone 4.6 — Reconcile shipped behavior with the settled decisions

Milestones 3 and 4 shipped against contracts that have since changed. This milestone
closes the gap between what the compiler does and what the specification now says, so that
later work is not built on top of superseded behavior. It adds no new language surface:
every item here is a divergence in something already implemented.

Deliver:

- rename the compiler-owned collection namespace from `/collections` to `/core/collections`
  in the bootstrap module table, the namespace registry, and every diagnostic and fixture
  that names it. The empty placeholder is removed rather than aliased, per the clean-cutover
  rule;
- record descriptor constructs as available without import, a category distinct from the seven
  prelude ordinary bindings, which are unchanged. `value int8 = 42` needs no import today and
  should not start needing one; the specification text stating that fixed widths require
  explicit import and binding predates the construct model and is what changes here. Explicit
  import and aliasing remain available and are still how a name is rebound or shadowed;
- implement descriptors as language constructs backed by canonical objects rather than as
  independently instantiated values. A descriptor binding such as `d = int8` names the
  construct and is a compile-time alias, so a statically resolved use needs no runtime storage
  and lowers to nothing. Today `d = int` passes `terrane check` and emits `d = int;` into
  generated Rust, handing the user a projected rustc `E0425` instead of a Terrane diagnostic —
  the exact failure milestone 6 forbids. The defect is emitting a plain Rust binding as if the
  descriptor were an ordinary value; a descriptor may still be materialised as its canonical
  object where reflection or dynamic descriptor use requires it (milestone 18);
- accept a descriptor alias everywhere the construct is valid — annotation position, a
  coercion destination, and the right side of `is a` — which fixes the currently rejected
  `target-type = float` followed by `x.coerce; target-type` that the specification documents;
- reject a descriptor alias at its source span wherever a runtime value is required, including
  `print; d`, arithmetic, and value parameters, since a descriptor has no display or value
  protocol in version one;
- fix `.concat` lowering when its result is bound. `m = sep.concat; 'a', 'b'` emits
  `m = format!(...)` with no `let`, so the generated Rust does not compile and the user
  receives a projected rustc `E0425`. Binding a `.length` or `.coerce` result emits `let`
  correctly, so the defect is specific to `concat`. The conformance corpus never binds a
  concat result — every fixture passes it straight to `print` — which is why it was not
  caught, so the fix ships with a fixture that binds one;

- align the `/core/errors` object set and its diagnostic text with the structural `error`
  interface now specified — stable `kind`, human-readable `message`, optional `cause`, and a
  source-context chain — even though construction and catching remain later work. Reserved
  names must not imply a shape the specification has since replaced;
- re-check every rejection message written against a superseded spelling. Diagnostics that
  suggest a flat coercion spelling, a `/collections` path, or an operation name the arithmetic
  family decision renamed must be corrected, since a diagnostic is a contract surface;
- refresh `docs/surface-today.md` so each entry matches the reconciled behavior, and
  re-verify the status labels rather than assuming they carried over.

Exit criterion: no compiler-owned name, diagnostic, fixture, or golden refers to a
superseded contract; a descriptor alias is accepted in every construct position and rejected
with a Terrane diagnostic in every value position, never reaching rustc; and
`docs/surface-today.md` agrees with the pipeline entry for entry.

Note on scope: the arithmetic families, abstract category descriptors, structured errors,
and float/string coercion destinations are **not** part of this milestone. They are new
surface rather than corrections, and they arrive with milestones 7 onward.

Implemented evidence: the collection namespace is registered as `/core/collections`. Type
descriptors behave as constructs — a descriptor in value position fails with a Terrane
diagnostic rather than reaching rustc, while construct positions including a coercion
destination accept a descriptor bound under another name. Binding a `.concat` result emits a
correct Rust binding. `/core/errors` exposes `error` as an interface symbol. `surface-today.md`
was refreshed with the rest of the corpus.

### Milestone 4.7 — Namespace path syntax, name casing, and directory correspondence

Milestone 3 shipped whitespace-separated namespace paths with no relationship between a
namespace and its location on disk. Both decisions have been reversed. This milestone
implements the replacement and migrates everything already built on the old form.

Deliver:

- `/` as the single namespace boundary marker, anchoring the root and separating every
  segment: `namespace my-app/http/handlers`, `from /core/output import .print`,
  `from ../shared/config import .settings`. Repeated parents nest as ordinary path
  components, replacing the `.. ..` form, which does not scale and reads as a typo;
- removal of `/` from the identifier-joiner set. A character cannot be both a joiner and the
  namespace separator without making `namespace foo/bar` ambiguous between one segment and
  two, and context-sensitive lexing would contradict the rule that a compact joiner sequence
  is always an identifier. `ipv4/ipv6` becomes `ipv4-ipv6`; update the Rust-name encoding,
  which currently escapes the slash;
- the segment grammar `[a-z]([a-z0-9]|-[a-z0-9])*`, enforced as an allowlist so that every
  filesystem-hazardous character is unformable rather than blocklisted, plus the reserved
  whole-name set `con`, `prn`, `aux`, `nul`, `com1`–`com9`, `lpt1`–`lpt9`, which is made of
  legal characters and therefore invisible to the allowlist. Reserve them now even though
  version one targets Linux first, because adding the restriction later breaks existing names;
- lowercase enforcement for every user-declared name, with uppercase parsing and then failing
  semantically with a diagnostic naming the lowercase form and a formatter fixit. Never fold
  silently: that is the silent repair principle 5 forbids. Type parameters remain uppercase;
- namespace-to-directory correspondence, with a declaration that disagrees with its location
  rejected unless the manifest declares that mapping; manifest namespace-root to
  directory-root mappings resolved by longest prefix; two roots mapped to one directory
  rejected at manifest load;
- bounded discovery over declared roots with sorted expansion, a dependency's namespaces read
  from its own manifest rather than by scanning its tree, and the resolved source set recorded
  in build metadata so a build stays auditable once the manifest no longer enumerates files;
- migration of every fixture, golden, example, and document to the new path syntax.

Also in scope, because it is the same migration:

- implement `string.join`, whose receiver is the separator and whose arguments are the parts,
  distinct from the existing `concat`, which appends without one. It is scheduled here rather
  than with the string families because it depends on nothing: `concat` already lowers
  correctly, and `join` is the same shape with a separator interleaved. It needs no descriptor
  model, callable-family machinery, error propagation, or byte views. Two things make it
  release-blocking earlier than its neighbours — milestone 4 is already delivered and its own
  text-composition example uses `': '.join`, and the specification's representative program,
  the first code any reader meets, uses it too. A flagship example that cannot run is the same
  defect class as the redundant imports removed below. Specify the boundary cases from the
  string-composition section: an empty call yields the empty string, a single argument yields
  that argument with no separator, and the separator never precedes the first or follows the
  last part;
- remove the redundant prelude imports from the conformance corpus. Fixtures currently open
  with `from /core/output import .print` and `print = .print`, but the prelude and descriptor
  constructs already make those unnecessary — `tests/conformance/run/fixed-overflow/case.trn`
  behaves identically with no import lines at all, including its expected `.arithmetic-overflow`
  failure. No compiler change is required; the fixtures teach a pattern the language does not
  need, and every reader copies it. Keep explicit imports only in cases that are specifically
  about importing, aliasing, or shadowing.

Scope warning: the specification and both surface maps already describe the settled version-one
design, which includes the single-lookup-view model that milestone 4.8 delivers. Every import
example in those documents is therefore written without the dot-prefixed form. That form is
still valid until 4.8 lands, so an implementer of this milestone migrates namespace *paths* and
leaves name *forms* alone:

```text
from /core output import .print     ->  from /core/output import .print
```

The path gains its separator; the dot-prefixed name and the `print = .print` binding that
follows it both stay until 4.8. Reading the specification alone will suggest more change than
this milestone authorises.

Exit criterion: no document, fixture, golden, or example uses a whitespace-separated namespace
path; a slash in an identifier is a lexical error; an uppercase or reserved segment fails with
a source-span diagnostic naming the correction; a misplaced source file fails the correspondence
check unless the manifest maps it; the conformance corpus contains no redundant prelude import;
and the specification's representative program compiles and runs, producing its documented
output exactly.

Implemented evidence: namespace declarations and imports now use `/` between every segment,
and declared names are checked for canonical lowercase spelling and reserved namespace
segments. Authored manifests map namespace roots to distinct relative directories; package
loading recursively discovers `.trn` files within those bounded roots in sorted path order,
assigns each declaration its longest-prefix expected namespace, and rejects duplicate,
missing, or mismatched roots before semantic resolution. `string.join` validates and lowers
through the shared call pipeline with empty, singleton, and multi-part conformance coverage.
The migrated corpus has no redundant default-prelude imports, and the representative
program runs with the documented output.

### Milestone 4.8 — Collapse the two-namespace lookup model

Milestone 3 shipped separate ordinary and object-form symbol tables, so an import made
`.print` available and the program then wrote `print = .print` to bind the ordinary name.
That second view is removed. There is one lookup view, and `.` appears only between a
receiver and its member.

Deliver:

- one symbol table and one lookup chain. The `object_form` discriminator on symbols and
  declarations, and the paired table selection it drives, are removed rather than left
  defaulted;
- `from path import name` binding an ordinary name directly in the scope containing the
  import, with `as` renaming it. The declare-then-bind step disappears, and with it every
  `print = .print` line in the corpus;
- a leading `.` in expression position rejected at its source span. This is the diagnostic
  that replaces object-form lookup, so it must name the receiver form rather than reporting an
  unresolved name;
- collision, shadowing, visibility, and idempotent-reimport rules restated over the single
  view. The two-view versions of those rules go away, not merely one of their branches;
- `import with selector` resolving its operand through ordinary lexical scope;
- migration of the reject fixtures that currently encode the two-view model:
  `import-collision` and `private-import` import `.item` and `.secret` and must keep failing
  for the same reasons under the new spelling, while `unresolved-object` currently binds
  `print = .missing` and needs a form that still exercises an unresolved import;
- migration of every remaining fixture, golden, example, and document.

Exit criterion: no source anywhere in the repository contains a leading `.` outside member
position; a program that writes one fails with a diagnostic naming the receiver form; the
collision, visibility, and shadowing cases fail for their original reasons under single-view
lookup; and no compiler type carries an object-form discriminator.

Sequencing note: milestone 4.7 changes namespace *paths* and this milestone changes *name
forms*. Both migrate the whole corpus, so running them back to back keeps the fixture churn
to two passes rather than interleaving it through later work. They are separate milestones
because they are separate language changes with separate exit criteria, not because the
migrations are independent.

### Milestone 5 — Rust IR, readable emission, and Cargo builds

Deliver:

- explicit Rust-oriented lowering IR;
- deterministic module and item ordering;
- injective source-name-to-Rust-name encoding;
- direct fixed-width scalar and function lowering where Rust preserves the source contract;
- integration of the adaptive core-`int` support component into the explicit Rust IR, preserving checked tier promotion, exact wide operations, result normalization, normative runtime failures, and target capability diagnostics;
- structured expression/block emission with a pinned formatter policy, including the nesting threshold at which the formatter reports a call better bound to a named intermediate;
- generated `Cargo.toml`, source tree, compiler metadata, and entrypoint;
- deterministic inclusion of the integer support crate by copying compiler-bundled, content-addressed source into the generated build directory and referring to it by a generated-project-relative Cargo path, without registry, network, or install-location paths; the bundled source content identity enters the build key, and the same vendoring mechanism applies to any authored third-party dependency admitted later;
- content-addressed build directory keyed by compiler version, source inputs, target, and relevant options;
- package-level artifact caching, keyed the same way, so a package compiles once per identity rather than once per dependent build. Delivery principle 9 makes this load-bearing rather than an optimisation: a Terrane-source standard library is recompiled by every build without it;
- `cargo check`, build, and run process wrappers with captured structured output;
- `terrane rust` output or path display suitable for inspection, clearly distinguishing authored generated modules from vendored support source.

Generated artifacts should be organized under a project-local ignored directory or a user cache, never mixed with authored source. A `--keep-generated` or stable development path may expose them intentionally.

Exit criterion: identical inputs produce byte-identical generated files; all accepted compile cases pass Cargo; the generated Rust for representative fixtures is readable and has reviewed goldens. Goldens pin canonical float display at both `float32` and `float64` width for `nan`, `inf`, `-inf`, negative zero, and shortest round-trippable finite values; they pin one multi-argument `print` call proving that arguments render adjacently with no inserted separator and exactly one trailing newline, alongside adjacent `print` calls proving record separation; and generated-project goldens include both authored lowered modules and the vendored support copy.

### Milestone 6 — Source diagnostics across Rust

Deliver:

- basic source associations from semantic nodes to generated Rust spans;
- JSON-formatted Cargo/rustc diagnostic ingestion;
- projection of backend errors to the most relevant Terrane span;
- raw Rust diagnostic retained as a note or opt-in detail;
- stable diagnostic codes and CLI rendering with color policy;
- distinction among source errors, uncaught source-language runtime failures, compiler defects, Rust toolchain failures, and ordinary user-program exits; normative runtime failures render Terrane namespace/function frames and source spans, retain generated Rust frames only as expandable detail, and never surface as raw Rust panics or backtraces;
- internal-error reports that preserve generated artifacts and reproduction metadata.

The frontend should prevent ordinary type/name errors from reaching rustc. Backend translation exists for missed constraints, target failures, generated-code defects, and handwritten/toolchain boundaries—not as a substitute for semantic analysis.

Exit criterion: at least one deliberately induced backend error is mapped to its Terrane source location, and raw rustc information remains available.

### Milestone 7 — Semantic descriptor, protocol, and category model

Deliver:

- compiler-owned abstract category descriptors `number`, `integer`, `fixed-integer`, `signed-fixed-integer`, `unsigned-fixed-integer`, and `floating`, beneath the `value` and `object` identity roots, exported from `/core/types` and never prelude names;
- declared conformance on every concrete scalar descriptor, replacing the enumerated match arms and "is integer" predicates that currently encode category membership;
- category-driven member attachment, compatibility, and finite-union reasoning, so a member set is derived from declared contracts rather than from a per-type list;
- the declared-conversion protocol behind `coerce`, with `coerce` fixed as option-free; the `parse` member taking a required, statically resolvable callback and typed by that callback's declared return, including its `checked` child; and the `radix` pair interpreting base-N text to `int` and rendering `int` to `string`;
- explicit rejection of fixed-width integers as assignment-compatible subtypes of `int`, preserving explicit coercion and the differing arithmetic contracts;
- a compiler-owned descriptor schema carrying bounds, bit width, signedness, and declared protocols, kept separate from generated-Rust representation metadata.

Exit criterion: category membership drives at least one real decision the compiler previously made by enumeration, `is a` answers abstract descriptors correctly, and no scalar is boxed merely to model source conformance.

### Milestone 8 — Callable signatures and bound-method families

Milestone 4.5 delivered the `.coerce` family as a special case. This generalises that machinery so later families reuse it instead of adding parallel special cases.

Deliver:

- general member-family, bound-method, callable-signature, and member-availability semantic forms, with the coercion family re-expressed in terms of them;
- typed child lookup and explicit default invocation in the model rather than in name matching;
- one call-checking path shared by ordinary function calls and member calls;
- member lookup returning typed candidates and availability constraints instead of a boolean "known member", with destination narrowing applied before call validation;
- the version-one restriction that a family selection must be invoked in the same expression, diagnosed at its source span.

Exit criterion: adding a new member family requires no new parser or lowering route, proven by re-expressing coercion and adding one further family through the shared path only.

### Milestone 9 — Structured errors and typed propagation

Deliver:

- the structural `error` interface with stable `kind`, human-readable `message`, optional `cause`, and a source-context chain;
- `throw`, `try`, `catch`, and `finally` over a compiler-owned result propagation representation rather than native unwinding;
- catch matching in source order, with a compile-time diagnostic for a clause made unreachable by an earlier one;
- `finally` semantics that always run and may replace a pending outcome only by explicitly returning or throwing;
- construction and catchability for the reserved `/core/errors` objects, converting the existing deterministic arithmetic and coercion failures onto this path;
- deterministic uncaught rendering of the cause and source chain, preserving the current outermost reporting policy and exit code.

Exit criterion: an arithmetic overflow and a failed coercion are catchable, a rethrow preserves the cause chain, uncaught output is unchanged from the current normative text, and generated Rust contains no panic-based control flow for recoverable failures.

### Milestone 10 — Named bounded-arithmetic families

Deliver:

- the `add`, `subtract`, `multiply`, `divide`, `remainder`, `div-rem`, `negate`, `shift-left`, and `shift-right` families attached to `integer`, with operators selecting each default child;
- `checked`, `wrap`, `saturate`, and `overflowing` children attached to `fixed-integer` only, absent from adaptive `int` rather than present as runtime no-ops;
- `int` exposing its throwing default always and `checked` only where genuinely fallible;
- `overflow-result of T` and `div-rem-result of T` as named compiler-supplied result types, with `div-rem` lowering to one backend operation;
- shift-count policy per receiver class, and postfix `++`/`--` restricted to the default child.

Exit criterion: every family and child has accepted, rejected, and runtime cases; `div-rem` divides once in reviewed generated Rust; and absent children fail at the source span rather than at runtime.

### Milestone 11 — Bytes, string views, and encoding objects

Deliver:

- `bytes` as a real sequence value with literals, byte length, indexing, slicing, and iteration, and with no text-display protocol;
- explicit `bytes`, `scalars`, and `graphemes` string views without changing the default grapheme length;
- the pinned Unicode version contract, sourced from the toolchain profile rather than the package lock;
- canonical `utf8`, `utf16-le`, `utf16-be`, `utf32-le`, and `utf32-be` encoding objects, with encoding total and decoding raising a typed `decode-error` carrying encoding and byte offset;
- prevention of arbitrary bytes reaching `print` through a blanket display implementation.

Exit criterion: a round-trip encode/decode case runs, an invalid byte sequence produces the typed decode error at its offset, and view lengths differ correctly for a multi-scalar grapheme.

### Milestone 12 — String transformation and search families

Deliver:

- the `trim` family with `start` and `end` children, where a child with no argument removes the whitespace run and a child with a literal removes that literal when present;
- the `contains` family with `start` and `end` children, all boolean, plus the separate `find` family returning `text-range|none` with `find.all` and `find.count`;
- `upper` and `lower` with their specified children, `normalise.nfc/nfd/nfkc/nfkd`, and locale-independent `case-fold` as explicitly named Unicode operations;
- literal `split` and `replace` with the settled empty-pattern and non-overlapping-advance rules;
- `text-range` with checked byte, scalar, and grapheme views over an immutable input.

Exit criterion: the specified empty-pattern, position-child, and Unicode-property behaviors each have cases, including a right-to-left sample proving the position children act on logical order.

### Milestone 13 — Iterator protocol

Deliver:

- `iteration-step of Item` with `item of Item` and `end` alternatives as the advancing result;
- stateful linear iterators with sticky `end` that do not consult the source after exhaustion;
- `for` desugared through the protocol without exposing or synthesising sentinel values;
- iterator state and item typing settled before any collection depends on them.

Exit criterion: a user-defined iterator drives `for` through the same path as the built-in string iteration, and an iterator yielding `none` as a legitimate item is distinguished from exhaustion.

### Milestone 14 — Collections

Deliver:

- list, map, set, tuple, range, and entry types under `/core/collections`, added vertically rather than as one batch;
- lookup and indexing whose default child throws `missing-key` or `index-error` and whose `checked` child returns absence, with no operation returning absence by default;
- insertion-ordered map and set as the observable contract, plus a separate unordered type that is deterministic under a fixed hash seed rather than merely unordered;
- half-open ranges with an explicit inclusive constructor, non-zero step, and empty-range rules;
- copy-on-write separation at the first mutation visible through a non-unique handle, with mutable and identity-bearing values rejected as hash keys.

Exit criterion: each collection has parsing, inference, mutation, lowering, and execution evidence; ordering is observable and reproducible across runs for both ordered and unordered variants.

### Milestone 15 — Function values and closures

Deliver:

- first-class function values and closures over the callable protocol established in milestone 8;
- storable bound method families, lifting the version-one restriction diagnosed since milestone 4.5;
- caller-supplied conversion callbacks for pairs no descriptor declares;
- capture semantics defined against the ownership rules, with no implicit boxing of statically known callables.

Exit criterion: a selected method family can be stored, passed, and invoked; the previously rejected form is accepted with a case proving the receiver still evaluates once.

### Milestone 16 — Classes, interfaces, and traits

Deliver:

- class declaration, fields, construction through `construct`, and deterministic drop;
- single class inheritance preserving complete subclass state;
- structural named interfaces and non-type traits with explicit conflict resolution;
- dispatch and compatibility over the descriptor model rather than a parallel class table.

Exit criterion: each of construction, inheritance, interface conformance, and trait reuse has an executable slice; dynamic-object state is preserved end to end.

### Milestone 17 — Ownership, references, and resources

Deliver:

- semantic value assignment for ordinary values, linear resources, and explicit references;
- `ref`, `move`, and weak references with lifetime and provenance analysis reported in source terms;
- the deterministic drop pipeline;
- identity metadata on type contracts, with source `is` never derived from Rust pointer identity.

Exit criterion: ownership analysis accompanies the first non-scalar mutable value type rather than being retrofitted, and borrow escape is diagnosed at the originating binding.

### Milestone 18 — Capabilities, effects, and reflection

Deliver:

- the closed effect vocabulary `throws E`, `io`, `blocks`, `awaits`, `mutates`, `unsafe`, and `foreign`, with allocation tracked internally rather than declared publicly;
- descriptor materialisation: reflection is the case that requires a canonical descriptor object at runtime, so this milestone supplies what milestone 4.6 deliberately does not. A statically resolved descriptor still lowers to nothing; a profile that strips reflection metadata removes the materialisation, not the identity;
- effect inference for private functions and declared public effect contracts for exported ones;
- capability objects that authorize effects without being effects, as linear authority values never synthesised from descriptive profile metadata;
- retained descriptor identity and public callable/type metadata in ordinary profiles, stripped private bodies and unrequested inventories, and a compile-time failure when code requests metadata a profile does not retain.

Exit criterion: a pure caller cannot call an effectful callee, a capability cannot be forged from profile data, and a minimal profile rejects an unavailable reflection request at compile time.

### Milestone 19 — Async core: tasks, scope, cancellation, and deadlines

This milestone precedes the timer, stream, and network milestones because their contracts are defined against it.

Deliver:

- the async callable type, `await`, and task objects, with sync and async callables incompatible without an explicit adapter;
- the structured-concurrency scope: creation, child spawn, join, and defined behavior for a child that throws while siblings run;
- cooperative cancellation with defined cancellation points, reporting completed work rather than silently discarding partial progress;
- deadlines as explicit values that additionally propagate down scope boundaries, where a child may shorten but never extend a parent's deadline;
- the executor boundary the language fixes versus the profile selects, with no hard-coded executor;
- borrow and linear-resource rules across suspension.

Exit criterion: a cancelled scope joins its children and reports partial progress; a nested deadline cannot be extended; and no borrow crosses suspension without a proven lifetime.

### Milestone 20 — Byte and text streams and process standard streams

Written in Terrane over the minimal Rust core, per delivery principle 9. Each layer implemented in Rust states which of the four justifications applies; everything above it is Terrane.


Deliver:

- byte reader and writer protocols with partial-operation and EOF contracts;
- text reader and writer adapters carrying explicit encodings and performing no implicit newline translation;
- typed stdin, stdout, and stderr over the same protocols;
- explicit idempotent close with observable close and flush failures, and `flush` distinguished from `sync-data` and `sync-all`;
- async variants sharing the same contracts with cancellation.

Exit criterion: partial reads and writes, EOF, and use-after-close each have cases; a cancelled stream operation reports what it completed.

### Milestone 21 — Paths, filesystem, and process facilities

Written in Terrane over the minimal Rust core, per delivery principle 9. Each layer implemented in Rust states which of the four justifications applies; everything above it is Terrane.


Deliver:

- lexical `path` values with platform-neutral components and normalization that resolves `..` lexically without crossing a root;
- the capability-gated `filesystem` object with metadata, symlink metadata, canonicalization, and permissions as a portable subset plus profile detail;
- race-resistant directory-handle-relative traversal with no-follow by default and explicit beneath and cross-filesystem policies;
- file handles as linear resources, bounded whole-file operations, and atomic replacement that renames without following links;
- environment and argument access over a lossless platform-string type, the schema-driven CLI parser, and `exit-status` with the `0..=255` code range.

Exit criterion: lexical resolution and filesystem canonicalization are separately observable, a traversal escape attempt is refused, and the CLI parser returns structured diagnostics without calling process exit itself.

### Milestone 22 — Document values, JSON, YAML, and URLs

Written in Terrane over the minimal Rust core, per delivery principle 9. Each layer implemented in Rust states which of the four justifications applies; everything above it is Terrane.


Deliver:

- the shared document-value model with exact `document-integer` and `document-decimal`, never routed through `float`;
- JSON parse, write, and canonical output per RFC 8785 ordering, with duplicate keys rejected by default;
- YAML restricted to a safe core schema with no executable tags and enforced depth, size, and alias-expansion limits;
- descriptor-driven `serializable` and `deserializable` mapping with field names, optional and default fields, unknown-field policy, and full data-path diagnostics;
- parsed `url` values following the pinned WHATWG standard with UTS #46 processing, ordered query entries, and credentials never displayed by default.

Exit criterion: a decode failure reports its document path and expected descriptor; canonical output is byte-identical across runs; a YAML alias bomb is refused by limit.

### Milestone 23 — Randomness, codecs, digests, and compression

Written in Terrane over the minimal Rust core, per delivery principle 9. Each layer implemented in Rust states which of the four justifications applies; everything above it is Terrane.


Deliver:

- incompatible `secure-random` and `pseudo-random of Algorithm` types, with rejection sampling for bounded generation;
- SHA-256 and SHA-512 digests and HMAC, with secret buffers, best-effort zeroisation, and constant-time digest comparison;
- strict hex and distinct standard and URL-safe base64 codecs with explicit padding policy;
- UUID parsing plus v4 and v7 generation;
- `gzip`, `zlib`, `deflate-raw`, and `zstd` codecs with no auto-detect default, deterministic mode, and mandatory output, ratio, nesting, and work limits on decompression.

Exit criterion: a pseudo-random source cannot satisfy a secure-random parameter; a decompression bomb is refused with a distinct resource-limit error rather than truncated success.

### Milestone 24 — Networking and TLS

Written in Terrane over the minimal Rust core, per delivery principle 9. Each layer implemented in Rust states which of the four justifications applies; everything above it is Terrane.


Deliver:

- parsed `ip-address`, `socket-address`, and a distinct `host-name` type, serialising IPv6 per RFC 5952;
- `tcp-listener`, `tcp-stream`, and `udp-socket` type objects owning their factories and returning distinct linear resource instances;
- explicit `dns` lookup returning ordered candidates with TTL, leaving caching to an explicit resolver and connection racing to `connect`;
- deadline and cancellation on every blocking operation, typed socket options, and explicit UDP truncation reporting;
- TLS over the shared stream protocol, defaulting to TLS 1.3 with supported 1.2, performing chain and hostname validation, with any insecure connector requiring a separately imported unsafe capability and remaining visibly typed as insecure.

Exit criterion: a loopback client and server exchange data under a deadline; certificate validation cannot be disabled through an ordinary option; a truncated datagram is reported rather than silently shortened.

### Milestone 25 — Structured logging

Written in Terrane over the minimal Rust core, per delivery principle 9. Each layer implemented in Rust states which of the four justifications applies; everything above it is Terrane.


Deliver:

- an imported, capability-gated `logging` package with named and default loggers and `debug`, `info`, `warning`, and `error` operations;
- immutable field and context enrichment, with fields retaining keys, values, source context, and severity rather than being flattened to text;
- an explicit `log-value` protocol, descriptor-driven redaction, and dot-separated logger hierarchies with filtering before expensive rendering;
- host- or profile-supplied sinks, bounded buffers with an explicit backpressure policy, and a fallback diagnostic sink that does not recursively log;
- an in-memory deterministic test sink with logical sequence numbers and controlled timestamps.

Exit criterion: structured fields survive to the sink unflattened, a secret-typed field is redacted by policy, and captured test output is byte-identical across runs.

### Milestone 26 — Rust dependencies, `reqwest`, and editor integration

Deliver:

- the governing dependency principle, which this milestone implements for Rust and later
  milestones specialise elsewhere: declarations name ecosystems and packages rather than APIs,
  the build resolves the exact package and generates only the boundary machinery Terrane source
  actually crosses, and tooling projects an advisory surface that never decides whether a
  program compiles. No generated adapter layer, no translation of Rust generics into
  instantiations, and no mapping of Rust traits, lifetimes, or error types into the Terrane
  model — those stay in Rust and are touched only inside native Rust bodies. A Terrane-visible
  wrapper is authored deliberately, never produced automatically;
- `use rust crate-name` adding a locked dependency directly to the generated Cargo graph, with exact version or lock-resolved requirement, feature set, default-feature policy, and target conditions declared in the manifest;
- build scripts and proc macros executing only under an explicit build capability and sandbox policy, with their outputs part of cache identity;
- Cargo errors source-mapped to the dependency declaration or native Rust span, without presenting Rust API errors as Terrane member errors;
- the `reqwest` vertical slice with `default-features = false` and explicit `blocking` and `rustls-tls` features plus a chosen roots variant, calling `reqwest::blocking` from a native Rust body with no Terrane request or error bridge;
- language-server integration reading Cargo metadata and lock data and delegating Rust semantic intelligence to rust-analyzer, with cache identity covering manifests, locks, features, target, toolchain, and source checksums.

Exit criterion: accepted and rejected dependency fixtures, lock and feature mismatch diagnostics, deterministic generated Cargo and Rust goldens, and a warning-free local loopback `reqwest` build and run. External-network tests do not prove the contract.

### Milestone 27 — Remaining concurrency and foreign adapters

Deliver:

- channels, mutexes, read/write locks, atomics, and thread-local objects as library objects over the milestone 19 core;
- capability profiles for the system and embedded targets;
- Rust and system adapters with explicit lifetime and error-translation contracts;
- the first Python runtime contract if it remains in version-one scope.

Exit criterion: each surface enters with a selected capability contract, typed objects and effects, deterministic lowering, and compiled and run evidence. No surface is represented as an empty compiler-owned name to make the map look complete.

### Milestone 28 — First-version hardening and release gate

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
2. report `terrane --version`;
3. run unit and conformance tests;
4. verify rejected fixtures and diagnostic goldens;
5. compile every accepted compile fixture with Cargo;
6. execute every run fixture and compare exact stdout, stderr, and exit code;
7. build every file under `examples/`;
8. run `terrane rust` twice for selected cases and compare generated artifacts byte-for-byte;
9. verify no test enumerated, parsed, or built anything under `demos/`;
10. package the `terrane` executable and install that artifact into a second clean environment;
11. compile and run `examples/build-report.trn` using only the installed artifact and Rust toolchain prerequisites.

## 9. Initial feature boundary

### Required in first version

- UTF-8, indentation, all three comment forms, exact spans, and legal empty blocks;
- exact ASCII-only version-one identifier character/joiner policy, spacing-sensitive operators, prefix negation, postfix `++`/`--`, and layered angle-generic rejection;
- normative §34 precedence, associativity, non-associative comparisons, call-free arguments, explicit semicolon calls, and grouping rules;
- a minimal package manifest, manifest-enumerated source units, implicit single-file package identity, namespace declarations, and the fixed version-one bootstrap module table;
- ordinary/object-form lexical lookup distinction, collision/idempotent-import rules, and structural imports unaffected by ordinary bindings;
- locals, namespace bindings, visibility, explicit `global` bindings, definite assignment, and the exact default prelude;
- core literals and static scalar types, including adaptive exact `int`, fixed-width integer contracts, `float` with its explicit `float32`/`float64` widths, the grouped integer `.coerce` family, normative arithmetic/conversion failures, and target capability diagnostics;
- functions, required/optional parameters, positional/named arguments, calls, and return values;
- basic expressions, descriptor-object identity and `.type`, type-membership predicates, assignment, shifts, bitwise operators, and specified evaluation order; ordinary values remain identity-less and `===` is rejected;
- `if`/`else`, `while`, collection and three-clause `for`, `break`, `continue`, and `return`;
- grapheme-defined default string length with capability diagnostics, explicit string views, and the `trim`, `contains`, and `find` families;
- deterministic Rust lowering, a compiler-bundled integer support component usable offline, Cargo build/run, source maps, and diagnostics;
- abstract category descriptors and declared conformance, replacing enumerated category predicates;
- the general callable-family model: member families, bound methods, callable signatures, and typed member availability on one call-checking path;
- structured errors with `throw`/`try`/`catch`/`finally`, catchable core errors, and deterministic uncaught rendering;
- the named bounded-arithmetic families with their policy children and named result types;
- `bytes` as a real value type, explicit encoding objects, and typed decode failures;
- the iterator protocol with `iteration-step of Item`, and list, map, set, tuple, range, and entry under `/core/collections`;
- function values, closures, and storable bound method families;
- classes, single inheritance, structural interfaces, traits, `construct`, and deterministic drop;
- ownership: semantic value assignment, linear resources, explicit `ref`/`move`/weak references, and the drop pipeline;
- the closed effect vocabulary, capability authority objects, and profile-governed reflection retention;
- async with `await`, task objects, the structured-concurrency scope, cooperative cancellation, and scope-propagated deadlines;
- byte and text stream protocols, process standard streams, files, paths, and race-resistant filesystem traversal;
- environment, arguments, the schema-driven CLI parser, and `exit-status`;
- date/time with monotonic timers, deadlines, and tickers;
- the document-value model with JSON, safe YAML, descriptor-driven mapping, and parsed URLs;
- secure and pseudo-random sources, hex and base64 codecs, digests and MACs, UUIDs, and bounded compression;
- networking addresses, DNS, TCP and UDP resources, and validated TLS;
- structured logging over profile sinks;
- direct Rust dependency declaration with a locked Cargo graph, the `reqwest::blocking` slice, and resolution-aware editor integration.

### Explicitly deferred

- source-declared generics and general pattern matching;
- multiple class inheritance and implicit signature overloading or multimethods;
- generators and `yield`;
- labels and `goto`;
- user-replaceable core structural constructs, including `function`;
- a truncating (C/Rust-style) signed integer division and remainder family alongside the specified default contract;
- variadics, if they delay the core pipeline;
- custom declaration modifiers and package-defined type constructors;
- custom importers and registries beyond the locked Cargo graph version one requires;
- C ABI export, and foreign runtimes beyond the first Python contract;
- debugger integration, tracing, and profiling beyond the retained reflection metadata;
- stateful hot-code replacement and time-travel or replay;
- locale-policy-rich text APIs until deterministic policy objects are specified;
- `no_std`, embedded, firmware, and kernel compilation;
- parsing or compiling `demos/fork.trn` as an acceptance goal.

Items moved out of this list by the settled decisions — classes, interfaces, traits, ownership and `ref`/`move`, `throw`/`try`/`catch`/`finally`, float and string coercion destinations, closures, inline Rust with locked Rust dependencies, reflection, and async — are now required above and scheduled in milestones 7 through 27.

Deferral means “diagnose as unsupported,” not “leave behavior accidental.”

### Note on the deferred truncating-division family

Version one specifies exactly one signed division and remainder contract. A later version
may add an explicit truncating pair. It is deferred rather than rejected because the
compatibility case is real even though the performance case is weak.

Performance is not the justification, and this should not be reopened on intuition alone.
Signed division already costs tens of cycles, so the rounding correction is noise beside
it; where both operands are provably non-negative the two contracts agree and the
correction folds away entirely; and for a constant power-of-two divisor the specified
contract is the *cheaper* one, lowering to an arithmetic shift and a mask where truncation
needs sign correction before and after. Reopen this on a measured hot path.

Compatibility is the justification. Porting an algorithm whose reference implementation is
specified in C, Java, Go, or Rust semantics, or implementing a published spec that defines
truncation, otherwise forces a hand-written correction at every division site, which is
both noisy and easy to get wrong.

Constraints on any future design:

- name the operation for its rounding behavior, never for a backend. A "Rust division"
  spelling inherits an underspecified contract at signed minimum divided by `-1` and at a
  zero divisor, and stops being true the moment a second backend exists;
- ship the division and remainder together, because mixing rounding modes across the two
  breaks `quotient * divisor + remainder == dividend`;
- do not cross the rounding axis with the overflow-policy families. Offer truncation at the
  default failure policy only; `checked-truncating-divide` is where the naming scheme fails
  and the combination can be composed by hand;
- no capability gate is warranted. Truncation is well defined, not unsafe, so it belongs in
  the ordinary named-operation tier beside the other explicit families.

### Note on the `.coerce` object and its policy table

Milestone 4.5 establishes one compiler-owned `.coerce` family carrying `.checked`,
`.wrap`, and `.saturate` members, reached through ordinary receiver syntax:

```terrane
value.coerce; int8              # throwing default
value.coerce.checked; int8      # int8|none
value.coerce.wrap; int8
value.coerce.saturate; int8
```

Conversion and overflow policy are independent axes, so flat names make every later
policy-bearing operation family multiply the source surface. The canonical grouping
prevents that growth without pretending that the policies share a result shape.

`.coerce` is one canonical compiler-owned family, not a protocol each type implements
separately. **Which policies exist is a property of the source and destination pair, not
of the family.** Saturating an `int` into a `string` is meaningless because a string has
no range to clamp against, while saturating a `string` into an `int8` can be defined once
text parsing exists. Version one rejects `wrap` and `saturate` for an `int` destination
because the adaptive integer is unbounded. The general model is a table.

Consequences for the implementation:

- applicability is a lookup on `(source type, destination type, policy)` returning a
  result shape or a source-oriented rejection. Version one covers integer sources and
  destinations only; non-integer pairs remain deferred above;
- the check happens on the complete call, not member access, because the destination
  follows policy selection;
- rejections name both types and the policy rather than reporting an unknown member;
- `.coerce` and its policies resolve statically and erase. A bare family outside a call is
  rejected until bound-method values and closures are implemented;
- `.wrap` and `.saturate` are total, `.checked` is partial, and the bare form raises;
- arithmetic retains its distinct named overflow-policy surface because operator syntax is
  its default form; coercion has no operator spelling.

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

- the compiler-side representation of the object protocols the language draft requires in its section 9.6, especially member lookup and coercion: how the compiler answers what members a static type carries, with what contracts, availability conditions, and result shapes, so that member semantics stop being decided by name text independently at each use site. This underlies `.coerce` and its policy table, `.length` and the explicit string views, `.type` and identity, text display, truth, iteration, and equality, so it wants one model rather than one decision per operation;
- representation and implementation strategy for exact adaptive core `int`, including its arbitrary-precision support dependency and target-capability boundary;
- whether the selected collection subset needs facilities beyond the already-required integer support component;
- representation of finite dynamic alternatives in generated Rust;
- generated module boundaries for multiple namespaces in one package;
- source-map encoding between Terrane byte spans and generated Rust spans;
- manifest location and deterministic discovery for multi-unit projects.

Each decision should leave behind executable accepted/rejected cases. Do not use `demos/` to settle these questions because their surrounding unsupported constructs would confound the result.

## 12. Immediate implementation backlog

Milestones 0 through 4.5 are delivered. The next work in order:

1. Rename `/collections` to `/core/collections` throughout the bootstrap table, registry, diagnostics, and fixtures.
2. Implement the descriptor-construct rule: accept an alias in every construct position, reject it at its source span in every value position, and lower a descriptor binding to nothing.
3. Update the specification text that requires explicit import for fixed-width descriptors, and add the construct-availability category alongside the seven prelude bindings.
4. Sweep diagnostics for text written against superseded spellings.
5. Refresh `docs/surface-today.md` and re-verify its status labels.
6. Close milestone 4.6, then continue milestone by milestone, adding real programs only when every construct they contain is supported.

Milestone 4.6 has no unresolved design question left; every item above is implementation. The `coerce` versus `parse` boundary that previously blocked milestone 7 is now settled: `coerce` is the complete built-in conversion surface and never takes options, `parse` always requires a callback and is typed by that callback's declared return, and base-N interpretation is the separate `radix` operation attached by receiver. Milestone 7 additionally delivers `parse` under its version-one restriction that the callback be a statically resolvable function name, and the `radix` pair.

## 13. Definition of done

The first-version compiler is done only when:

- its supported subset is explicit and executable;
- accepted programs are checked, lowered, compiled, and run through one pipeline;
- rejected programs fail at the correct Terrane spans with stable diagnostics;
- generated Rust and Cargo files are deterministic and readable;
- a nontrivial purpose-built CLI program builds from a clean installed compiler;
- tests cover parsing, semantics, lowering, Cargo integration, runtime behavior, and backend diagnostic projection;
- `examples/` contains only programs guaranteed to build;
- `demos/` remains clearly excluded from all support and conformance claims;
- unsupported draft features fail clearly rather than being silently miscompiled.
