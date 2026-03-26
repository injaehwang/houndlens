# omnilens — AI-Native Code Verification Engine

## What is this project?

omnilens is a Rust-based CLI tool that performs **semantic code analysis** across Rust, TypeScript/JavaScript, and Python. It parses source code into a Universal Semantic IR (USIR) using tree-sitter, builds a cross-file semantic graph, and provides impact analysis, invariant discovery, semantic diffing, and a query language (OmniQL).

The core thesis: AI generates code faster than humans can verify it. omnilens closes that gap.

## Project structure

```
Cargo.toml                          # 14-crate Rust workspace
crates/
  omnilens-cli/                     # Binary entry point, 10 subcommands
  omnilens-core/                    # Engine orchestration, verify pipeline, invariant discovery
  omnilens-ir/                      # Universal Semantic IR: Node, Edge, Type, Invariant, Contract
  omnilens-graph/                   # petgraph-based semantic graph + impact analysis + linker
  omnilens-index/                   # File discovery + incremental indexing (content-addressed)
  omnilens-storage/                 # Persistent storage (redb + content-addressed objects)
  omnilens-query/                   # OmniQL parser + executor
  omnilens-frontend-rust/           # tree-sitter-rust → USIR
  omnilens-frontend-typescript/     # tree-sitter-typescript → USIR
  omnilens-frontend-python/         # tree-sitter-python → USIR
  omnilens-testgen/                 # Test generation engine (stub)
  omnilens-runtime/                 # Runtime profiler (stub)
  omnilens-lsp/                     # LSP server (stub)
  omnilens-plugin/                  # WASM plugin system (stub)
docs/
  vision.md                         # AI-native testing vision document
  architecture.md                   # 9-layer architecture with data flow
  ROADMAP.md                        # 4-phase roadmap
  tech-decisions.md                 # 7 ADRs
action.yml                          # GitHub Action definition
.github/workflows/omnilens.yml      # Example CI workflow
```

## Key commands

```bash
omnilens init                                    # Initialize in current project
omnilens index                                   # Build semantic index
omnilens impact <file> --fn <name> --depth N     # Bidirectional impact analysis
omnilens verify --diff HEAD~1                    # Semantic diff + invariant check
omnilens verify --format json --diff HEAD~1      # JSON output for CI
omnilens verify --format sarif --diff HEAD~1     # SARIF for GitHub Code Scanning
omnilens invariants                              # Auto-discover codebase patterns
omnilens query "FIND functions WHERE ..."        # OmniQL query
```

## Build and test

```bash
cargo build              # Build all crates
cargo test               # Run 26 tests (Rust 6, TS 7, Python 5, OmniQL 8)
cargo run -- index       # Self-dogfood: index omnilens itself
```

## Architecture: data flow

```
Source Files → tree-sitter → Language Frontend → USIR Nodes/Edges
                                                      ↓
                                              Semantic Graph (petgraph)
                                                      ↓
                                              Cross-file Linker
                                                      ↓
                                    ┌─────────────────┼─────────────────┐
                                    ↓                 ↓                 ↓
                              Impact Analysis   Invariant Discovery   OmniQL
                                    ↓                 ↓                 ↓
                              Semantic Diff     Violation Check     Query Results
                                    ↓                 ↓                 ↓
                                    └─────────────────┼─────────────────┘
                                                      ↓
                                              Output (Text/JSON/SARIF)
```

## OmniQL syntax

```sql
FIND <target> WHERE <condition> (AND <condition>)*

Targets: functions | types | modules | bindings | all
Fields:  name, visibility, complexity, params, fields, async, unsafe, file, kind
Ops:     = | != | > | < | >= | <= | ~ (glob)
Preds:   calls(x), called_by(x), returns(x), implements(x), has_field(x), in_file(x)
Logic:   AND, NOT
```

## Conventions

- Rust edition 2024, minimum rust-version 1.85
- All public APIs documented with `///` doc comments
- Frontend trait: `LanguageFrontend` in `omnilens-core/src/frontend.rs`
- Node IDs are globally unique `u64` via `AtomicU64` counters (per-frontend range)
- Placeholder nodes have `complexity: None` — resolved by the linker
- Paths normalized to forward slashes for cross-platform matching
