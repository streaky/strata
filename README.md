# Terrane

Terrane is an experimental programming language for writing high-level native software without making low-level representation machinery part of everyday programming.

The project aims to offer one approachable language across ordinary applications, libraries, services, command-line tools, WebAssembly, embedded systems, firmware, and kernels. Terrane is intended to feel lightweight when a program is simple while allowing types, ownership, visibility, platform capabilities, and other constraints to be stated where they matter.

![Source-to-native FLow Diagram](docs/images/source-to-native-program-flow.png "Source-to-native Flow Diagram")

## Semantics and representation

Terrane is built around a simple distinction: what a value means in the language is not the same thing as how it must be represented at runtime.

Everything is an object semantically. An integer is an object, a function is an object, a type is an object. But object semantics do not require object-shaped runtime machinery. If an integer can be realised directly as a machine value, Terrane may lower it to one. If a call can be resolved statically, it may become a direct Rust call. If value semantics can be preserved without a physical copy, the compiler is free to avoid one.

The programmer describes semantics; the compiler chooses representation. The aim is to keep the conveniences normally associated with high-level languages while compiling away machinery that does not need to survive into the final program, with the generated Rust left in place as the receipt.

## Why Terrane?

Modern software often asks developers to cross several language, build-system, and runtime boundaries as a project grows. Terrane explores a different approach: keep the human-facing language coherent, lower onto the mature Rust and native-code ecosystem rather than a bespoke runtime black box, and make the boundary between convenient source and deployed software visible rather than magical.

Its guiding priorities are:

- **Semantic objects, efficient representation.** Everything is an object at the language level, but the compiler is free to realise values as native scalars, direct calls, enums, borrowed values, or other efficient representations when doing so preserves observable behaviours.
- **Readable everyday code.** Common syntax should favour clear words and familiar control flow over punctuation-heavy ceremony.
- **Progressive strictness.** Start with concise bindings and add precise contracts at the function, type, package, or build level when needed.
- **Native reach.** The same language should be able to target applications, libraries, WebAssembly, embedded devices, and systems software when the program uses capabilities available in that environment.
- **Ecosystem access.** Rust crates, native libraries, platform APIs, and carefully isolated foreign runtimes should be usable without turning Terrane into an island.
- **Inspectable lowering.** Generated Rust is not hidden compiler exhaust: it is the public lowered form, available for auditing, debugging, profiling, and understanding why the compiler made a particular representation, allocation, copy, or performance choice.
- **Explicit power.** Shared identity, ownership transfer, unsafe operations, and platform-specific facilities should be visible choices rather than hidden surprises.

## Why Rust underneath?

Designing a new source language does not require designing a new execution platform.

Mature dynamic languages require substantial runtimes because much of their language machinery remains active between source code and execution: object representation, dispatch, memory management, interpreters or virtual machines, extension interfaces, platform integration, and often a JIT. Rebuilding all of that would consume enormous effort while producing machinery that already exists elsewhere.

Terrane takes a different approach. The project concentrates on the part it actually intends to change - the human-facing language and its semantics - and lowers those semantics to Rust. Rust already provides a mature native compilation ecosystem, optimisation, ownership machinery, platform support, linking, C interoperability, WebAssembly, `no_std`, debugging, and access to a large systems ecosystem.

This also means Terrane does not need every high-level abstraction to survive as runtime machinery. An object may become a scalar, a dynamic call may become a direct call, an independent value assignment may require no copy, and an exact numeric conversion may become a machine operation. What matters is preserving Terrane semantics, not preserving their most general implementation.

Rust is therefore not hidden underneath Terrane as an implementation accident. It is the deliberate lower layer: **Terrane is where the program is expressed; Rust is where its concrete native realisation can be inspected or taken over when necessary.**

Terrane is a source language designed to behave like a semantic layer over Rust rather than a separate execution world. The boundary between the two is deliberate and useful. Terrane semantics do not always map one-for-one onto Rust, which gives the compiler room to enforce stronger source guarantees, translate runtime failures into Terrane-level errors, select efficient representations, and remove machinery that is no longer observable.

Rust also provides a much safer foundation for the execution layer itself. Building a bespoke runtime means taking responsibility for memory management, object lifetime, dispatch, concurrency, native-extension boundaries, buffer handling, platform integration, and a large amount of security-sensitive infrastructure. Rust cannot eliminate every class of bug, especially across unsafe and foreign interfaces, but it removes a substantial amount of memory-safety risk from the default implementation model.

The alternative is therefore not merely the engineering cost of building something on the scale of CPython or Zend. It is also the long-term security, compatibility, optimisation, and maintenance burden of owning an execution platform. Terrane instead spends its complexity on language semantics and lowering, while inheriting a mature native toolchain and a safer implementation substrate.

Terrane compiles at development and build boundaries, not at execution boundaries. Its lowering is deterministic and incrementally cacheable, and the generated Rust participates in the existing Cargo/rustc caching ecosystem. Large native programs still take time to build because large native programs take time to build; that cost is paid when the program changes, not every time it runs.

## Project status

Terrane is still in design, but the compiler has progressed well beyond the prototype-parser stage. The language document describes the proposed full contract; the implemented subset is intentionally smaller.

The working `terrane` CLI can check, lower, build, and run manifest-backed programs through deterministic generated Rust and Cargo. One shared pipeline now covers the lexer and lossless parser, packages and namespaces, typed semantics, explicit Rust IR, Cargo build caching, structured Terrane errors, and compiler-owned member families. The implemented language surface includes native scalars, exact adaptive integers, fixed-width arithmetic policies and named results, typed bindings and calls, control flow, descriptors, strings and grapheme iteration, immutable bytes, explicit Unicode text views and encodings, and the version-one string transformation and search families. A manifest-driven conformance corpus exercises accepted, rejected, generated-Rust, and runtime contracts; later collections, general iterator protocols, ownership, concurrency, and platform capabilities remain planned rather than implemented.

## Learn more

The [language specification and compiler architecture draft](docs/language-spec-and-compiler-architecture-draft.md) is the main source for syntax, semantics, examples, interoperability, tooling, and other technical details.

The [first-version compiler plan](docs/compiler-plan.md) describes the implementation milestones and the capabilities targeted for the first usable release.

The `demos/` directory contains exploratory design exercises. These files deliberately stress ambitious or unfinished ideas and should not be read as examples of features already supported by a compiler.

Editor support lives under `editors/`. The VS Code extension launches the Rust `terrane-language-server`, which reuses the recovering compiler frontend for semantic highlighting and source diagnostics.
