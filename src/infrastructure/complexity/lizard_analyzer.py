"""Run lizard for function-level cyclomatic complexity."""

from __future__ import annotations

import os

from src.domain.file_complexity import FileComplexity, FunctionComplexity


def analyze_lizard(file_paths: list[str]) -> dict[str, FileComplexity]:
    """Run lizard on files. Returns empty dict if lizard not installed."""
    try:
        import lizard
    except ImportError:
        return {}

    metrics: dict[str, FileComplexity] = {}
    for path in file_paths:
        if not os.path.exists(path):
            continue
        result = _analyze_single(lizard, path)
        if result:
            metrics[path] = result
    return metrics


def _analyze_single(lizard, path: str) -> FileComplexity | None:
    try:
        analysis = lizard.analyze_file(path)
    except Exception:
        return None

    functions = _extract_functions(analysis)
    total_cc = sum(f.cyclomatic for f in functions)
    avg_cc = total_cc / len(functions) if functions else 0
    max_cc = max((f.cyclomatic for f in functions), default=0)

    ext = path.rsplit(".", 1)[-1] if "." in path else ""
    return FileComplexity(
        language=ext, lines=analysis.nloc, code=analysis.nloc,
        comments=0, blanks=0, complexity=total_cc,
        functions=functions,
        avg_cyclomatic=round(avg_cc, 1),
        max_cyclomatic=max_cc,
    )


def _extract_functions(analysis) -> list[FunctionComplexity]:
    return [
        FunctionComplexity(
            name=func.name,
            start_line=func.start_line,
            end_line=func.end_line,
            cyclomatic=func.cyclomatic_complexity,
            token_count=func.token_count,
            parameter_count=len(func.parameters),
            length=func.nloc,
        )
        for func in analysis.function_list
    ]
