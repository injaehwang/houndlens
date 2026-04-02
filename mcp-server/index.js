#!/usr/bin/env node

/**
 * houndlens MCP Server
 *
 * Task-oriented tools for AI agents — not raw CLI wrappers.
 *
 * Tools:
 *   houndlens_before_edit  — everything AI needs to know before modifying a file
 *   houndlens_check_work   — verify AI's changes, report what to fix
 *   houndlens_plan_change  — plan what files need changing for a task
 *   houndlens_file_context — full context for a specific file
 *   houndlens_query        — search codebase with HoundQL
 */

import { execSync } from "child_process";
import { readFileSync, existsSync } from "fs";
import { createInterface } from "readline";
import { join } from "path";

// ─── Tools ──────────────────────────────────────────────────────

const TOOLS = [
  {
    name: "houndlens_before_edit",
    description:
      "Call this BEFORE modifying a file. Returns: what functions are in the file, who calls them, what depends on them, and what will break if you change them. Prevents blind edits.",
    inputSchema: {
      type: "object",
      properties: {
        file: {
          type: "string",
          description: "File path to inspect before editing.",
        },
        function: {
          type: "string",
          description: "Specific function to check (optional — if omitted, checks all functions in the file).",
        },
      },
      required: ["file"],
    },
  },
  {
    name: "houndlens_check_work",
    description:
      "Call this AFTER modifying files. Rescans the project (~10ms), runs all project tools (tsc, eslint, pytest, etc.) on changed files only, and returns a single report: what's broken, what to fix. If result has errors, fix them and call this again.",
    inputSchema: {
      type: "object",
      properties: {
        diff: {
          type: "string",
          description: 'Git ref to compare against. Default: "HEAD".',
          default: "HEAD",
        },
      },
    },
  },
  {
    name: "houndlens_plan_change",
    description:
      "Call this BEFORE starting a task. Given a description of what you want to change, returns: which files need to be modified, in what order, and what dependencies exist between them.",
    inputSchema: {
      type: "object",
      properties: {
        target: {
          type: "string",
          description: 'What you want to change. E.g., "add rememberMe to login", "rename UserService to AuthService".',
        },
        function: {
          type: "string",
          description: "Specific function involved (optional).",
        },
      },
      required: ["target"],
    },
  },
  {
    name: "houndlens_file_context",
    description:
      "Get the full context of a file without reading its source: all functions with signatures, types, imports, who calls what, and cross-file dependencies. Saves tokens compared to reading the file.",
    inputSchema: {
      type: "object",
      properties: {
        file: {
          type: "string",
          description: "File path to get context for.",
        },
      },
      required: ["file"],
    },
  },
  {
    name: "houndlens_query",
    description:
      'Search the codebase semantically. Examples: "FIND functions WHERE complexity > 15", "FIND functions WHERE calls(db.query)", "FIND types WHERE fields > 5".',
    inputSchema: {
      type: "object",
      properties: {
        query: {
          type: "string",
          description: "HoundQL query string.",
        },
      },
      required: ["query"],
    },
  },
];

// ─── Tool implementations ───────────────────────────────────────

function run(args, cwd) {
  try {
    return execSync(`houndlens ${args}`, {
      cwd: cwd || process.cwd(),
      encoding: "utf-8",
      timeout: 60000,
      stdio: ["pipe", "pipe", "pipe"],
    });
  } catch (err) {
    if (err.stdout) return err.stdout;
    return `Error: ${err.stderr || err.message}`;
  }
}

function readSnapshot(cwd) {
  const path = join(cwd, ".houndlens", "snapshot.json");
  if (!existsSync(path)) {
    // Generate snapshot first.
    run("", cwd);
  }
  try {
    return JSON.parse(readFileSync(path, "utf-8"));
  } catch {
    return null;
  }
}

