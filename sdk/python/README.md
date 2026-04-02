# houndlens Python SDK

Python interface for the houndlens code verification engine.

## Install

```bash
pip install houndlens
```

**Prerequisite**: houndlens CLI must be installed (`cargo install houndlens`).

## Usage

### Direct API

```python
from houndlens import Houndlens

lens = Houndlens("/path/to/project")

# Verify changes
result = lens.verify(diff="HEAD~1")
print(f"Status: {result.status}, Risk: {result.risk_score:.0%}")
for change in result.semantic_changes:
    print(f"  [{change.risk}] {change.file}:{change.line} — {change.description}")

# Query the codebase
matches = lens.query("FIND functions WHERE complexity > 15")
for m in matches.matches:
    print(f"  {m.name} at {m.file}:{m.line}")

# Impact analysis
impact = lens.impact("src/auth.rs", function="verify_token")
print(impact.raw_text)

# Discover invariants
for inv in lens.invariants():
    print(inv.description)
```

### LangChain

```python
from houndlens import as_langchain_tools

tools = as_langchain_tools("/path/to/project")
# Add tools to your LangChain agent
```

### OpenAI Function Calling

```python
from houndlens import as_openai_tools

tools = as_openai_tools()
# Pass to: client.chat.completions.create(tools=tools, ...)
```
