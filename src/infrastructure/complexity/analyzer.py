"""Complexity analysis — picks best available tool."""

from __future__ import annotations

from src.domain.file_complexity import FileComplexity
from src.domain.ports.complexity_analyzer import ComplexityAnalyzer
from src.infrastructure.complexity.lizard_analyzer import analyze_lizard
from src.infrastructure.complexity.scc_analyzer import analyze_scc


class LizardSccAnalyzer(ComplexityAnalyzer):
    """Uses lizard (preferred) or scc for complexity metrics."""

    def analyze(self, file_paths: list[str]) -> dict[str, FileComplexity]:
        result = analyze_lizard(file_paths)
        if result:
            return result
        return analyze_scc(file_paths)
