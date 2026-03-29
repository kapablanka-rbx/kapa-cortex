"""Domain service: build file dependency graph from import/symbol data."""

from __future__ import annotations

from collections import defaultdict
from pathlib import Path

from src.domain.entity.changed_file import ChangedFile
from src.domain.entity.import_ref import ImportRef
from src.domain.entity.symbol_def import SymbolDef
from src.domain.port.definition_resolver import DefinitionResolver


def build_dependency_edges(
    files: list[ChangedFile],
    imports_by_file: dict[str, list[ImportRef]],
    resolver: DefinitionResolver | None = None,
) -> list[tuple[str, str, str, float]]:
    """
    Build (source, target, kind, weight) edges.
    source depends on target: target must land first.
    Uses LSP resolver for precise edges when available,
    falls back to fuzzy module/symbol matching.
    """
    changed_paths = {file.path for file in files}
    module_index = _build_module_index(files)
    symbol_index = _build_symbol_index(files)
    edges: list[tuple[str, str, str, float]] = []

    if resolver:
        edges.extend(_lsp_edges(files, imports_by_file, resolver, changed_paths))
    else:
        edges.extend(_import_edges(files, imports_by_file, module_index))

    edges.extend(_symbol_edges(files, symbol_index))

    return edges


def _build_module_index(files: list[ChangedFile]) -> dict[str, str]:
    """Map module-like keys to file paths."""
    index: dict[str, str] = {}
    for file in files:
        mod = _path_to_module(file.path)
        index[mod] = file.path
        short = mod.rsplit(".", 1)[-1]
        index.setdefault(short, file.path)
        dir_mod = _path_to_module(str(Path(file.path).parent))
        if dir_mod and dir_mod != ".":
            index.setdefault(dir_mod, file.path)
    return index


def _build_symbol_index(
    files: list[ChangedFile],
) -> dict[str, set[str]]:
    """Map symbol names to file paths that define them."""
    index: defaultdict[str, set[str]] = defaultdict(set)
    for file in files:
        for sym in file.symbols_defined:
            index[sym.name].add(file.path)
    return dict(index)


def _lsp_edges(
    files: list[ChangedFile],
    imports_by_file: dict[str, list[ImportRef]],
    resolver: DefinitionResolver,
    changed_paths: set[str],
) -> list[tuple[str, str, str, float]]:
    """Precise edges from LSP go-to-definition."""
    edges = []
    for file in files:
        for imp in imports_by_file.get(file.path, []):
            location = resolver.resolve(file.path, imp.module)
            if location and location.file_path in changed_paths:
                if location.file_path != file.path:
                    edges.append((file.path, location.file_path, "lsp", 1.0))
    return edges


def _import_edges(
    files: list[ChangedFile],
    imports_by_file: dict[str, list[ImportRef]],
    module_index: dict[str, str],
) -> list[tuple[str, str, str, float]]:
    """Edges from import analysis."""
    edges = []
    for file in files:
        for imp in imports_by_file.get(file.path, []):
            norm = _normalize_import(imp.module)
            target = _resolve_target(norm, file.path, module_index)
            if target:
                edges.append((file.path, target, "import", 1.0))
    return edges


def _symbol_edges(
    files: list[ChangedFile],
    symbol_index: dict[str, set[str]],
) -> list[tuple[str, str, str, float]]:
    """Edges from symbol usage analysis."""
    edges = []
    for file in files:
        for ref in file.symbols_used:
            for provider in symbol_index.get(ref.name, set()):
                if provider != file.path:
                    edges.append((file.path, provider, "symbol", 0.8))
    return edges


def _resolve_target(
    norm: str,
    source_path: str,
    module_index: dict[str, str],
) -> str | None:
    """Find the file a normalized import refers to."""
    for key, target in module_index.items():
        if target == source_path:
            continue
        if norm == key or norm.endswith(f".{key}") or key.endswith(f".{norm}"):
            return target
    return None


def _normalize_import(raw: str) -> str:
    return raw.replace("/", ".").replace("::", ".").lstrip(".")


def _path_to_module(path: str) -> str:
    module_path = Path(path).with_suffix("")
    return str(module_path).replace("/", ".").replace("\\", ".")
