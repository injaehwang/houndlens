"""
houndlens — AI-native code verification engine.

Python SDK for calling houndlens from any AI framework
(LangChain, LlamaIndex, CrewAI, AutoGen, OpenAI Assistants, etc.)

Usage:
    from houndlens import Houndlens

    lens = Houndlens("/path/to/project")
    result = lens.verify(diff="HEAD~1")
    print(result.risk_score)
    print(result.semantic_changes)

    # HoundQL query
    matches = lens.query("FIND functions WHERE complexity > 10")

    # Impact analysis
    impact = lens.impact("src/auth.rs", function="verify_token")

    # Invariants
    invariants = lens.invariants()
"""

from .client import Houndlens, VerifyResult, ImpactResult, QueryResult

__version__ = "0.1.0"
__all__ = ["Houndlens", "VerifyResult", "ImpactResult", "QueryResult"]
