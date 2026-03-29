"""Detect programming language from file path."""

from __future__ import annotations

from pathlib import Path

_EXT_TO_LANG: dict[str, str] = {
    ".py": "python", ".pyi": "python",
    ".c": "c", ".h": "c",
    ".cc": "cpp", ".cpp": "cpp", ".cxx": "cpp", ".hpp": "cpp", ".hxx": "cpp",
    ".java": "java",
    ".kt": "kotlin", ".kts": "kotlin",
    ".go": "go",
    ".rs": "rust",
    ".js": "javascript", ".jsx": "javascript", ".mjs": "javascript", ".cjs": "javascript",
    ".ts": "typescript", ".tsx": "typescript",
    ".cmake": "cmake",
    ".bzl": "starlark", ".star": "starlark",
    ".bxl": "bxl",
    ".groovy": "groovy",
    ".gradle": "gradle_groovy",
}

_FILENAME_TO_LANG: dict[str, str | None] = {
    "CMakeLists.txt": "cmake",
    "BUCK": "buck2", "TARGETS": "buck2",
    "BUILD": "starlark", "BUILD.bazel": "starlark",
    "WORKSPACE": "starlark", "WORKSPACE.bazel": "starlark",
    "build.gradle": "gradle_groovy",
    "settings.gradle": "gradle_groovy",
    "build.gradle.kts": "gradle_kts",
    "settings.gradle.kts": "gradle_kts",
    "gradle.properties": None,
}


def detect_language(file_path: str) -> str | None:
    """Detect language from file path. Returns None if unknown."""
    p = Path(file_path)

    lang = _FILENAME_TO_LANG.get(p.name)
    if lang is not None:
        return lang if lang else None

    suffixes = "".join(p.suffixes).lower()
    if suffixes.endswith(".gradle.kts"):
        return "gradle_kts"

    return _EXT_TO_LANG.get(p.suffix.lower())
