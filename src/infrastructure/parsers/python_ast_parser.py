"""Parse Python imports using stdlib ast module."""

from __future__ import annotations

import ast
import re

from src.domain.import_ref import ImportRef


def parse_python_imports(source: str) -> list[ImportRef]:
    """Parse Python imports via AST, fallback to regex."""
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return _parse_regex(source)
    return _parse_ast(tree)


def _parse_ast(tree: ast.Module) -> list[ImportRef]:
    results: list[ImportRef] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                results.append(ImportRef(alias.name, alias.name, "module"))
        elif isinstance(node, ast.ImportFrom):
            mod = node.module or ""
            prefix = "." * (node.level or 0)
            results.append(ImportRef(f"{prefix}{mod}", mod, "module"))
    return results


_RE = re.compile(
    r"^\s*(?:from\s+([\w.]+)\s+import|import\s+([\w.]+))", re.MULTILINE
)


def _parse_regex(source: str) -> list[ImportRef]:
    return [
        ImportRef(m.group(1) or m.group(2), m.group(1) or m.group(2), "module")
        for m in _RE.finditer(source)
    ]
