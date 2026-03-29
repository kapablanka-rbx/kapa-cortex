# Coding Standards

## Architecture: DDD with Layers

- **Domain layer** — Core business logic, pure Python, no external dependencies.
  Models, value objects, domain services, repository interfaces.
- **Application layer** — Use cases and orchestration. Coordinates domain objects
  and infrastructure. Thin: delegates, never implements business rules.
- **Infrastructure layer** — All external I/O: git, LLM backends, file system,
  parsers (tree-sitter, ast-grep, ctags, lizard), LSP clients, subprocess calls.
  Implements repository interfaces defined in domain.
- **Interface layer** — All entry points: CLI, daemon server, reporters,
  future MCP/web. Imports from application and domain. Never from infrastructure.
  No business logic — only translates between protocols and use cases.

## Single Responsibility Principle

- One responsibility per file.
- One responsibility per class.
- One responsibility per method/function.
- If you can't name it clearly in 3 words, it's doing too much. Split it.

## Variable Names

- **No single-letter or two-letter variable names.** Exception: `i`, `j`, `k` for loop indices.
- Names must be descriptive: `analyze_use_case`, not `uc`. `graph`, not `G`.
- Bad: `f`, `pr`, `tp`, `G`, `uc`, `rp`, `ag`, `ts`.
- Good: `changed_file`, `proposed_pr`, `test_pair`, `dep_graph`, `analyze_use_case`.

## Function Rules

- **Max 3 parameters.** Use dataclasses or config objects when more are needed.
- **Max 30 lines** per function. Sweet spot is **10-15 lines**.
- **No default parameter values.** Pass values explicitly at the call site.
- Small, composable, easy to test.
- Prefer pure functions (input → output, no side effects) in domain layer.

## Naming

- Files: `snake_case.py`, one class/concept per file.
- Classes: `PascalCase`.
- Functions/methods: `snake_case`, verb-first (`build_graph`, `parse_imports`).
- Constants: `UPPER_SNAKE_CASE`.
- Private: prefix with `_` only when truly internal.

## Project Structure

```
src/
  domain/            # Core models and business logic
  application/       # Use cases, orchestration
  infrastructure/    # Git, LLM, parsers, LSP, external tools
  interface/         # CLI, daemon, reporters, MCP (all entry points)
tests/
  domain/            # Mirrors src/domain/
  application/       # Mirrors src/application/
  infrastructure/    # Mirrors src/infrastructure/
  interface/         # Mirrors src/interface/
```

- Tests mirror source structure exactly.
- Test file matches source: `src/domain/pr_group.py` -> `tests/domain/test_pr_group.py`.

## Testing

- Every public function/class gets a test.
- Tests are small and focused: one assertion per test when practical.
- Use descriptive test names: `test_docs_exempt_from_line_limit`.
- No test should depend on another test.
- Mock infrastructure (git, LLM, filesystem) in domain/application tests.

## Dependencies

- Domain layer imports **nothing** from application, infrastructure, or interface.
- Application layer imports from domain only.
- Infrastructure layer imports from domain (to implement interfaces).
- Interface layer imports from application and domain. Never from infrastructure.
- Never import upward through layers.

## Code Navigation (mandatory for all agents and sub-agents)

ALWAYS use local CLI tools instead of reading files. Do not burn tokens on what the CPU can do.
These run on the local CPU — faster, cheaper, and more precise.

1. `ctags` for symbol indexing — finds definitions and their files in one shot.
2. `ast-grep` for structural code search — pattern matching over ASTs.
3. `fd` for filename/pattern search. Never use `find`.
4. `scc` or `lizard` for repo shape and metrics. Never hand-count or guess.
5. `rg` (ripgrep) as last resort for unstructured text search. Never use `grep`.

At the end of every prompt generate a navigation report with: tool used, query, results found, lines read, tokens consumed, false positives (files/lines/symbols returned but irrelevant), and false negatives (relevant items missed). Include sub-agent data. This is crucial for debugging and improving code navigation accuracy over time.

Only read files when you need full context after narrowing down with the tools above.

## General

- No dead code. Delete it, don't comment it out.
- No speculative abstractions. Build what's needed now.
- Prefer composition over inheritance.
- Type hints on all public interfaces.
- Docstrings on public classes and non-obvious functions. Skip for self-evident code.
