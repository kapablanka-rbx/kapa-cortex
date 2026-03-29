"""Complexity analyzer with cache-first strategy."""

from __future__ import annotations

from src.domain.value_object.file_complexity import FileComplexity
from src.domain.port.complexity_analyzer import ComplexityAnalyzer
from src.infrastructure.complexity.analyzer import LizardAnalyzer
from src.infrastructure.indexer.complexity_cache import load_complexity_cache


class CachedComplexityAnalyzer(ComplexityAnalyzer):
    """Checks .cortex-cache/complexity.json first, falls back to live analysis."""

    def __init__(self, root: str = "."):
        self._root = root
        self._fallback = LizardAnalyzer()

    def analyze(self, file_paths: list[str]) -> dict[str, FileComplexity]:
        cached = load_complexity_cache(self._root)
        if cached is None:
            return self._fallback.analyze(file_paths)
        return _resolve(cached, file_paths, self._fallback)


def _resolve(
    cached: dict[str, dict],
    file_paths: list[str],
    fallback: LizardAnalyzer,
) -> dict[str, FileComplexity]:
    """Return cached metrics where available, live-analyze the rest."""
    result: dict[str, FileComplexity] = {}
    missing: list[str] = []

    for path in file_paths:
        entry = cached.get(path)
        if entry:
            result[path] = FileComplexity(
                language=entry.get("language", ""),
                lines=entry.get("lines", 0),
                code=entry.get("code", 0),
                comments=entry.get("comments", 0),
                blanks=entry.get("blanks", 0),
                complexity=entry.get("complexity", 0),
                avg_cyclomatic=entry.get("avg_cyclomatic", 0.0),
                max_cyclomatic=entry.get("max_cyclomatic", 0),
            )
        else:
            missing.append(path)

    if missing:
        result.update(fallback.analyze(missing))

    return result
