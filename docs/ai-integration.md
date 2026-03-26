# AI Agent Integration Guide

omnilens is designed to be called by AI coding agents (Claude, GPT, Cursor, Devin, etc.) as a verification tool. This document explains all integration methods.

## 1. MCP Server (Claude Desktop / Claude Code)

The fastest path for Claude-based agents.

### Setup

```json
// claude_desktop_config.json or .mcp.json
{
  "mcpServers": {
    "omnilens": {
      "command": "npx",
      "args": ["@omnilens/mcp-server"],
      "cwd": "/path/to/your/project"
    }
  }
}
```

### Available tools

| Tool | Purpose | Key args |
|------|---------|----------|
| `omnilens_verify` | Verify code changes | `diff`, `files`, `cwd` |
| `omnilens_impact` | Impact analysis | `file`, `function`, `depth` |
| `omnilens_query` | OmniQL query | `query`, `cwd` |
| `omnilens_invariants` | Discover patterns | `cwd` |
| `omnilens_index` | Build index | `cwd` |

### Example agent workflow

```
1. Agent generates code changes
2. Agent calls omnilens_verify(diff="HEAD")
3. If risk_score > 0.5 or breaking > 0:
   - Agent calls omnilens_impact(file, function) for each breaking change
   - Agent adjusts code based on impact analysis
4. Agent calls omnilens_verify again to confirm fixes
5. Agent uses suggested_tests to generate test code
```

## 2. CLI (any agent with shell access)

Any AI agent that can execute shell commands can use omnilens directly.

### Verification loop pattern

```bash
# Step 1: Index the project (only needed once per session)
omnilens index

# Step 2: After generating code, verify changes
omnilens --format json verify --diff HEAD~1

# Step 3: Parse JSON to check status
# If status == "fail" or risk_score > 0.5, investigate further

# Step 4: Investigate specific concerns
omnilens impact src/changed_file.rs --fn changed_function

# Step 5: Query for related patterns
omnilens query "FIND functions WHERE calls(changed_function)"
```

### JSON output parsing

```python
import json, subprocess

result = json.loads(subprocess.check_output(
    ["omnilens", "--format", "json", "verify", "--diff", "HEAD~1"]
))

if result["status"] == "fail":
    for change in result["semantic_changes"]:
        if change["risk"] == "Breaking":
            print(f"BREAKING: {change['description']} at {change['file']}:{change['line']}")

for test in result["suggested_tests"]:
    print(f"Missing test: {test['description']}")
    if test.get("skeleton"):
        print(test["skeleton"])
```

## 3. GitHub Action (CI/CD agents)

For agents that operate through pull requests.

```yaml
# .github/workflows/omnilens.yml
- uses: omnilens/omnilens@v1
  with:
    comment: "true"    # Post results as PR comment
    fail-on: "error"   # Gate the PR
```

The action:
1. Runs `omnilens verify --diff <base>...<head>`
2. Uploads SARIF to GitHub Code Scanning
3. Posts a structured PR comment with risk score, changes, and test suggestions
4. Exits with code 1 if breaking changes detected

## 4. Programmatic Rust API

For agents built in Rust or compiled tools.

```rust
use omnilens_core::Engine;
use omnilens_core::verify::DiffSpec;
use omnilens_frontend_rust::RustFrontend;

let mut engine = Engine::init(project_path)?;
engine.register_frontend(Box::new(RustFrontend::new()));

// Index
engine.index()?;

// Verify
let result = engine.verify(&DiffSpec::GitDiff {
    base: "HEAD~1".into(),
    head: "HEAD".into(),
})?;

println!("Risk: {}", result.risk_score);
println!("Breaking changes: {}", result.semantic_changes.iter()
    .filter(|c| matches!(c.risk, ChangeRisk::Breaking))
    .count());

// Query
let qr = omnilens_query::run_query(&engine.graph, "FIND functions WHERE complexity > 15")?;
for m in &qr.matches {
    println!("{} at {}:{}", m.name, m.file, m.line);
}
```

## 5. Decision matrix for AI agents

When should an AI agent call omnilens?

| Situation | Action |
|-----------|--------|
| After generating new code | `omnilens_verify(diff="HEAD")` |
| Before modifying a function | `omnilens_impact(file, function)` |
| Need to understand codebase rules | `omnilens_invariants()` |
| Looking for specific patterns | `omnilens_query("FIND ...")` |
| PR review automation | GitHub Action with SARIF upload |
| Checking if tests are needed | Read `suggested_tests` from verify output |

## 6. Risk thresholds for agents

Recommended thresholds for autonomous agents:

| Risk score | Action |
|-----------|--------|
| 0.0 - 0.2 | Auto-approve, safe to proceed |
| 0.2 - 0.5 | Proceed with caution, flag for review |
| 0.5 - 0.8 | Stop and ask human, likely breaking changes |
| 0.8 - 1.0 | Do not proceed, critical issues detected |

| Change risk | Agent behavior |
|------------|----------------|
| `Safe` | No action needed |
| `NeedsReview` | Log the change, continue if autonomous |
| `Breaking` | Stop, analyze impact, suggest fix |
| `SecuritySensitive` | Always flag for human review |
