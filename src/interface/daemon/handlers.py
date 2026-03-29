"""Daemon query handlers — wire use cases to daemon actions."""

from __future__ import annotations

from src.infrastructure.git.git_client import GitClient
from src.infrastructure.parsers.multi_lang_parser import (
    MultiLangImportParser,
    MultiLangSymbolExtractor,
)
from src.infrastructure.complexity.cached_analyzer import CachedComplexityAnalyzer
from src.infrastructure.llm.ollama_backend import NullLLMService
from src.infrastructure.git.cochange_adapter import CachedCochangeProvider
from src.infrastructure.diff.difftastic_classifier import DifftasticClassifier
from src.application.analyze_branch import AnalyzeBranchUseCase
from src.interface.reporters.json_reporter import build_json


def handle_analyze(params: dict) -> dict:
    """Run branch analysis and return JSON-serializable result."""
    git = GitClient()
    base = params.get("base") or git.detect_base()
    max_files = params.get("max_files", 3)
    max_lines = params.get("max_lines", 200)

    analyze_use_case = AnalyzeBranchUseCase(
        git=git,
        parser=MultiLangImportParser(),
        symbols=MultiLangSymbolExtractor(),
        complexity=CachedComplexityAnalyzer(),
        llm=NullLLMService(),
        cochange=CachedCochangeProvider(),
        diff_classifier=DifftasticClassifier(),
    )

    result = analyze_use_case.execute(base, max_files, max_lines)
    if not result.files:
        return {"branch": result.branch, "base": base, "total_prs": 0, "prs": []}

    return build_json(result.prs, result.branch, base, result.graph)


def handle_status(params: dict) -> dict:
    """Return daemon status."""
    return {"running": True, "lsp_servers": []}


def build_handler_map() -> dict:
    """Build action → handler mapping for the query router."""
    return {
        "analyze": handle_analyze,
        "status": handle_status,
    }
