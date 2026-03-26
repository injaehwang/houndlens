# omnilens — Copilot Instructions

This is omnilens, an AI-native code verification engine in Rust.

## Project basics
- 14-crate Cargo workspace
- 3 language frontends: Rust, TypeScript, Python (tree-sitter based)
- Key command: `omnilens verify --diff HEAD~1` (semantic diff)
- Query language: `omnilens query "FIND functions WHERE complexity > 10"`

## When generating code for this project

### Rust code
- Use edition 2024 features
- All IR types must derive `Serialize, Deserialize`
- Frontend NodeId ranges: Rust 1+, TypeScript 100_000+, Python 200_000+
- Placeholder nodes: `complexity: None`, resolved by `linker.rs`

### New language frontend
1. Implement `LanguageFrontend` trait from `omnilens-core/src/frontend.rs`
2. Use tree-sitter for parsing, convert to `UsirNode`/`UsirEdge`
3. Register in `omnilens-cli/src/commands/mod.rs`

### OmniQL queries
Syntax: `FIND <target> WHERE <conditions>`
- Targets: functions, types, modules, bindings, all
- Comparisons: name, visibility, complexity, params, fields, async, unsafe, file, kind
- Predicates: calls(x), returns(x), implements(x), has_field(x)
- Operators: = != > < >= <= ~ (glob)
- Logic: AND, NOT

### Testing
Run `cargo test` — 26 tests across 4 crates. Each frontend has its own test suite.
