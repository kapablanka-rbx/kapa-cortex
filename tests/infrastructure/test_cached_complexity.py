"""Tests for CachedComplexityAnalyzer."""

import json
import os
import tempfile
import unittest

from src.infrastructure.complexity.cached_analyzer import CachedComplexityAnalyzer


class TestCachedComplexityAnalyzer(unittest.TestCase):

    def test_loads_from_cache(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            cache_dir = os.path.join(tmpdir, ".cortex-cache")
            os.makedirs(cache_dir)
            data = {
                "src/foo.py": {
                    "hash": "abc123",
                    "language": "Python",
                    "lines": 100,
                    "code": 80,
                    "comments": 10,
                    "blanks": 10,
                    "complexity": 5,
                    "avg_cyclomatic": 2.5,
                    "max_cyclomatic": 4,
                },
            }
            with open(os.path.join(cache_dir, "complexity.json"), "w") as f:
                json.dump(data, f)

            analyzer = CachedComplexityAnalyzer(root=tmpdir)
            result = analyzer.analyze(["src/foo.py"])

            self.assertIn("src/foo.py", result)
            self.assertEqual(result["src/foo.py"].complexity, 5)
            self.assertEqual(result["src/foo.py"].language, "Python")
            self.assertEqual(result["src/foo.py"].lines, 100)

    def test_no_cache_does_not_crash(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            analyzer = CachedComplexityAnalyzer(root=tmpdir)
            result = analyzer.analyze([])
            self.assertEqual(result, {})

    def test_missing_file_falls_back(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            cache_dir = os.path.join(tmpdir, ".cortex-cache")
            os.makedirs(cache_dir)
            with open(os.path.join(cache_dir, "complexity.json"), "w") as f:
                json.dump({}, f)

            analyzer = CachedComplexityAnalyzer(root=tmpdir)
            # nonexistent file — fallback analyzer returns empty for it
            result = analyzer.analyze(["nonexistent.py"])
            self.assertNotIn("nonexistent.py", result)


if __name__ == "__main__":
    unittest.main()
