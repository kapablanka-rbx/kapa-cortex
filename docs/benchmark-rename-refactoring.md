# Token Benchmark: Rename Refactoring

## Task

Rename `solveConstraints` → `solveContactConstraints` in bullet3 (2,869 files, C++).
Virtual method in `btDiscreteDynamicsWorld` with overrides in 3 subclasses.

**Result**: 8 files changed, 15 lines modified.

## With kapa-cortex (lookup + refs)

| Step | Tool call | Tokens |
|------|-----------|--------|
| `kapa-cortex lookup solveConstraints` | Daemon query (ctags index) | 691 |
| `kapa-cortex refs btDiscreteDynamicsWorld::solveConstraints` | Daemon + LSP | 50 |
| Reasoning (structured output, minimal) | — | 500 |
| sed + verify | Bash | 300 |
| **Total** | **2 tool calls** | **1,541** |

## Without kapa-cortex (grep + read)

| Step | Tool call | Tokens |
|------|-----------|--------|
| `rg solveConstraints` (31 hits across 2 symbol families) | Grep | 416 |
| Read `btDynamicsWorld.h` (check if parent declares it) | Read | 1,464 |
| `rg ": public btDiscreteDynamicsWorld"` (find subclasses) | Grep | 146 |
| `rg` for grandchild classes | Grep | 93 |
| Read 3 subclass headers (check which override) | Read | 4,592 |
| Read `btSoftBody.h` (confirm different signature) | Read | 50 |
| Reasoning (manual classification of 31 grep hits) | — | 2,000 |
| sed + verify | Bash | 300 |
| **Total** | **8 tool calls** | **9,061** |

## Comparison

| Metric | With skill | Without skill | Improvement |
|--------|-----------|--------------|-------------|
| Discovery tokens | 741 | 6,761 | 9.1x |
| Reasoning tokens | 500 | 2,000 | 4.0x |
| Total tokens | 1,541 | 9,061 | **5.9x** |
| Tool calls | 2 | 8 | **4x** |

## Why the skill wins

**lookup** returns FQN-scoped definitions. Claude sees `btDiscreteDynamicsWorld::solveConstraints` vs `btSoftBody::solveConstraints` instantly — no file reading needed to understand the inheritance tree.

**refs** returns LSP references at the exact symbol position. No grep filtering, no false positives from similarly-named methods in unrelated classes.

The **4,592 tokens** spent reading 3 subclass header files (to check which ones override the method) are eliminated entirely. `lookup` already listed every definition with its scope, kind, file, and line.

Reasoning drops from 2,000 to 500 tokens because the skill output is structured (scope, kind, file, line) vs raw grep text that needs manual parsing and signature comparison.

## Methodology

- Both approaches produce identical diffs (verified)
- Token estimates: output bytes / 4 for tool results, manual estimate for reasoning
- Tested on bullet3 (github.com/bulletphysics/bullet3), 2,869 indexed files
- kapa-cortex uses universal-ctags for symbol index + clangd LSP for references
- Branch `refactor-with-skill` in bullet3 has the committed result
