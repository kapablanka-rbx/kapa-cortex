"""Infrastructure: multi-language import and symbol parser.

Implements ImportParser and SymbolExtractor ports.
Delegates to the import_dispatcher which handles language detection
and strategy selection (tree-sitter -> ast-grep -> ast -> regex).
"""

from __future__ import annotations

from src.domain.import_ref import ImportRef
from src.domain.symbol_def import SymbolDef
from src.domain.ports.import_parser import ImportParser
from src.domain.ports.symbol_extractor import SymbolExtractor
from src.infrastructure.parsers.import_dispatcher import dispatch_parse_imports


class MultiLangImportParser(ImportParser):
    """Parses imports across 15+ languages."""

    def parse(self, file_path: str, source: str) -> list[ImportRef]:
        return dispatch_parse_imports(file_path, source)


class MultiLangSymbolExtractor(SymbolExtractor):
    """Extracts symbol definitions. Currently a no-op placeholder."""

    def extract(self, file_path: str, source: str) -> list[SymbolDef]:
        # TODO: add ctags/tree-sitter symbol extraction
        return []
