"""Tests for graph_builder — call resolution, module index, fuzzy matching."""

import unittest

from src.infrastructure.indexer.graph_builder import (
    _pick_callee_file,
    _import_matches_path,
    _build_module_index,
    _resolve_import,
)
from src.infrastructure.indexer.index_store import (
    IndexStore, FileEntry, SymbolEntry, ImportEntry, EdgeEntry,
)


class TestPickCalleeFile(unittest.TestCase):
    def test_prefers_import_edge(self):
        result = _pick_callee_file(
            caller_file="a.rs",
            candidate_files=["b.rs", "c.rs"],
            caller_deps={"c.rs"},
            caller_imports=set(),
        )
        self.assertEqual(result, "c.rs")

    def test_unambiguous_single_candidate(self):
        result = _pick_callee_file(
            caller_file="a.rs",
            candidate_files=["b.rs"],
            caller_deps=set(),
            caller_imports=set(),
        )
        self.assertEqual(result, "b.rs")

    def test_skips_same_file(self):
        result = _pick_callee_file(
            caller_file="a.rs",
            candidate_files=["a.rs"],
            caller_deps=set(),
            caller_imports=set(),
        )
        self.assertIsNone(result)

    def test_ambiguous_no_signal_returns_none(self):
        result = _pick_callee_file(
            caller_file="a.rs",
            candidate_files=["b.rs", "c.rs"],
            caller_deps=set(),
            caller_imports=set(),
        )
        self.assertIsNone(result)

    def test_fuzzy_import_match(self):
        result = _pick_callee_file(
            caller_file="app/caller.rs",
            candidate_files=["app/buck2_error/src/context.rs", "other/context.rs"],
            caller_deps=set(),
            caller_imports={"buck2_error.context"},
        )
        self.assertEqual(result, "app/buck2_error/src/context.rs")

    def test_import_edge_beats_fuzzy(self):
        result = _pick_callee_file(
            caller_file="a.rs",
            candidate_files=["b.rs", "c.rs"],
            caller_deps={"b.rs"},
            caller_imports={"some.module.c"},
        )
        self.assertEqual(result, "b.rs")


class TestImportMatchesPath(unittest.TestCase):
    def test_rust_crate_module_match(self):
        self.assertTrue(_import_matches_path(
            {"buck2_error.context"},
            "app/buck2_error/src/context.rs",
        ))

    def test_no_match(self):
        self.assertFalse(_import_matches_path(
            {"totally.unrelated"},
            "app/buck2_error/src/context.rs",
        ))

    def test_single_segment_import_no_match(self):
        self.assertFalse(_import_matches_path(
            {"context"},
            "app/buck2_error/src/context.rs",
        ))

    def test_python_module_match(self):
        self.assertTrue(_import_matches_path(
            {"mypackage.utils"},
            "src/mypackage/utils.py",
        ))


class TestModuleIndex(unittest.TestCase):
    def setUp(self):
        self.store = IndexStore()
        self.store.add_file(FileEntry(
            path="src/domain/service/graph.py",
            language="python", file_hash="abc", lines=100,
        ))
        self.store.add_file(FileEntry(
            path="src/infrastructure/git/client.py",
            language="python", file_hash="def", lines=50,
        ))
        self.index = _build_module_index(self.store)

    def test_exact_match(self):
        result = _resolve_import(
            "src.domain.service.graph",
            "other.py",
            self.index,
        )
        self.assertEqual(result, "src/domain/service/graph.py")

    def test_suffix_match(self):
        result = _resolve_import(
            "service.graph",
            "other.py",
            self.index,
        )
        self.assertEqual(result, "src/domain/service/graph.py")

    def test_no_match_returns_none(self):
        result = _resolve_import(
            "nonexistent.module",
            "other.py",
            self.index,
        )
        self.assertIsNone(result)

    def test_skips_self(self):
        result = _resolve_import(
            "src.domain.service.graph",
            "src/domain/service/graph.py",
            self.index,
        )
        self.assertIsNone(result)


if __name__ == "__main__":
    unittest.main()
