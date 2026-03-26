"""
omnilens — AI-native code verification engine.

Python SDK for calling omnilens from any AI framework
(LangChain, LlamaIndex, CrewAI, AutoGen, OpenAI Assistants, etc.)

Usage:
    from omnilens import Omnilens

    lens = Omnilens("/path/to/project")
    result = lens.verify(diff="HEAD~1")
    print(result.risk_score)
    print(result.semantic_changes)

    # OmniQL query
    matches = lens.query("FIND functions WHERE complexity > 10")

    # Impact analysis
    impact = lens.impact("src/auth.rs", function="verify_token")

    # Invariants
    invariants = lens.invariants()
"""

from .client import Omnilens, VerifyResult, ImpactResult, QueryResult

__version__ = "0.1.0"
__all__ = ["Omnilens", "VerifyResult", "ImpactResult", "QueryResult"]
