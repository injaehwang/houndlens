# houndlens

**Give your AI the full picture. Save your tokens.**

houndlens analyzes your entire project in milliseconds — every file, function, dependency, and impact chain. AI sees the full picture, works faster, and doesn't break things.

## Install

```bash
npm install -g houndlens
```

## Use

### Step 1: Analyze your project

```bash
cd your-project
houndlens
```

Output:
```
  houndlens 11ms | 45 files | 320 functions | 87 types
  Health: 85/100
  Cross-file deps: 142

  Tell your AI: "let's start houndlens"
```

### Step 2: Tell your AI to start

Open your AI tool and say:

| AI tool | What to type |
|---------|-------------|
| Claude Code | `let's start houndlens` |
| Cursor | `let's start houndlens` |
| Gemini | `let's start houndlens` |
| ChatGPT | `let's start houndlens` |
| Windsurf | `let's start houndlens` |
| Any AI | `let's start houndlens` |

Any variation works: `houndlens`, `start houndlens`, `houndlens 시작`, `review houndlens snapshot` — anything mentioning "houndlens".

AI reads the analysis and responds:

> "Project analyzed. 45 files, 320 functions. What would you like to do?"

### Step 3: Work with your AI

Just tell it what you need. AI uses houndlens internally to verify its work.

```
You: "Add empty state handling to all tables"
You: "Fix the login function — it's not handling errors"
You: "Refactor auth service into smaller functions"
```

AI modifies your code, checks for breaking changes, and fixes them automatically.

## How it works

1. `houndlens` creates `.houndlens/snapshot.json` — a complete map of your project
2. AI reads the snapshot and understands every file, function, and dependency
3. When AI modifies code, it runs `houndlens verify` to catch errors
4. If something breaks, AI fixes it before telling you it's done

## Supported languages

Rust · TypeScript · JavaScript · Python

## Performance

| Project size | Time |
|-------------|------|
| 10 files | ~10ms |
| 100 files | ~100ms |
| 1000 files | ~1s |

## License

Apache-2.0 OR MIT
