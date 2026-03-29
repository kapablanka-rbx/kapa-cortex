"""Tests for CachedCochangeProvider."""

import json
import os
import tempfile
import unittest

from src.infrastructure.git.cochange_adapter import (
    CachedCochangeProvider,
    _filter_cached,
)


class TestFilterCached(unittest.TestCase):

    def test_filters_to_requested_paths(self):
        cache = {"a.py::b.py": 5, "a.py::c.py": 3, "d.py::e.py": 1}
        result = _filter_cached(cache, ["a.py", "b.py"])
        self.assertEqual(result, {("a.py", "b.py"): 5})

    def test_empty_cache(self):
        result = _filter_cached({}, ["a.py"])
        self.assertEqual(result, {})

    def test_no_matching_paths(self):
        cache = {"a.py::b.py": 5}
        result = _filter_cached(cache, ["x.py", "y.py"])
        self.assertEqual(result, {})

    def test_skips_malformed_keys(self):
        cache = {"a.py::b.py": 5, "bad_key": 2}
        result = _filter_cached(cache, ["a.py", "b.py"])
        self.assertEqual(result, {("a.py", "b.py"): 5})


class TestCachedCochangeProvider(unittest.TestCase):

    def test_loads_from_cache_file(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            cache_dir = os.path.join(tmpdir, ".cortex-cache")
            os.makedirs(cache_dir)
            cache_file = os.path.join(cache_dir, "cochange.json")
            data = {"src/a.py::src/b.py": 10, "src/a.py::src/c.py": 3}
            with open(cache_file, "w") as f:
                json.dump(data, f)

            provider = CachedCochangeProvider(root=tmpdir)
            result = provider.cochange_history(["src/a.py", "src/b.py"])
            self.assertEqual(result, {("src/a.py", "src/b.py"): 10})

    def test_no_cache_does_not_crash(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            provider = CachedCochangeProvider(root=tmpdir)
            # Falls back to git log — in a temp dir with no repo, returns empty
            result = provider.cochange_history(["a.py"])
            self.assertEqual(result, {})


if __name__ == "__main__":
    unittest.main()
