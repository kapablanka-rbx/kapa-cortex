"""Interface: JSON output reporter."""

from __future__ import annotations

import json


def build_json(prs, branch, base, graph) -> dict:
    """Build JSON-serializable dict from analysis results."""
    return {
        "branch": branch,
        "base": base,
        "total_prs": len(prs),
        "file_dependency_edges": graph.number_of_edges(),
        "prs": [_pr_to_dict(pr) for pr in prs],
    }


def print_json(prs, branch, base, graph):
    data = build_json(prs, branch, base, graph)
    print(json.dumps(data, indent=2))


def _pr_to_dict(pr):
    return {
        "index": pr.index,
        "title": pr.title,
        "files": [
            {
                "path": file.path, "status": file.status,
                "added": file.added, "removed": file.removed,
                "is_docs": file.is_text_or_docs,
                "complexity": file.cyclomatic_complexity,
            }
            for file in pr.files
        ],
        "code_lines": pr.total_code_lines,
        "total_lines": pr.total_all_lines,
        "complexity": pr.total_complexity,
        "depends_on": pr.depends_on,
        "merge_strategy": pr.merge_strategy,
        "risk_score": pr.risk_score,
    }
