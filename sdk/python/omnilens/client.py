"""houndlens Python client — wraps the CLI for use in any Python AI framework."""

import json
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


@dataclass
class SemanticChange:
    file: str
    line: int
    kind: str
    risk: str
    description: str


@dataclass
class InvariantWarning:
    file: str
    line: int
    severity: str
    description: str


@dataclass
class TestSuggestion:
    description: str
    priority: str
    skeleton: Optional[str] = None


@dataclass
class VerifyResult:
    status: str
    risk_score: float
    confidence: float
    total_changes: int
    breaking: int
    needs_review: int
    warnings: int
    semantic_changes: list[SemanticChange] = field(default_factory=list)
    invariant_warnings: list[InvariantWarning] = field(default_factory=list)
    suggested_tests: list[TestSuggestion] = field(default_factory=list)
    raw: dict = field(default_factory=dict)

    @property
    def passed(self) -> bool:
        return self.status == "pass"

    @property
    def has_breaking_changes(self) -> bool:
        return self.breaking > 0


@dataclass
class ImpactNode:
    name: str
    file: str
    line: int
    distance: int


@dataclass
class ImpactResult:
    target: str
    callers: list[ImpactNode] = field(default_factory=list)
    callees: list[ImpactNode] = field(default_factory=list)
    total_affected: int = 0
    raw_text: str = ""


@dataclass
class QueryMatch:
    name: str
    file: str
    line: int
    kind: str
    description: str


@dataclass
class QueryResult:
    query: str
    total_scanned: int
    matches: list[QueryMatch] = field(default_factory=list)


@dataclass
class Invariant:
    kind: str
    description: str
    confidence: float
    evidence_count: int


class Houndlens:
    """Python interface to the houndlens CLI.

    Args:
        cwd: Project root directory. Defaults to current directory.
        binary: Path to houndlens binary. Defaults to "houndlens" (from PATH).
    """

    def __init__(self, cwd: Optional[str] = None, binary: str = "houndlens"):
        self.cwd = cwd or str(Path.cwd())
        self.binary = binary
        self._indexed = False

    def _run(self, args: list[str], json_output: bool = False) -> str:
        cmd = [self.binary]
        if json_output:
            cmd.extend(["--format", "json"])
        cmd.extend(args)

        result = subprocess.run(
            cmd,
            cwd=self.cwd,
            capture_output=True,
            text=True,
            timeout=120,
        )

        # houndlens exits 1 for verification failures — still valid output.
        if result.returncode not in (0, 1):
            raise RuntimeError(
                f"houndlens failed (exit {result.returncode}): {result.stderr}"
            )

        return result.stdout

    def _ensure_indexed(self):
        if not self._indexed:
            self.index()

    def index(self) -> dict:
        """Build or update the semantic index."""
        output = self._run(["index"])
        self._indexed = True
        # Parse text output for stats.
        return {"output": output}

    def verify(
        self,
        diff: Optional[str] = None,
        files: Optional[list[str]] = None,
    ) -> VerifyResult:
        """Verify code changes semantically.

        Args:
            diff: Git ref to compare against (e.g., "HEAD~1", "main").
            files: Specific files to verify.

        Returns:
            VerifyResult with risk score, semantic changes, and test suggestions.
        """
        self._ensure_indexed()

        args = ["verify"]
        if diff:
            args.extend(["--diff", diff])
        if files:
            for f in files:
                args.extend(["--files", f])

        output = self._run(args, json_output=True)

        try:
            data = json.loads(output)
        except json.JSONDecodeError:
            return VerifyResult(
                status="error", risk_score=1.0, confidence=0.0,
                total_changes=0, breaking=0, needs_review=0, warnings=0,
            )

        summary = data.get("summary", {})
        return VerifyResult(
            status=data.get("status", "unknown"),
            risk_score=data.get("risk_score", 0),
            confidence=data.get("confidence", 0),
            total_changes=summary.get("total_changes", 0),
            breaking=summary.get("breaking", 0),
            needs_review=summary.get("needs_review", 0),
            warnings=summary.get("warnings", 0),
            semantic_changes=[
                SemanticChange(**c) for c in data.get("semantic_changes", [])
            ],
            invariant_warnings=[
                InvariantWarning(**w)
                for w in data.get("invariant_warnings", [])
            ],
            suggested_tests=[
                TestSuggestion(**t) for t in data.get("suggested_tests", [])
            ],
            raw=data,
        )

    def impact(
        self, file: str, function: Optional[str] = None, depth: int = 5
    ) -> ImpactResult:
        """Analyze impact of changing a function.

        Args:
            file: Path to the file.
            function: Function name (optional, defaults to first symbol).
            depth: Maximum traversal depth.
        """
        self._ensure_indexed()

        args = ["impact", file]
        if function:
            args.extend(["--fn", function])
        args.extend(["--depth", str(depth)])

        output = self._run(args)
        return ImpactResult(target=function or file, raw_text=output)

    def query(self, query_str: str) -> QueryResult:
        """Run an HoundQL query.

        Args:
            query_str: HoundQL query (e.g., "FIND functions WHERE complexity > 10")
        """
        self._ensure_indexed()

        output = self._run(["query", query_str])

        # Parse text output.
        matches = []
        for line in output.split("\n"):
            line = line.strip()
            if line.startswith("→"):
                parts = line.lstrip("→").strip().split(" — ", 1)
                if len(parts) == 2:
                    loc, rest = parts
                    loc_parts = loc.rsplit(":", 1)
                    file = loc_parts[0].strip()
                    line_num = int(loc_parts[1]) if len(loc_parts) > 1 else 0
                    name_desc = rest.split(" [", 1)
                    name = name_desc[0].strip()
                    desc = name_desc[1].rstrip("]") if len(name_desc) > 1 else ""
                    matches.append(
                        QueryMatch(
                            name=name, file=file, line=line_num,
                            kind="", description=desc,
                        )
                    )

        total_scanned = 0
        for line in output.split("\n"):
            if "scanned" in line:
                try:
                    total_scanned = int(
                        line.split("scanned")[1].strip().rstrip(")")
                    )
                except (ValueError, IndexError):
                    pass

        return QueryResult(
            query=query_str, total_scanned=total_scanned, matches=matches
        )

    def invariants(self) -> list[Invariant]:
        """Discover codebase invariants."""
        self._ensure_indexed()
        output = self._run(["invariants"])
        # Parse text output for invariants.
        invs = []
        for line in output.split("\n"):
            if line.strip().startswith("INV"):
                invs.append(
                    Invariant(
                        kind="discovered",
                        description=line.strip(),
                        confidence=0.0,
                        evidence_count=0,
                    )
                )
        return invs


