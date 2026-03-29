# kapa-cortex

Local code intelligence engine — stacked PRs, repo analysis, dependency graphs.

Analyzes code dependencies across 15+ languages using tree-sitter, ast-grep,
ctags, lizard, difftastic, and co-change history. Splits feature branches into
small PRs (~3 files, ~200 lines), generates git commands, and uses a local LLM
(ollama) for smarter grouping. Runs as a CLI, daemon, or Claude Code skill.

## Install

```bash
pip install -e .

# Now use it anywhere:
kapa-cortex --help
```

Or without installing:

```bash
pip install networkx
python kapa-cortex.py --help
```

## Quick Start

```bash
# Pre-compute caches (ctags, imports, co-change, complexity)
kapa-cortex --index

# Analyze and see proposed stacked PRs
kapa-cortex

# JSON output (for scripts and Claude Code skill)
kapa-cortex --json

# Generate an execution plan with all git commands
kapa-cortex --generate-plan

# Dry run first, then execute
kapa-cortex --run-plan --dry-run
kapa-cortex --run-plan

# Check plan progress
kapa-cortex --check-plan

# If your base branch isn't main
kapa-cortex --base develop
```

## Daemon Mode

Start once, query many times — keeps LSP servers warm and index in memory:

```bash
kapa-cortex --daemon              # start (boots pyright, clangd, gopls, jdtls, rust-analyzer)
kapa-cortex --daemon-status       # check status
kapa-cortex --query "analyze"     # fast query via daemon
kapa-cortex --daemon-stop         # stop
```

## Extract Specific Changes

Pull a subset of files into a separate PR branch using natural language:

```bash
kapa-cortex --extract "gradle init-script files"
kapa-cortex --extract "src/core/ changes"
kapa-cortex --extract "all CMakeLists.txt changes"
kapa-cortex --extract "python test files"
kapa-cortex --extract "the authentication refactor"
```

## Claude Code Skill

Install as a Claude Code skill for token-efficient analysis:

```bash
kapa-cortex --install-skill
```

Claude Code will auto-trigger on phrases like "split this branch into PRs",
"analyze my changes", or "what depends on this file".

## Output Formats

```bash
kapa-cortex --json
kapa-cortex --visualize
kapa-cortex --dot-file graph.dot
kapa-cortex --print-commands
kapa-cortex --shell-script > create-stack.sh
```

## AI Mode

AI is **on by default** using ollama. If ollama isn't running, it silently
falls back to rule-based analysis. No API keys needed.

```bash
kapa-cortex --setup              # install all deps
kapa-cortex --setup-minimal      # smallest model (~1.6 GB)
kapa-cortex --ai-check           # check backends
kapa-cortex --no-ai              # disable AI
```

## Supported Languages

Python, C, C++, Java, Kotlin, Go, Rust, JavaScript, TypeScript,
Gradle (Groovy + KTS), CMake, Buck2, BXL, Starlark/Bazel, Groovy.

Analysis chain: LSP (daemon) → tree-sitter → ast-grep → regex.

## Architecture (DDD + 4 Layers)

```
src/
  domain/          # Pure logic, zero external deps
  application/     # Use cases, orchestration
  infrastructure/  # Git, parsers, LSP, LLM, caches
  interface/       # CLI, daemon, reporters, skill
```

## Running Tests

```bash
python -m unittest discover -s tests -v    # all tests
python -m unittest discover -s tests/domain -v  # domain only (fast)
```
