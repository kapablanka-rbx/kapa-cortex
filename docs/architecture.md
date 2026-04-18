# kapa-cortex Architecture

## Overview

kapa-cortex is a local code intelligence engine written in Rust. It indexes
source repositories using ctags, tree-sitter, and LSP servers, then answers
structural queries (definitions, references, impact, call graphs) and proposes
stacked PR splits for large feature branches.

It runs as a daemon with warm LSP connections and an in-memory SQLite index,
or as a one-shot CLI for branch analysis. A Claude Code skill and an MCP
server expose the same capabilities to AI agents.

## Layer Architecture (DDD)

```
core/src/
  domain/            Pure business logic, no I/O
    entities.rs      ChangedFile, ProposedPr, AnalysisResult, ExecutionPlan
    model.rs         SymbolDef, CallerInfo, ExplainResult, TraceResult, etc.
    services.rs      Title generation, test pairing, risk scoring, merge strategy

  application/       Use cases — orchestrate domain + infrastructure
    analyze.rs       Analyze branch, group files into stacked PRs
    extract.rs       Extract file subsets matching a description
    indexer.rs        Full-repo indexing pipeline

  infrastructure/    All external I/O
    sqlite.rs        SQLite index: symbols, imports, edges, calls, targets
    tree_sitter.rs   Call extraction from source (Python, C++, Rust, Go, ...)
    ctags.rs         Symbol parsing via universal-ctags
    imports.rs       Import/include parsing (15+ languages)
    lsp.rs           LSP client: start servers, get references, find column
    git.rs           Git operations: diff-stat, diff-text, branch detection
    complexity.rs    Cyclomatic complexity via lizard
    walker.rs        Source file discovery (respects .gitignore)
    hasher.rs        Content hashing for incremental indexing
    buck2.rs         Buck2/Starlark TARGETS parser
    cochange.rs      Co-change frequency cache
    llm.rs           Rule-based title/summary generation (+ LLM placeholder)

  iface/             All entry points — never imports from infrastructure
    cli.rs           Clap CLI definition (subcommands, flags)
    server.rs        Unix socket daemon server
    handler.rs       Daemon request dispatcher (12 actions)
    protocol.rs      Length-prefixed JSON wire format
    daemon_client.rs Client: connect to daemon, send query, read response
    mcp.rs           MCP stdio server (JSON-RPC 2.0, 10 tools)
    reporter.rs      Analysis output formatting (text, JSON, brief)
```

### Dependency Rules

- **Domain** imports nothing from other layers.
- **Application** imports from domain only (+ infrastructure for I/O).
- **Infrastructure** imports from domain to implement interfaces.
- **Interface** imports from application and domain. Never from infrastructure.
- Exception: `main.rs` wires all layers together.

## What Is Implemented

### Daemon (interface layer)
- Unix socket server with length-prefixed JSON protocol
- 12 handler actions: lookup, symbols, explain, trace, impact, deps,
  hotspots, calls, refs, reindex, status, shutdown
- Auto-start: first query boots daemon + index if needed
- Multi-LSP: detects languages in repo, boots one server per language
  (clangd, pyright, gopls, rust-analyzer, lua-language-server)

### Indexing Pipeline (application layer)
- Full-repo index: walk files, hash for incremental, parse symbols (ctags),
  parse imports (15+ languages), extract calls (tree-sitter), resolve edges
- SQLite storage with tables: files, symbols, imports, edges, calls, targets
- Buck2 TARGETS parsing via starlark_syntax crate
- Complexity metrics via lizard subprocess

### Branch Analysis (application layer)
- `analyze`: diff against base, compute complexity, group into stacked PRs
- `extract`: match files by glob/prefix/keyword, pull test pairs, create branch
- Grouping rules: docs first, split by module, respect max_files/max_lines,
  keep test pairs together, assign risk and merge strategy
- Title generation: detects new types/functions from diff, falls back to
  common directory

### CLI (interface layer)
- Subcommands: daemon (start/stop/status), index, defs, refs, inspect,
  rdeps, deps, hotspots, symbols, trace, analyze, extract, buck2, status,
  reindex, mcp, install-skill
- Output modes: text (human), JSON, brief (token-efficient for AI agents)
- Auto-daemon: query commands start daemon if not running

### MCP Server (interface layer)
- JSON-RPC 2.0 over stdio
- 10 tools: defs, inspect, refs, rdeps, deps, trace, symbols, hotspots,
  status, (dispatches to daemon)
- Protocol version 2024-11-05

### Claude Code Skill
- SKILL.md triggers on code understanding questions
- Routes to CLI with --brief flag for token efficiency

## Test Coverage

### Tested (146 unit + 11 integration)

