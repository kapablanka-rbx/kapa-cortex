"""Interface: terminal text reporter with ANSI colors."""

from __future__ import annotations

from src.domain.entity.proposed_pr import ProposedPR
from src.domain.service.merge_order_resolver import compute_merge_order, compute_waves

RESET = "\033[0m"
BOLD = "\033[1m"
GREEN = "\033[32m"
YELLOW = "\033[33m"
RED = "\033[31m"
CYAN = "\033[36m"
DIM = "\033[2m"


def print_analysis(prs, branch, base, total_files, graph):
    _print_header(branch, base, total_files, len(prs))
    for proposed_pr in prs:
        _print_pr(proposed_pr)
    _print_merge_order(prs)
    _print_waves(prs)
    print()


def _print_header(branch, base, total_files, pr_count):
    print(f"\n{BOLD}{'=' * 70}{RESET}")
    print(f"{BOLD}  kapa-cortex analysis{RESET}")
    print(f"{BOLD}{'=' * 70}{RESET}")
    print(f"  Branch : {CYAN}{branch}{RESET} → {CYAN}{base}{RESET} ({total_files} files changed)")
    print(f"  PRs    : {GREEN}{pr_count}{RESET}")
    print(f"{BOLD}{'=' * 70}{RESET}")


def _print_pr(proposed_pr):
    risk_label = _risk_label(proposed_pr.risk_score)
    complexity_label = _complexity_label(proposed_pr.total_complexity)

    print(f"\n  {BOLD}{proposed_pr.title}{RESET}")
    print(f"  {DIM}{'─' * 60}{RESET}")

    if proposed_pr.depends_on:
        deps = ", ".join(f"PR #{dep}" for dep in proposed_pr.depends_on)
        print(f"    after {YELLOW}{deps}{RESET}")

    warnings = _build_warnings(proposed_pr, risk_label, complexity_label)
    for warning in warnings:
        print(f"    {warning}")

    print(f"    {proposed_pr.total_code_lines} lines, {len(proposed_pr.files)} files")
    for file in proposed_pr.files:
        status = _status_label(file.status)
        doc = f" {DIM}(docs){RESET}" if file.is_text_or_docs else ""
        print(f"      {file.path}  +{file.added}/-{file.removed}  {status}{doc}")


def _print_merge_order(prs):
    print(f"\n  {BOLD}Order:{RESET}")
    print(f"  {DIM}{'─' * 60}{RESET}")
    for index, proposed_pr in enumerate(compute_merge_order(prs), 1):
        deps = f"  (after {', '.join(f'#{dep}' for dep in proposed_pr.depends_on)})" if proposed_pr.depends_on else ""
        print(f"  {index}. {proposed_pr.title}{deps}")


def _print_waves(prs):
    waves = compute_waves(prs)
    if len(waves) <= 1:
        return
    print(f"\n  {BOLD}Parallelism:{RESET}")
    print(f"  {DIM}{'─' * 60}{RESET}")
    for index, wave in enumerate(waves, 1):
        names = ", ".join(f"PR #{proposed_pr.index}" for proposed_pr in wave)
        parallel = " (can merge in parallel)" if len(wave) > 1 else ""
        print(f"  Wave {index}: {names}{parallel}")


def _build_warnings(proposed_pr, risk_label, complexity_label):
    """Build human-readable warning lines for a PR."""
    warnings = []
    if risk_label in ("high", "critical"):
        color = RED if risk_label == "critical" else YELLOW
        reasons = _risk_reasons(proposed_pr)
        reason_text = f" — {reasons}" if reasons else ""
        label_text = "High risk" if risk_label == "high" else "Critical risk"
        warnings.append(f"{color}⚠ {label_text}{reason_text}{RESET}")
    if complexity_label in ("complex", "very_complex"):
        label_text = "Complex" if complexity_label == "complex" else "Very complex"
        warnings.append(f"{YELLOW}⚠ {label_text} — consider splitting or careful review{RESET}")
    return warnings


def _risk_reasons(proposed_pr):
    """Describe why a PR is risky in plain language."""
    reasons = []
    if proposed_pr.total_code_lines > 300:
        reasons.append("large change")
    if len(proposed_pr.depends_on) >= 3:
        reasons.append("many dependencies")
    extensions = {file.ext for file in proposed_pr.files if not file.is_text_or_docs}
    if len(extensions) >= 3:
        reasons.append("touches multiple languages")
    if proposed_pr.total_complexity > 30:
        reasons.append("high complexity")
    return ", ".join(reasons)


def _risk_label(score):
    """Convert 0.0-1.0 risk score to human label."""
    if score < 0.2:
        return "low"
    if score < 0.5:
        return "moderate"
    if score < 0.7:
        return "high"
    return "critical"


def _complexity_label(total_complexity):
    """Convert cyclomatic complexity to human label."""
    if total_complexity <= 5:
        return "simple"
    if total_complexity <= 15:
        return "moderate"
    if total_complexity <= 30:
        return "complex"
    return "very_complex"


def _status_label(status):
    """Convert A/M/D/R to human-readable word."""
    return {
        "A": f"{GREEN}new{RESET}",
        "M": f"{CYAN}modified{RESET}",
        "D": f"{RED}deleted{RESET}",
        "R": f"{YELLOW}renamed{RESET}",
    }.get(status, status)
