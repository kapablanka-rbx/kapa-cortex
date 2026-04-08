# kapa-cortex

Local code intelligence engine — stacked PRs, repo analysis, dependency graphs.

Indexes source repositories using ctags, tree-sitter, and LSP servers.
Answers structural queries (definitions, references, impact, call graphs)
and splits feature branches into small stacked PRs (~3 files, ~200 lines).
Runs as a daemon, CLI, MCP server, or Claude Code skill.

## Build

```bash
cd core
cargo build --release
```

## Quick Start

```bash
kapa-cortex index                       # build symbol + call graph index
kapa-cortex analyze                     # see proposed stacked PRs
kapa-cortex analyze --json              # JSON output
kapa-cortex analyze --brief             # token-efficient output for AI agents
kapa-cortex analyze --base develop      # custom base branch
```

## Daemon Mode

Start once, query many times — keeps LSP servers warm and index in memory:

```bash
kapa-cortex daemon start                # boots clangd, pyright, gopls, rust-analyzer, ...
kapa-cortex daemon status               # check health
kapa-cortex daemon stop
```

Query commands auto-start the daemon if it's not running.

## Code Intelligence

```bash
kapa-cortex defs MyClass                # find all definitions of a symbol
kapa-cortex inspect MyClass::method     # signature, callers, callees, overrides
kapa-cortex refs MyClass::method        # all references via LSP
kapa-cortex rdeps src/auth.rs           # what breaks if this file changes
kapa-cortex deps src/auth.rs            # transitive dependencies
kapa-cortex trace source_fn target_fn   # call path between two symbols
kapa-cortex symbols src/auth.rs         # list all symbols in a file
kapa-cortex hotspots                    # rank files by risk (complexity * dependents)
```

## Extract Specific Changes

Pull a subset of files into a separate PR branch:

```bash
kapa-cortex extract "gradle files"
kapa-cortex extract "src/core/ changes" --branch pr/core-refactor
kapa-cortex extract "all CMakeLists.txt changes"
```

## Buck2 Build System

```bash
kapa-cortex buck2 targets               # list all targets
kapa-cortex buck2 owner src/main.rs     # find owning target
kapa-cortex buck2 deps //app:lib        # target dependencies
kapa-cortex buck2 rdeps //app:lib       # reverse target dependencies
```

## MCP Server

Run as an MCP server for AI agent integration:

```bash
kapa-cortex mcp                         # stdio JSON-RPC 2.0
```

## Claude Code Skill

```bash
kapa-cortex install-skill               # install SKILL.md to ~/.claude/
```

## Output Modes

All query commands support three output modes:

- `--json` — full structured JSON
- `--brief` — compact text, optimized for AI agent token efficiency
- (default) — human-readable with ANSI colors

## Supported Languages

Python, C, C++, Java, Kotlin, Go, Rust, JavaScript, TypeScript,
Gradle (Groovy + KTS), CMake, Buck2, BXL, Starlark/Bazel, Groovy, Lua.

Analysis chain: LSP (daemon) -> tree-sitter -> ctags -> regex.

## Architecture

DDD with 4 layers. See [docs/architecture.md](docs/architecture.md).

```
core/src/
  domain/          Pure logic, zero external deps
  application/     Use cases: analyze, extract, index
  infrastructure/  Git, parsers, LSP, SQLite, complexity
  iface/           CLI, daemon, MCP, reporters
```

## Tests

```bash
cd core
cargo test --lib                        # 146 unit tests
cargo test --test pr_splitting          # 11 integration scenarios
cargo test                              # all tests
```