| Module | Tests | What's covered |
|--------|-------|----------------|
| domain/entities | 6 | is_text_or_docs, code_lines, module_key |
| domain/services | 26 | Title gen (10 languages), test pairing (5 conventions), prompt parsing, risk, merge strategy, diff reconstruction |
| application/analyze | 10 | Empty files, docs ordering, split by files/lines, risk, merge strategy, descriptions |
| application/extract | 1 | Prompt rule parsing |
| application/indexer | 4 | Import parsing, edge building |
| infrastructure/sqlite | 11 | Insert/lookup, callers/callees, deps, impact, trace, file counts |
| infrastructure/tree_sitter | 8 | Python/C++ call extraction, caller attribution, empty source |
| infrastructure/imports | 6 | Python, Rust, Starlark imports |
| infrastructure/llm | 7 | JSON parsing, rule-based title/summary |
| infrastructure/walker | 3 | File discovery, empty dir, skip dirs |
| infrastructure/hasher | 2 | File hashing |
| infrastructure/cochange | 2 | Cache filtering |
| infrastructure/complexity | 2 | Lizard parsing |
| infrastructure/buck2 | 64 | Starlark target parsing, package resolution |
| **tests/pr_splitting** | **11** | End-to-end scenarios: docs separation, test pairing, max files/lines, multi-module, deletions, class titles, complexity/risk, dependency chains, merge strategies |

### Not Tested

| Module | Lines | Gap |
|--------|-------|-----|
| iface/handler.rs | 466 | No tests. Largest file. All daemon request handlers. dispatch, split_fqn, read_line are pure and testable without a live daemon. |
| iface/mcp.rs | 190 | No tests. handle_request, dispatch_tool, require_str, initialize_result, tools_list are pure JSON in/out — high value to test since this is the AI agent API surface. |
| iface/protocol.rs | 60 | No tests. Request/Response serde, read/write over stream. Pure serialization logic. |
| iface/reporter.rs | 46 | No tests. Output formatting — low risk but easy to test. |
| iface/cli.rs | 229 | No tests. Clap struct definitions — clap validates at compile time, but output mode parsing is testable. |
| iface/server.rs | 73 | No tests. Socket lifecycle — hard to unit test, needs integration test. |
| iface/daemon_client.rs | 93 | No tests. Client logic — needs mock server or integration test. |
| infrastructure/git.rs | 85 | No tests. diff_stat, diff_text, cherry_pick_files, detect_base. Needs a temp git repo fixture. |
| infrastructure/ctags.rs | 60 | No tests. parse_file shells out to ctags. |
| infrastructure/lsp.rs | 313 | No tests. LSP client — protocol formatting is testable, server interaction needs integration test. |
| domain/model.rs | 84 | No tests. Pure data structs — only ImpactResult::total_affected and TraceResult::hops have logic. |
| application/extract.rs | 92 | 1 test. extract_files flow (matching, test pair pull-in) untested because it calls git. |

### Missing Test Types

1. **No end-to-end tests** — nothing runs the binary against a real repo.
   The current branch (61 commits, 150 files vs master) is a good candidate.
2. **No handler integration tests** — handler.rs is the core dispatcher but
   is only tested indirectly through manual daemon use.
3. **No MCP conformance tests** — the MCP server should be tested against
   the protocol spec (initialize -> tools/list -> tools/call).
4. **No git fixture tests** — analyze_branch and extract_files depend on
   git but have no test that sets up a temp repo.

## What Is Not Implemented

### Cloud Index (Phase 5)
S3-backed shared indexes so teams avoid per-engineer rebuilds.
Designed but not started. See CLAUDE.md project memory for context.

- Index keyed by `{repo}:{commit_hash}`
- CLI: `index --push`, `index --pull`, `index --auto`
- CI integration: build on merge to main, push to S3

### LLM-Powered Analysis
The `llm.rs` module has rule-based title/summary generation but the LLM
backend (ollama) is not wired in the Rust codebase. The Python version had
full ollama integration. Current Rust code generates titles from diff
patterns (new class/function detection) and falls back to directory names.

### Execution Plan
`ExecutionPlan` and `ExecutionStep` entities exist but there is no
`generate_plan` or `execute_plan` use case in the Rust codebase. The Python
version had full plan generation (git commands) and interactive execution.

### Incremental Indexing
`reindex` handler exists for targeted re-indexing of specific files.
File-system watching or git-hook-triggered incremental updates are not
implemented.

### Missing CLI Commands (vs Python version)
- `--generate-plan` / `--run-plan` / `--check-plan` — plan workflow
- `--visualize` / `--dot` — DOT graph output
- `--shell-script` / `--print-commands` — git command generation
- `--setup` — dependency installer
- `--no-ai` / `--ai-check` — LLM control

### Reporter Gaps
- No DOT graph reporter
- No plan reporter
- No extraction reporter (extract output is inline in main.rs)
