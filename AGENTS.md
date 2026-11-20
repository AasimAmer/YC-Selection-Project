# FAULTLINE Codex development rules

Read `docs/01_FAULTLINE_PROJECT_BLUEPRINT.md` and `docs/02_TWO_PERSON_EXECUTION_PLAYBOOK.md` before changing code.

## Architecture invariants
- Dependency edges are `consumer -> dependency`; impact traverses reverse edges.
- Tree-sitter is syntactic: do not invent unresolved semantic bindings.
- Public DTOs and `EngineApi` live in `crates/faultline-contract/` and are frozen at `contract-v1` after bootstrap.
- TUI/CLI/evaluation call `EngineApi`; no parser or internal graph types leak into these crates.
- Each response is bound to a snapshot epoch; heuristic impact score is not a probability of failure.
- No false performance or retrieval claims: report measured data only.

## Two independent owners
- Engine owner edits: `crates/faultline-parser/`, `faultline-core/`, `faultline-index/`, `faultline-engine/`.
- Experience owner edits: `crates/faultline-cli/`, `faultline-tui/`, `faultline-eval/`, `faultline-testkit/`, `fixtures/`, `docs/` and approved `.github/`.
- Do not edit another owner's files, `faultline-contract/`, root Cargo manifest or lockfile as part of a feature task. Propose tiny separately reviewed contract/infrastructure PRs.

## Per-task checklist
1. Choose a bounded A# or B# work packet from the execution playbook.
2. Read API and fixtures. Explain assumptions and acceptance criteria.
3. Edit only owned files; add minimal regression tests for expected and failure paths.
4. Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` when scaffold is present.
5. Report modified files, test evidence, unresolved concerns and exact user-visible behavior in PR.
6. Never check secrets into git or execute arbitrary repository scripts during indexing.
