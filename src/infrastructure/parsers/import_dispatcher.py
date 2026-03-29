"""Dispatch import parsing to the right strategy per language.

Chain: tree-sitter -> ast-grep -> Python AST / regex fallback.
"""

from __future__ import annotations

from src.domain.import_ref import ImportRef
from src.infrastructure.parsers.language_detector import detect_language
from src.infrastructure.parsers.python_ast_parser import parse_python_imports
from src.infrastructure.parsers import regex_parsers as rp

_REGEX_DISPATCH: dict[str, callable] = {
    "python": parse_python_imports,
    "c": rp.parse_cpp,
    "cpp": rp.parse_cpp,
    "java": rp.parse_java,
    "kotlin": rp.parse_kotlin,
    "go": rp.parse_go,
    "rust": rp.parse_rust,
    "javascript": rp.parse_js_ts,
    "typescript": rp.parse_js_ts,
    "cmake": rp.parse_cmake,
    "buck2": rp.parse_buck2,
    "starlark": rp.parse_starlark,
    "bxl": rp.parse_bxl,
    "gradle_groovy": rp.parse_gradle_groovy,
    "gradle_kts": rp.parse_gradle_kts,
    "groovy": rp.parse_groovy,
}


def dispatch_parse_imports(file_path: str, source: str) -> list[ImportRef]:
    """Parse imports using the best available strategy for the language."""
    lang = detect_language(file_path)
    if not lang:
        return []

    # TODO: add tree-sitter and ast-grep layers here when available
    # For now, go straight to regex/AST fallback

    parser = _REGEX_DISPATCH.get(lang)
    if parser:
        return parser(source)

    return []
