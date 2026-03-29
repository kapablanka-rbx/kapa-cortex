# kapa-stacker

Split feature branches into reviewable, dependency-ordered stacked PRs.

Analyzes code dependencies across 15+ languages, groups files into small PRs
(~3 files, ~200 lines), generates git commands to create the branches, and
uses a local LLM (ollama) for smarter grouping and PR descriptions.

## Install

```bash
pip install -e .

# Now use it anywhere:
kapa-stacker --help
```

Or without installing:

```bash
pip install networkx
python kapa-stacker.py --help
```

## Quick Start

```bash
# On your feature branch, analyze and see proposed stacked PRs
kapa-stacker

# Generate an execution plan with all git commands
kapa-stacker --generate-plan

# Check plan progress
kapa-stacker --check-plan

# Execute the plan (interactive, with retry/skip)
kapa-stacker --run-plan

# Dry run first (preview without executing)
kapa-stacker --run-plan --dry-run

# If your base branch isn't main
kapa-stacker --base develop
```

## Extract Specific Changes

Pull a subset of files into a separate PR branch using natural language:

```bash
kapa-stacker --extract "gradle init-script files"
kapa-stacker --extract "src/core/ changes"
kapa-stacker --extract "all CMakeLists.txt changes"
kapa-stacker --extract "python test files"
kapa-stacker --extract "the authentication refactor"
```

## Output Formats

```bash
kapa-stacker --json
kapa-stacker --visualize
kapa-stacker --dot-file graph.dot
kapa-stacker --print-commands
kapa-stacker --shell-script > create-stack.sh
```

## AI Mode

AI is **on by default** using ollama. If ollama isn't running, it silently
falls back to rule-based analysis. No API keys needed.

```bash
# First time setup (installs ollama, pulls model, smoke tests it)
kapa-stacker --setup

# Use smallest model (~1.6 GB)
kapa-stacker --setup-minimal

# Check what's available
kapa-stacker --ai-check

# Disable AI
kapa-stacker --no-ai
```

## Supported Languages

Python, C, C++, Java, Kotlin, Go, Rust, JavaScript, TypeScript,
Gradle (Groovy + KTS), CMake, Buck2, BXL, Starlark/Bazel, Groovy.

## Architecture (DDD + Layers)

```
src/
  domain/                          # Pure logic, zero external deps
    changed_file.py                  Entity
    proposed_pr.py                   Entity
    execution_plan.py                Entity (PlanStep, PRPlan)
    import_ref.py                    Value object
    symbol_def.py                    Value object
    file_complexity.py               Value object
    risk_score.py                    Value object
    merge_strategy.py                Enum
    step_status.py                   Enum
    extraction_rule.py               Value object
    test_pair.py                     Value object
    dependency_resolver.py           Service
    file_grouper.py                  Service
    test_pair_finder.py              Service
    risk_scorer.py                   Service
    merge_strategy_assigner.py       Service
    merge_order_resolver.py          Service
    file_matcher.py                  Service
    prompt_parser.py                 Service
    pr_namer.py                      Service
    ports/                           Interfaces
      git_reader.py, import_parser.py, symbol_extractor.py,
      complexity_analyzer.py, llm_service.py, plan_persistence.py,
      command_runner.py

  application/                     # Use cases
    analyze_branch.py                Full analysis pipeline
    extract_files.py                 Prompt-driven extraction
    generate_plan.py                 Git execution plan
    execute_plan.py                  Run plan steps

  infrastructure/                  # All I/O
    git/                             GitClient, ShellCommandRunner
    parsers/                         Language detection, regex/AST parsers
    complexity/                      lizard, scc analyzers
    llm/                             Ollama, llama-cpp backends, setup
    persistence/                     JSON plan store

  presentation/                    # CLI + output
    cli.py                           Entry point
    reporters/                       text, JSON, DOT, mermaid, plan
```

## Running Tests

```bash
# Domain tests (fast, pure logic, zero mocks)
python -m unittest discover -s tests/domain -v

# All tests
python -m unittest discover -s tests -v
```
