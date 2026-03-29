"""Tests for graph query services."""

import unittest

from src.domain.service.graph_queries import (
    find_impact, find_deps, find_hotspots,
    find_call_impact, find_symbol_impact,
    CallChain,
)
from src.infrastructure.indexer.index_store import CallEntry


class TestFindImpact(unittest.TestCase):

    def _build_graph(self):
        """
        a.py → b.py → c.py → d.py
                  ↘ e.py
        """
        reverse_edges = {
            "a.py": ["b.py"],
            "b.py": ["c.py"],
            "c.py": ["d.py"],
            "e.py": ["b.py"],
        }
        return lambda path: reverse_edges.get(path, [])

    def test_direct_dependents(self):
        result = find_impact("a.py", self._build_graph())
        self.assertEqual(result.direct, ["b.py"])

    def test_transitive_dependents(self):
        result = find_impact("a.py", self._build_graph())
        self.assertIn("c.py", result.transitive)
        self.assertIn("d.py", result.transitive)

    def test_total_affected(self):
        result = find_impact("a.py", self._build_graph())
        self.assertGreaterEqual(result.total_affected, 3)

    def test_no_dependents(self):
        result = find_impact("d.py", self._build_graph())
        self.assertEqual(result.direct, [])
        self.assertEqual(result.transitive, [])


class TestFindDeps(unittest.TestCase):

    def test_transitive_dependencies(self):
        forward_edges = {
            "d.py": ["c.py"],
            "c.py": ["b.py"],
            "b.py": ["a.py"],
        }
        get_deps = lambda path: forward_edges.get(path, [])

        result = find_deps("d.py", get_deps)
        self.assertIn("c.py", result)
        self.assertIn("b.py", result)
        self.assertIn("a.py", result)

    def test_no_dependencies(self):
        result = find_deps("leaf.py", lambda path: [])
        self.assertEqual(result, [])


class TestFindHotspots(unittest.TestCase):

    def test_ranks_by_complexity_times_dependents(self):
        files = ["a.py", "b.py", "c.py"]
        complexities = {"a.py": 20, "b.py": 5, "c.py": 10}
        dependents = {"a.py": ["x.py"], "b.py": ["x.py", "y.py", "z.py"], "c.py": []}

        result = find_hotspots(
            files,
            get_complexity=lambda path: complexities.get(path, 0),
            get_dependents=lambda path: dependents.get(path, []),
            limit=10,
        )

        self.assertEqual(len(result), 3)
        # a.py: 20 * (1+1) = 40, b.py: 5 * (1+3) = 20, c.py: 10 * (1+0) = 10
        self.assertEqual(result[0].path, "a.py")
        self.assertEqual(result[1].path, "b.py")
        self.assertEqual(result[2].path, "c.py")

    def test_limit(self):
        files = [f"file{index}.py" for index in range(50)]
        result = find_hotspots(
            files,
            get_complexity=lambda path: 5,
            get_dependents=lambda path: ["other.py"],
            limit=10,
        )
        self.assertEqual(len(result), 10)

    def test_skips_zero_complexity_zero_dependents(self):
        result = find_hotspots(
            ["empty.py"],
            get_complexity=lambda path: 0,
            get_dependents=lambda path: [],
        )
        self.assertEqual(result, [])


class TestFindCallImpact(unittest.TestCase):
    """Call impact with strong names — (function, file) pairs."""

    def _build_call_graph(self):
        """
        foo@a.py ← bar@b.py ← baz@c.py
        foo@x.py ← qux@y.py  (different foo, should not mix)
        """
        calls = {
            ("foo", "a.py"): [
                CallEntry("b.py", "bar", "a.py", "foo", 10),
            ],
            ("bar", "b.py"): [
                CallEntry("c.py", "baz", "b.py", "bar", 5),
            ],
            ("foo", "x.py"): [
                CallEntry("y.py", "qux", "x.py", "foo", 3),
            ],
        }
        return lambda name, file: calls.get((name, file), [])

    def test_direct_callers(self):
        result = find_call_impact("foo", "a.py", self._build_call_graph())
        self.assertEqual(len(result.direct_callers), 1)
        self.assertEqual(result.direct_callers[0].caller_function, "bar")
        self.assertEqual(result.direct_callers[0].caller_file, "b.py")

    def test_transitive_callers(self):
        result = find_call_impact("foo", "a.py", self._build_call_graph())
        self.assertEqual(len(result.transitive_callers), 1)
        self.assertEqual(result.transitive_callers[0].caller_function, "baz")

    def test_does_not_mix_duplicate_names(self):
        result = find_call_impact("foo", "a.py", self._build_call_graph())
        all_callers = result.direct_callers + result.transitive_callers
        caller_files = {chain.caller_file for chain in all_callers}
        self.assertNotIn("y.py", caller_files)

    def test_other_foo_has_own_callers(self):
        result = find_call_impact("foo", "x.py", self._build_call_graph())
        self.assertEqual(len(result.direct_callers), 1)
        self.assertEqual(result.direct_callers[0].caller_file, "y.py")

    def test_no_callers(self):
        get_callers = lambda name, file: []
        result = find_call_impact("lonely", "z.py", get_callers)
        self.assertEqual(result.total_call_chains, 0)


class TestFindSymbolImpact(unittest.TestCase):
    """Pure call-graph blast radius."""

    def test_returns_call_chains(self):
        calls = {
            ("foo", "a.py"): [
                CallEntry("b.py", "bar", "a.py", "foo", 10),
            ],
            ("bar", "b.py"): [
                CallEntry("c.py", "baz", "b.py", "bar", 5),
            ],
        }
        get_callers = lambda name, file: calls.get((name, file), [])

        result = find_symbol_impact("foo", "a.py", get_callers)
        self.assertEqual(len(result.call_chains), 2)
        self.assertEqual(result.affected_files, ["b.py", "c.py"])

    def test_intra_file_calls(self):
        calls = {
            ("foo", "a.py"): [
                CallEntry("a.py", "bar", "a.py", "foo", 10),
            ],
            ("bar", "a.py"): [
                CallEntry("a.py", "baz", "a.py", "bar", 5),
            ],
        }
        get_callers = lambda name, file: calls.get((name, file), [])

        result = find_symbol_impact("foo", "a.py", get_callers)
        self.assertEqual(len(result.call_chains), 2)
        self.assertEqual(result.affected_files, ["a.py"])

    def test_no_callers_no_impact(self):
        result = find_symbol_impact(
            "lonely", "z.py",
            lambda name, file: [],
        )
        self.assertEqual(result.total_affected, 0)


if __name__ == "__main__":
    unittest.main()