function handleToolCall(name, args) {
  const cwd = args.cwd || process.cwd();

  switch (name) {
    case "houndlens_before_edit": {
      // Rescan to get fresh data.
      run("", cwd);
      const snapshot = readSnapshot(cwd);
      if (!snapshot) return text("No snapshot. Run houndlens first.");

      const file = args.file.replace(/\\/g, "/");

      // Find the file in snapshot.
      const fileInfo = Object.entries(snapshot.files).find(
        ([k]) => k.endsWith(file) || file.endsWith(k)
      );

      if (!fileInfo) return text(`File not found in snapshot: ${file}`);

      const [filePath, info] = fileInfo;
      const result = { file: filePath, functions: [], dependencies: [] };

      // Get function details.
      for (const fn of info.functions) {
        const fnData = {
          name: fn.name,
          line: fn.line,
          params: fn.params,
          return_type: fn.return_type,
          complexity: fn.complexity,
          calls: fn.calls,
          called_by: fn.called_by,
        };

        // Filter to specific function if requested.
        if (args.function) {
          if (!fn.name.toLowerCase().includes(args.function.toLowerCase())) continue;
        }

        result.functions.push(fnData);
      }

      // Get cross-file deps involving this file.
      result.dependencies = snapshot.dependencies.filter(
        (d) => d.from_file === filePath || d.to_file === filePath
      );

      result.warning = result.functions.some((f) => f.called_by.length > 3)
        ? "Some functions have many callers. Changes may have wide impact."
        : null;

      return text(JSON.stringify(result, null, 2));
    }

    case "houndlens_check_work": {
      // Rescan project.
      run("", cwd);

      // Run verify with project tools.
      const diff = args.diff || "HEAD";
      const verifyOutput = run(`--format json verify --diff ${diff}`, cwd);

      try {
        const result = JSON.parse(verifyOutput);

        // Read changes.json for structural changes.
        const changesPath = join(cwd, ".houndlens", "changes.json");
        let changes = null;
        if (existsSync(changesPath)) {
          try { changes = JSON.parse(readFileSync(changesPath, "utf-8")); } catch {}
        }

        const report = {
          status: result.status,
          errors: result.summary?.errors || 0,
          warnings: result.summary?.warnings || 0,
          breaking: result.summary?.breaking || 0,
          risk_score: result.risk_score,
          issues: result.semantic_changes?.map((c) => ({
            file: c.file,
            line: c.line,
            risk: c.risk,
            description: c.description,
          })) || [],
          structural_changes: changes,
          action: result.status === "pass"
            ? "All clear. Safe to continue."
            : "Fix the issues above, then call houndlens_check_work again.",
        };

        return text(JSON.stringify(report, null, 2));
      } catch {
        return text(verifyOutput);
      }
    }

    case "houndlens_plan_change": {
      // Rescan.
      run("", cwd);
      const snapshot = readSnapshot(cwd);
      if (!snapshot) return text("No snapshot.");

      const target = args.target.toLowerCase();

      // Find files and functions related to the target.
      const relatedFiles = [];

      for (const [filePath, info] of Object.entries(snapshot.files)) {
        let relevance = 0;
        const matchedFunctions = [];

        for (const fn of info.functions) {
          const fnLower = fn.name.toLowerCase();
          if (fnLower.includes(target) || target.includes(fnLower.split("::").pop())) {
            relevance += 10;
            matchedFunctions.push({
              name: fn.name,
              line: fn.line,
              called_by: fn.called_by,
            });
          }

          // Check if function calls something matching target.
          for (const call of fn.calls) {
            if (call.toLowerCase().includes(target)) {
              relevance += 5;
              matchedFunctions.push({
                name: fn.name,
                line: fn.line,
                reason: `calls ${call}`,
              });
            }
          }
        }

        if (args.function) {
          for (const fn of info.functions) {
            if (fn.name.toLowerCase().includes(args.function.toLowerCase())) {
              relevance += 20;
              matchedFunctions.push({
                name: fn.name,
                line: fn.line,
                called_by: fn.called_by,
                calls: fn.calls,
              });
            }
          }
        }

        if (relevance > 0) {
          relatedFiles.push({
            file: filePath,
            relevance,
            functions: matchedFunctions,
          });
        }
      }

      // Sort by relevance.
      relatedFiles.sort((a, b) => b.relevance - a.relevance);

      // Build plan.
      const plan = {
        target: args.target,
        files_to_modify: relatedFiles.slice(0, 10).map((f) => ({
          file: f.file,
          functions: f.functions,
        })),
        dependencies: snapshot.dependencies.filter((d) =>
          relatedFiles.some(
            (f) => f.file === d.from_file || f.file === d.to_file
          )
        ),
        order: "Modify the primary file first, then update all callers.",
      };

      return text(JSON.stringify(plan, null, 2));
    }

    case "houndlens_file_context": {
      run("", cwd);
      const snapshot = readSnapshot(cwd);
      if (!snapshot) return text("No snapshot.");

      const file = args.file.replace(/\\/g, "/");
      const fileInfo = Object.entries(snapshot.files).find(
        ([k]) => k.endsWith(file) || file.endsWith(k)
      );

      if (!fileInfo) return text(`File not found: ${file}`);

      const [filePath, info] = fileInfo;

      const context = {
        file: filePath,
        language: info.language,
        functions: info.functions.map((f) => ({
          signature: `${f.is_async ? "async " : ""}${f.name}(${f.params.join(", ")}) → ${f.return_type || "void"}`,
          complexity: f.complexity,
          calls: f.calls,
          called_by: f.called_by,
        })),
        types: info.types.map((t) => ({
          name: t.name,
          kind: t.kind,
          fields: t.fields,
        })),
        imports: info.imports,
        cross_file_deps: snapshot.dependencies.filter(
          (d) => d.from_file === filePath || d.to_file === filePath
        ),
      };

      return text(JSON.stringify(context, null, 2));
    }

    case "houndlens_query": {
      run("", cwd);
      const output = run(`query "${args.query}"`, cwd);
      return text(output);
    }

    default:
      return { content: [{ type: "text", text: `Unknown tool: ${name}` }], isError: true };
  }
}

function text(t) {
  return { content: [{ type: "text", text: t }] };
}

// ─── MCP JSON-RPC Server (stdio) ────────────────────────────────

const rl = createInterface({ input: process.stdin });

rl.on("line", (line) => {
  try {
    const msg = JSON.parse(line);
    const response = handleMessage(msg);
    if (response) {
      process.stdout.write(JSON.stringify(response) + "\n");
    }
  } catch (err) {
    process.stderr.write(`Error: ${err.message}\n`);
  }
});

function handleMessage(msg) {
  const { id, method, params } = msg;

  switch (method) {
    case "initialize":
      return {
        jsonrpc: "2.0",
        id,
        result: {
          protocolVersion: "2024-11-05",
          capabilities: { tools: {} },
          serverInfo: { name: "houndlens", version: "2.2.0" },
        },
      };

    case "notifications/initialized":
      return null;

    case "tools/list":
      return { jsonrpc: "2.0", id, result: { tools: TOOLS } };

    case "tools/call": {
      const { name, arguments: args } = params;
      const result = handleToolCall(name, args || {});
      return { jsonrpc: "2.0", id, result };
    }

    case "ping":
      return { jsonrpc: "2.0", id, result: {} };

    default:
      return {
        jsonrpc: "2.0",
        id,
        error: { code: -32601, message: `Method not found: ${method}` },
      };
  }
}

process.stderr.write("houndlens MCP server started\n");
