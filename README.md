# Strata

Strata is an experimental programming language for building native software without making low-level machinery part of everyday programming.

The project aims to offer one approachable language across ordinary applications, libraries, services, command-line tools, WebAssembly, embedded systems, firmware, and kernels. Strata is intended to feel lightweight when a program is simple while allowing types, ownership, visibility, platform capabilities, and other constraints to be stated where they matter.

## Why Strata?

Modern software often asks developers to cross several language, build-system, and runtime boundaries as a project grows. Strata explores a different approach: keep the human-facing language coherent, build on the mature Rust and native-code ecosystem, and make the boundary between convenient source and deployed software visible rather than magical.

Its guiding priorities are:

- **Readable everyday code.** Common syntax should favour clear words and familiar control flow over punctuation-heavy ceremony.
- **Progressive strictness.** Start with concise bindings and add precise contracts at the function, type, package, or build level when needed.
- **Native reach.** The same language should be able to target applications, libraries, WebAssembly, embedded devices, and systems software when the program uses capabilities available in that environment.
- **Ecosystem access.** Rust crates, native libraries, platform APIs, and carefully isolated foreign runtimes should be usable without turning Strata into an island.
- **Inspectable behaviour.** Generated code, allocations, copies, diagnostics, source mappings, and performance decisions should remain understandable.
- **Explicit power.** Shared identity, ownership transfer, unsafe operations, and platform-specific facilities should be visible choices rather than hidden surprises.

## Project status

Strata is in the design and early compiler stage. The language document describes the proposed full contract; the implemented subset is intentionally much smaller.

The working `strata` CLI can check, lower, build, and run the milestone-zero hello program through generated Rust and Cargo. Development is growing that coherent subset through executable examples and conformance cases.

## Learn more

The [language specification and compiler architecture draft](docs/language-spec-and-compiler-architecture-draft.md) is the main source for syntax, semantics, examples, interoperability, tooling, and other technical details.

The [first-version compiler plan](docs/compiler-plan.md) describes the implementation milestones and the capabilities targeted for the first usable release.

The `demos/` directory contains exploratory design exercises. These files deliberately stress ambitious or unfinished ideas and should not be read as examples of features already supported by a compiler.

Editor support currently lives under `editors/`; a VS Code extension provides basic Strata syntax highlighting while the language and toolchain take shape.