# ─── LangChain / LlamaIndex tool definitions ────────────────────

def as_langchain_tools(cwd: Optional[str] = None):
    """Create LangChain-compatible tools for houndlens.

    Usage:
        from houndlens import as_langchain_tools
        tools = as_langchain_tools("/path/to/project")
        # Add to your LangChain agent
    """
    try:
        from langchain_core.tools import tool
    except ImportError:
        raise ImportError("Install langchain-core: pip install langchain-core")

    lens = Houndlens(cwd)

    @tool
    def houndlens_verify(diff: str = "HEAD~1") -> str:
        """Verify code changes semantically. Detects breaking changes, signature modifications, and invariant violations. Returns risk score and actionable recommendations."""
        result = lens.verify(diff=diff)
        return json.dumps(result.raw, indent=2)

    @tool
    def houndlens_query(query: str) -> str:
        """Search codebase using HoundQL. Example: 'FIND functions WHERE complexity > 10'. Supports: functions, types, modules. Operators: =, !=, >, <, ~. Predicates: calls(), returns(), implements()."""
        result = lens.query(query)
        return f"Found {len(result.matches)} matches:\n" + "\n".join(
            f"  {m.name} ({m.file}:{m.line}) — {m.description}"
            for m in result.matches[:20]
        )

    @tool
    def houndlens_impact(file: str, function: str = "") -> str:
        """Analyze impact of changing a function. Shows callers and callees."""
        result = lens.impact(file, function=function or None)
        return result.raw_text

    @tool
    def houndlens_invariants() -> str:
        """Discover codebase invariants — patterns always followed in the code."""
        invs = lens.invariants()
        return "\n".join(i.description for i in invs)

    return [houndlens_verify, houndlens_query, houndlens_impact, houndlens_invariants]


def as_openai_tools():
    """Return OpenAI function-calling tool definitions.

    Usage:
        from houndlens import as_openai_tools
        tools = as_openai_tools()
        # Pass to OpenAI chat completions as tools=tools
    """
    return [
        {
            "type": "function",
            "function": {
                "name": "houndlens_verify",
                "description": "Verify code changes semantically. Detects breaking changes, signature modifications, complexity changes, and invariant violations. Returns risk score and recommendations.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "diff": {
                            "type": "string",
                            "description": "Git ref to compare against (e.g., HEAD~1, main)",
                        },
                        "files": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Specific files to verify",
                        },
                    },
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "houndlens_query",
                "description": "Search codebase using HoundQL. Syntax: FIND <target> WHERE <conditions>. Targets: functions, types, modules. Fields: complexity, visibility, params, async, name. Operators: = != > < ~ AND NOT. Predicates: calls(x), returns(x), implements(x).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "HoundQL query string",
                        }
                    },
                    "required": ["query"],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "houndlens_impact",
                "description": "Analyze the blast radius of changing a function. Shows all callers and callees.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file": {"type": "string", "description": "File path"},
                        "function": {
                            "type": "string",
                            "description": "Function name",
                        },
                        "depth": {
                            "type": "integer",
                            "description": "Max depth",
                            "default": 5,
                        },
                    },
                    "required": ["file"],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "houndlens_invariants",
                "description": "Discover codebase invariants: naming conventions, error handling patterns, call ordering rules, type usage constraints.",
                "parameters": {"type": "object", "properties": {}},
            },
        },
    ]
