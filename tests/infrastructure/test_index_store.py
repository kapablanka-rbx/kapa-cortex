"""Tests for IndexStore."""

import tempfile
import unittest

from src.infrastructure.indexer.index_store import (
    IndexStore, FileEntry, SymbolEntry, ImportEntry, EdgeEntry, CallEntry,
)


class TestIndexStore(unittest.TestCase):

    def _build_store(self):
        store = IndexStore()
        store.add_file(FileEntry("src/a.py", "python", "abc123", 100, 5))
        store.add_file(FileEntry("src/b.py", "python", "def456", 50, 2))
        store.add_symbols("src/a.py", [
            SymbolEntry("Foo", "class", 10, "", "src/a.py"),
            SymbolEntry("bar", "function", 20, "", "src/a.py"),
        ])
        store.add_imports("src/b.py", [
            ImportEntry("from src.a import Foo", "src.a", "module", "src/b.py"),
        ])
        store.add_edge(EdgeEntry("src/b.py", "src/a.py", "import", 1.0))
        return store

    def test_file_count(self):
        store = self._build_store()
        self.assertEqual(store.file_count, 2)

    def test_symbol_count(self):
        store = self._build_store()
        self.assertEqual(store.symbol_count, 2)

    def test_get_symbols_for_file(self):
        store = self._build_store()
        symbols = store.get_symbols_for_file("src/a.py")
        self.assertEqual(len(symbols), 2)
        self.assertEqual(symbols[0].name, "Foo")

    def test_get_files_defining_symbol(self):
        store = self._build_store()
        files = store.get_files_defining_symbol("Foo")
        self.assertEqual(files, ["src/a.py"])

    def test_get_dependents(self):
        store = self._build_store()
        dependents = store.get_dependents("src/a.py")
        self.assertEqual(dependents, ["src/b.py"])

    def test_get_dependencies(self):
        store = self._build_store()
        deps = store.get_dependencies("src/b.py")
        self.assertEqual(deps, ["src/a.py"])

    def test_remove_file(self):
        store = self._build_store()
        store.remove_file("src/a.py")
        self.assertEqual(store.file_count, 1)
        self.assertEqual(store.get_files_defining_symbol("Foo"), [])
        self.assertEqual(store.edge_count, 0)

    def test_save_and_load(self):
        store = self._build_store()
        with tempfile.NamedTemporaryFile(suffix=".msgpack", delete=False) as tmp_file:
            path = tmp_file.name

        store.save(path)
        loaded = IndexStore.load(path)

        self.assertEqual(loaded.file_count, 2)
        self.assertEqual(loaded.symbol_count, 2)
        self.assertEqual(loaded.edge_count, 1)
        self.assertEqual(loaded.get_dependents("src/a.py"), ["src/b.py"])

    def test_empty_store(self):
        store = IndexStore()
        self.assertEqual(store.file_count, 0)
        self.assertEqual(store.symbol_count, 0)
        self.assertEqual(store.edge_count, 0)
        self.assertEqual(store.get_dependents("nope"), [])


class TestStrongNameCallers(unittest.TestCase):
    """Call graph queries using (function, file) strong names."""

    def _build_call_store(self):
        store = IndexStore()
        store.add_file(FileEntry("a.py", "python", "h1", 10))
        store.add_file(FileEntry("b.py", "python", "h2", 20))
        store.add_file(FileEntry("c.py", "python", "h3", 30))
        # Same function name 'foo' defined in both a.py and b.py
        store.add_symbols("a.py", [
            SymbolEntry("foo", "function", 1, "", "a.py"),
        ])
        store.add_symbols("b.py", [
            SymbolEntry("foo", "function", 1, "", "b.py"),
            SymbolEntry("bar", "function", 5, "", "b.py"),
        ])
        # bar() in b.py calls foo() in a.py
        store.add_call(CallEntry(
            caller_file="b.py", caller_function="bar",
            callee_file="a.py", callee_function="foo", line=6,
        ))
        # baz() in c.py calls foo() in b.py (different foo!)
        store.add_call(CallEntry(
            caller_file="c.py", caller_function="baz",
            callee_file="b.py", callee_function="foo", line=3,
        ))
        return store

    def test_strong_name_distinguishes_same_name(self):
        store = self._build_call_store()
        calls_a = store.get_callers("foo", "a.py")
        calls_b = store.get_callers("foo", "b.py")
        self.assertEqual(len(calls_a), 1)
        self.assertEqual(calls_a[0].caller_function, "bar")
        self.assertEqual(len(calls_b), 1)
        self.assertEqual(calls_b[0].caller_function, "baz")

    def test_by_name_returns_all(self):
        store = self._build_call_store()
        all_calls = store.get_callers_by_name("foo")
        self.assertEqual(len(all_calls), 2)

    def test_nonexistent_strong_name(self):
        store = self._build_call_store()
        self.assertEqual(store.get_callers("foo", "c.py"), [])

    def test_nonexistent_name(self):
        store = self._build_call_store()
        self.assertEqual(store.get_callers_by_name("nonexistent"), [])

    def test_call_count(self):
        store = self._build_call_store()
        self.assertEqual(store.call_count, 2)

    def test_save_load_preserves_strong_names(self):
        store = self._build_call_store()
        with tempfile.NamedTemporaryFile(suffix=".msgpack", delete=False) as tmp:
            path = tmp.name
        store.save(path)
        loaded = IndexStore.load(path)
        calls_a = loaded.get_callers("foo", "a.py")
        calls_b = loaded.get_callers("foo", "b.py")
        self.assertEqual(len(calls_a), 1)
        self.assertEqual(calls_a[0].caller_function, "bar")
        self.assertEqual(len(calls_b), 1)
        self.assertEqual(calls_b[0].caller_function, "baz")

    def test_remove_file_clears_call_indexes(self):
        store = self._build_call_store()
        store.remove_file("b.py")
        self.assertEqual(store.get_callers("foo", "a.py"), [])
        self.assertEqual(store.get_callers("foo", "b.py"), [])


if __name__ == "__main__":
    unittest.main()
