#!/usr/bin/env bash
#
# A/B token benchmark: Claude Code with vs without kapa-cortex skill.
# Runs two headless Claude sessions with the same prompt, compares tokens.
#
set -uo pipefail

SKILL_GLOBAL="$HOME/.claude/skills/kapa-cortex"
SKILL_LOCAL="/home/kapablanka/repos/bullet3/.claude/skills/kapa-cortex"
BACKUP_GLOBAL="/tmp/kapa-cortex-skill-backup-global"
BACKUP_LOCAL="/tmp/kapa-cortex-skill-backup-local"
BULLET3="/home/kapablanka/repos/bullet3"

PROMPT='In bullet3, the method solveConstraints exists in two separate class hierarchies: btDiscreteDynamicsWorld and btSoftBody. I want to rename ONLY the btDiscreteDynamicsWorld::solveConstraints to solveContactConstraints. Do NOT touch btSoftBody::solveConstraints or btSoftBodySolver::solveConstraints. List every file and line that needs to change, and explain how you determined which occurrences belong to the DynamicsWorld hierarchy vs the SoftBody hierarchy. Do NOT make any edits.'

echo "============================================"
echo "  A/B Token Benchmark"
echo "  Task: rename solveConstraints in bullet3"
echo "============================================"
echo ""

# ── Run A: WITH skill ──
echo "=== Run A: WITH kapa-cortex skill ==="
echo ""

# Make sure skill is installed
if [ ! -f "$SKILL_GLOBAL/SKILL.md" ] && [ ! -f "$SKILL_LOCAL/SKILL.md" ]; then
    echo "ERROR: Skill not installed"
    exit 1
fi

# Kill any stale daemon
rm -f /tmp/kapa-cortex.sock

cd "$BULLET3"
claude -p "$PROMPT" --output-format json 2>/dev/null > /tmp/benchmark_with_skill.json
echo "  Output saved to /tmp/benchmark_with_skill.json"
echo ""

# Kill daemon before next run
rm -f /tmp/kapa-cortex.sock

# ── Run B: WITHOUT skill ──
echo "=== Run B: WITHOUT kapa-cortex skill ==="
echo ""

# Back up and remove ALL skill copies
[ -d "$SKILL_GLOBAL" ] && cp -r "$SKILL_GLOBAL" "$BACKUP_GLOBAL" && rm -rf "$SKILL_GLOBAL"
[ -d "$SKILL_LOCAL" ] && cp -r "$SKILL_LOCAL" "$BACKUP_LOCAL" && rm -rf "$SKILL_LOCAL"
echo "  Skills removed (global + local)"

cd "$BULLET3"
claude -p "$PROMPT" --output-format json 2>/dev/null > /tmp/benchmark_without_skill.json
echo "  Output saved to /tmp/benchmark_without_skill.json"
echo ""

# ── Restore skill ──
[ -d "$BACKUP_GLOBAL" ] && cp -r "$BACKUP_GLOBAL" "$SKILL_GLOBAL" && rm -rf "$BACKUP_GLOBAL"
[ -d "$BACKUP_LOCAL" ] && cp -r "$BACKUP_LOCAL" "$SKILL_LOCAL" && rm -rf "$BACKUP_LOCAL"
echo "  Skills restored"
echo ""

# ── Compare ──
echo "=== Results ==="
echo ""
print_usage() {
    local file="$1"
    python3 -c "
import sys, json
data = json.load(open('$file'))
u = data.get('usage', {})
inp = u.get('input_tokens', 0) + u.get('cache_creation_input_tokens', 0) + u.get('cache_read_input_tokens', 0)
out = u.get('output_tokens', 0)
cost = data.get('total_cost_usd', 0)
turns = data.get('num_turns', 0)
duration = data.get('duration_ms', 0) / 1000
print(f'  Input tokens:  {inp}')
print(f'  Output tokens: {out}')
print(f'  Total tokens:  {inp + out}')
print(f'  Cost:          \${cost:.4f}')
print(f'  Turns:         {turns}')
print(f'  Duration:      {duration:.1f}s')
" 2>/dev/null || echo "  (could not parse — check $file)"
}

echo "WITH skill:"
print_usage /tmp/benchmark_with_skill.json

echo ""
echo "WITHOUT skill:"
print_usage /tmp/benchmark_without_skill.json

echo ""
echo "Raw output files:"
echo "  /tmp/benchmark_with_skill.json"
echo "  /tmp/benchmark_without_skill.json"
