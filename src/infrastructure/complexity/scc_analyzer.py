"""Run scc (Sloc Cloc and Code) for file-level metrics."""

from __future__ import annotations

import json
import subprocess

from src.domain.file_complexity import FileComplexity


def analyze_scc(file_paths: list[str]) -> dict[str, FileComplexity]:
    """Run scc on files, return per-file metrics."""
    if not file_paths:
        return {}
    try:
        result = subprocess.run(
            ["scc", "--format", "json", "--by-file", "--no-cocomo", *file_paths],
            capture_output=True, text=True, timeout=30,
        )
        if result.returncode != 0:
            return {}
    except (subprocess.TimeoutExpired, FileNotFoundError):
        return {}

    return _parse_scc_output(result.stdout)


def _parse_scc_output(raw: str) -> dict[str, FileComplexity]:
    metrics: dict[str, FileComplexity] = {}
    try:
        data = json.loads(raw)
        for group in data:
            lang = group.get("Name", "")
            for entry in group.get("Files", []):
                path = entry.get("Location", "")
                metrics[path] = FileComplexity(
                    language=lang,
                    lines=entry.get("Lines", 0),
                    code=entry.get("Code", 0),
                    comments=entry.get("Comments", 0),
                    blanks=entry.get("Blank", 0),
                    complexity=entry.get("Complexity", 0),
                )
    except (json.JSONDecodeError, KeyError):
        pass
    return metrics
