# Strata repository guidance

This file contains local working rules for coding agents. It is intentionally ignored by Git while the implementation workflow is still taking shape.

## Sources of truth

- Read `README.md` for the project's purpose and current status.
- For AI retrieval, prefer `docs/dump/language-spec-ai.md`: it is a compact, keyed derivative designed for quickly learning Strata syntax and locating language/compiler contracts. Consult `docs/language-spec-and-compiler-architecture-draft.md` when exact wording, omitted detail, or a conflict matters; the full draft remains authoritative.
- Read `docs/compiler-plan.md` before compiler implementation work.
- Executable conformance cases define what the current compiler actually supports. The design specification may describe capabilities that are not implemented yet.
- Do not use `demos/` as compiler fixtures, smoke tests, or evidence of implemented behavior. Demos are exploratory design pressure tests and may contain unsupported constructs.

## Branches and milestones

- Implement each compiler milestone on its own branch.
- Name branches after the capability being delivered, not after a milestone number. Prefer names such as `compiler-toolchain-skeleton`, `indentation-lexer`, or `namespace-resolution`; do not use names such as `milestone-0` or `m1-lexer`.
- A branch must have one clear end-to-end outcome and must satisfy the relevant milestone exit criterion before it is considered complete.
- Do not bundle work from a later milestone merely because nearby code is convenient to touch. Small prerequisite work is acceptable when it is required by the branch's stated capability.

## Work units and commits

- Divide a milestone into coherent work units before implementation. A work unit should deliver one reviewable capability or contract, preferably as a vertical slice rather than an isolated layer.
- Give every completed work unit its own commit. Do not collapse an entire milestone into one commit, and do not combine unrelated work units in one commit.
- Name commits for the behavior or capability delivered, not for the milestone number or planning bookkeeping.
- Each commit should leave the branch buildable and its relevant targeted checks passing. If a work unit necessarily spans several internal layers, keep those layers together in the same commit so the commit does not knowingly introduce a broken intermediate state.
- Keep mechanical changes, dependency updates, and behavioral changes separate when they can be reviewed independently.
- Do not commit placeholders, knowingly unsupported success paths, disabled checks, or source-text substitutions that imitate compilation.

## Implementation boundaries

- Build vertical slices through the real pipeline: source, tokens/syntax, resolution and semantics where applicable, deterministic Rust lowering, Cargo, and execution.
- Keep `check`, `rust`, `build`, and `run` on one compiler pipeline. Do not create command-specific parsers or semantic paths.
- Keep source files, spans, tokens, syntax nodes, semantic models, diagnostics, and stable diagnostic codes compiler-owned even when parser libraries are used.
- Reject unsupported or ambiguous source with a source-oriented diagnostic. Never silently reinterpret it as a nearby supported construct.
- Do not introduce a universal boxed runtime value as a shortcut for statically known values.
- Generated Rust is a public debugging surface: keep it readable, deterministic, and traceable to Strata source.
- Do not implement speculative future capabilities until their dependent milestone or an explicit task requires them.

## Tests and evidence

- Start a language feature with focused accepted and plausible rejected conformance cases.
- Test observable contracts and meaningful malformed boundaries, not implementation trivia.
- Review generated-Rust goldens; never accept snapshot changes blindly.
- Compile generated crates for accepted compile cases and run programs whose behavior changed. Parsing or plausible-looking generated text alone is not completion evidence.
- Keep fixtures minimal so each case isolates one language decision. Do not copy `demos/` into the conformance corpus.
- Run the narrowest relevant checks during a work unit, then run the complete milestone-relevant suite before declaring the branch complete.
- Treat rustc and Clippy warnings as failures in compiler-owned Rust and in generated Rust for every source program expected to compile successfully. Run the applicable lowered crate with warnings denied; warnings in generated output usually indicate a lowering defect and must not be dismissed merely because the compiler itself is clean.
- Do not hide generated-code warnings with blanket lint allowances such as `allow(warnings)` or `allow(unused)`. An intentionally erroneous or warning-focused case may expect its specific failure, and a narrowly scoped generated allowance may be justified by a documented language or ABI contract, but neither may conceal unrelated warnings.
- Update claims about implemented features only after the end-to-end conformance case passes.

## Documentation discipline

- When implementation exposes an unresolved language question, settle it with a small prototype and accepted/rejected cases before updating the specification.
- Update the language document when a semantic decision changes. Update the implementation plan when sequencing, scope, or milestone evidence changes.
- Keep `docs/dump/language-spec-ai.md` synchronized whenever `docs/language-spec-and-compiler-architecture-draft.md` changes semantics, grammar, architecture, invariants, validation points, or deferred scope. Update both in the same work unit; never let the AI reference become an independent source of truth.
- If the compact AI reference has a gap, ambiguity, or retrieval failure that requires consulting the full specification, improve the compact reference in the same work unit when the full specification resolves it. Add the smallest durable clarification or retrieval key that would prevent the same failure; compress or replace existing text instead of allowing the derivative to grow without bound.
- Keep the README oriented toward project purpose, capabilities, and honest status; direct technical detail to the language document.
- Do not describe planned behavior as implemented behavior.

## Scope and maintenance

- Prefer the smallest coherent change that completely satisfies the current work unit.
- Reuse existing conventions rather than introducing parallel representations or workflows.
- Remove obsolete paths during a clean cutover; do not leave compatibility aliases unless explicitly required.
- Treat unexpected working-tree changes as user work and preserve them.
- Revise this file when real implementation experience proves a rule unhelpful. It is guidance for reliable work, not a substitute for evidence.
