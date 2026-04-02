#!/usr/bin/env node

/**
 * houndlens MCP Server
 *
 * Exposes houndlens CLI as MCP tools for AI agents.
 * Tools: verify, impact, query, invariants, index
 *
 * Usage:
 *   npx @houndlens/mcp-server
 *
 * Configure in Claude Desktop / claude_desktop_config.json:
 *   {
 *     "mcpServers": {
 *       "houndlens": {
 *         "command": "npx",
 *         "args": ["@houndlens/mcp-server"]
 *       }
 *     }
 *   }
 */

import { execSync } from "child_process";
import { createInterface } from "readline";

// ─── MCP Protocol ───────────────────────────────────────────────

const TOOLS = [
  {
    name: "houndlens_verify",
    description:
      "Verify code changes semantically. Detects function additions/removals, signature changes, complexity changes, visibility changes, invariant violations, and generates test suggestions. Returns JSON with risk score, semantic changes, and actionable recommendations.",
    inputSchema: {
      type: "object",
      properties: {
        diff: {
          type: "string",
          description:
            'Git diff spec to verify against. Examples: "HEAD~1" (last commit), "main" (compare to main branch), "abc123" (specific commit). Default: working directory changes.',
        },
        files: {
          type: "array",
          items: { type: "string" },
          description:
            "Specific files to verify instead of git diff. Provide file paths relative to project root.",
        },
        cwd: {
          type: "string",
          description:
            "Working directory (project root). Defaults to current directory.",
        },
      },
    },
  },
  {
    name: "houndlens_impact",
    description:
      "Analyze the impact of changing a specific function. Shows who calls this function (reverse impact) and what it calls (forward impact), with depth-limited graph traversal.",
    inputSchema: {
      type: "object",
      properties: {
        file: {
          type: "string",
          description: "Path to the file containing the function.",
        },
        function: {
          type: "string",
          description: "Name of the function to analyze.",
        },
        depth: {
          type: "number",
          description: "Maximum traversal depth (default: 5).",
          default: 5,
        },
        cwd: {
          type: "string",
          description: "Working directory (project root).",
        },
      },
      required: ["file"],
    },
  },
  {
    name: "houndlens_query",
    description:
      'Run an HoundQL query to search the codebase semantically. Query language supports: FIND functions/types/modules WHERE conditions. Example: "FIND functions WHERE complexity > 15", "FIND types WHERE fields > 5", "FIND functions WHERE calls(db.query) AND visibility = public".',
    inputSchema: {
      type: "object",
      properties: {
        query: {
          type: "string",
          description:
            'HoundQL query string. Syntax: FIND <target> WHERE <conditions>. Targets: functions, types, modules, bindings, all. Operators: =, !=, >, <, >=, <=, ~ (glob). Predicates: calls(x), returns(x), implements(x), has_field(x). Logic: AND, NOT.',
        },
        cwd: {
          type: "string",
          description: "Working directory (project root).",
        },
      },
      required: ["query"],
    },
  },
  {
    name: "houndlens_invariants",
    description:
      "Auto-discover codebase invariants — patterns that are always followed in the code. Detects: naming conventions, error handling patterns, call ordering rules, type usage constraints. Useful for understanding codebase rules before making changes.",
    inputSchema: {
      type: "object",
      properties: {
        cwd: {
          type: "string",
          description: "Working directory (project root).",
        },
      },
    },
  },
  {
    name: "houndlens_index",
    description:
      "Build or update the semantic index for a project. Parses all Rust, TypeScript, and Python files, builds a cross-file semantic graph. Must be run before other commands if the project hasn't been indexed yet.",
    inputSchema: {
      type: "object",
      properties: {
        cwd: {
          type: "string",
          description: "Working directory (project root).",
        },
      },
    },
  },
];

function runHoundlens(args, cwd) {
  try {
    const result = execSync(`houndlens ${args}`, {
      cwd: cwd || process.cwd(),
      encoding: "utf-8",
      timeout: 60000,
      stdio: ["pipe", "pipe", "pipe"],
    });
    return result;
  } catch (err) {
    // houndlens may exit with code 1 for verification failures — still valid output.
    if (err.stdout) return err.stdout;
    throw new Error(`houndlens failed: ${err.stderr || err.message}`);
  }
}

function handleToolCall(name, args) {
  const cwd = args.cwd || process.cwd();

  switch (name) {
    case "houndlens_verify": {
      let cmd = "--format json verify";
      if (args.diff) cmd += ` --diff "${args.diff}"`;
      if (args.files) {
        for (const f of args.files) cmd += ` --files "${f}"`;
      }
      const output = runHoundlens(cmd, cwd);
      try {
        const json = JSON.parse(output);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(json, null, 2),
            },
          ],
        };
      } catch {
        return { content: [{ type: "text", text: output }] };
      }
    }

    case "houndlens_impact": {
      let cmd = `impact "${args.file}"`;
      if (args.function) cmd += ` --fn "${args.function}"`;
      if (args.depth) cmd += ` --depth ${args.depth}`;
      const output = runHoundlens(cmd, cwd);
      return { content: [{ type: "text", text: output }] };
    }

    case "houndlens_query": {
      const output = runHoundlens(`query "${args.query}"`, cwd);
      return { content: [{ type: "text", text: output }] };
    }

    case "houndlens_invariants": {
      const output = runHoundlens("invariants", cwd);
      return { content: [{ type: "text", text: output }] };
    }

    case "houndlens_index": {
      const output = runHoundlens("index", cwd);
      return { content: [{ type: "text", text: output }] };
    }

    default:
      return {
        content: [{ type: "text", text: `Unknown tool: ${name}` }],
        isError: true,
      };
  }
}

// ─── MCP JSON-RPC Server (stdio) ────────────────────────────────

const rl = createInterface({ input: process.stdin });
let buffer = "";

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
          serverInfo: {
            name: "houndlens",
            version: "0.1.0",
          },
        },
      };

    case "notifications/initialized":
      return null;

    case "tools/list":
      return {
        jsonrpc: "2.0",
        id,
        result: { tools: TOOLS },
      };

    case "tools/call": {
      const { name, arguments: args } = params;
      const result = handleToolCall(name, args || {});
      return {
        jsonrpc: "2.0",
        id,
        result,
      };
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
