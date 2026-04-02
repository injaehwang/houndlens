# houndlens — AI-Native Code Verification Engine

## What is this project?

houndlens is a Rust-based CLI tool that performs **semantic code analysis** across Rust, TypeScript/JavaScript, and Python. It parses source code into a Universal Semantic IR (USIR) using tree-sitter, builds a cross-file semantic graph, and provides impact analysis, invariant discovery, semantic diffing, and a query language (HoundQL).

The core thesis: AI generates code faster than humans can verify it. houndlens closes that gap.

## Project structure

```
Cargo.toml                          # 14-crate Rust workspace
crates/
  houndlens-cli/                     # Binary entry point, 10 subcommands
  houndlens-core/                    # Engine orchestration, verify pipeline, invariant discovery
  houndlens-ir/                      # Universal Semantic IR: Node, Edge, Type, Invariant, Contract
  houndlens-graph/                   # petgraph-based semantic graph + impact analysis + linker
  houndlens-index/                   # File discovery + incremental indexing (content-addressed)
  houndlens-storage/                 # Persistent storage (redb + content-addressed objects)
  houndlens-query/                   # HoundQL parser + executor
  houndlens-frontend-rust/           # tree-sitter-rust → USIR
  houndlens-frontend-typescript/     # tree-sitter-typescript → USIR
  houndlens-frontend-python/         # tree-sitter-python → USIR
  houndlens-testgen/                 # Test generation engine (stub)
  houndlens-runtime/                 # Runtime profiler (stub)
  houndlens-lsp/                     # LSP server (stub)
  houndlens-plugin/                  # WASM plugin system (stub)
docs/
  vision.md                         # AI-native testing vision document
  architecture.md                   # 9-layer architecture with data flow
  ROADMAP.md                        # 4-phase roadmap
  tech-decisions.md                 # 7 ADRs
action.yml                          # GitHub Action definition
.github/workflows/houndlens.yml      # Example CI workflow
```

## Key commands

```bash
houndlens init                                    # Initialize in current project
houndlens index                                   # Build semantic index
houndlens impact <file> --fn <name> --depth N     # Bidirectional impact analysis
houndlens verify --diff HEAD~1                    # Semantic diff + invariant check
houndlens verify --format json --diff HEAD~1      # JSON output for CI
houndlens verify --format sarif --diff HEAD~1     # SARIF for GitHub Code Scanning
houndlens invariants                              # Auto-discover codebase patterns
houndlens query "FIND functions WHERE ..."        # HoundQL query
```

## Build and test

```bash
cargo build              # Build all crates
cargo test               # Run 26 tests (Rust 6, TS 7, Python 5, HoundQL 8)
cargo run -- index       # Self-dogfood: index houndlens itself
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
                              Impact Analysis   Invariant Discovery   HoundQL
                                    ↓                 ↓                 ↓
                              Semantic Diff     Violation Check     Query Results
                                    ↓                 ↓                 ↓
                                    └─────────────────┼─────────────────┘
                                                      ↓
                                              Output (Text/JSON/SARIF)
```

## HoundQL syntax

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
- Frontend trait: `LanguageFrontend` in `houndlens-core/src/frontend.rs`
- Node IDs are globally unique `u64` via `AtomicU64` counters (per-frontend range)
- Placeholder nodes have `complexity: None` — resolved by the linker
- Paths normalized to forward slashes for cross-platform matching
