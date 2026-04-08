#!/usr/bin/env bash
#
# Token benchmark: kapa-cortex vs grep+read on bullet3.
# Measures bytes of output (proxy for tokens: bytes/4 ≈ tokens).
#
set -uo pipefail

CORTEX="/home/kapablanka/repos/kapa-cortex/core/target/debug/kapa-cortex-core"
BULLET3="/home/kapablanka/repos/bullet3"

# Clean any stale daemon
rm -f /tmp/kapa-cortex.sock

cd "$BULLET3"

bytes() { wc -c | tr -d ' '; }

echo "============================================"
echo "  Token Benchmark: kapa-cortex vs grep+read"
echo "  bullet3: $(find src -name '*.cpp' -o -name '*.h' | wc -l) source files"
echo "============================================"
echo ""

# ── 1. Rename method: solveConstraints ──
echo "=== 1. Rename: solveConstraints ==="
echo ""

WITH=$("$CORTEX" defs solveConstraints --brief 2>/dev/null | bytes)
WITH2=$("$CORTEX" refs btDiscreteDynamicsWorld::solveConstraints --brief 2>/dev/null | bytes)
WITH_TOTAL=$((WITH + WITH2))

WITHOUT=$(rg -n "solveConstraints" src/ 2>/dev/null | bytes)
WITHOUT2=$(cat src/BulletDynamics/Dynamics/btDynamicsWorld.h 2>/dev/null | bytes)
WITHOUT3=$(rg -n ": public btDiscreteDynamicsWorld" src/ 2>/dev/null | bytes)
WITHOUT4=$(cat src/BulletDynamics/Dynamics/btDiscreteDynamicsWorldMt.h src/BulletDynamics/Featherstone/btMultiBodyDynamicsWorld.h src/BulletSoftBody/btDeformableMultiBodyDynamicsWorld.h 2>/dev/null | bytes)
WITHOUT_TOTAL=$((WITHOUT + WITHOUT2 + WITHOUT3 + WITHOUT4))

RATIO=$(echo "scale=1; $WITHOUT_TOTAL / $WITH_TOTAL" | bc 2>/dev/null || echo "?")
echo "  WITH:    ${WITH_TOTAL} bytes (defs: ${WITH}, refs: ${WITH2})"
echo "  WITHOUT: ${WITHOUT_TOTAL} bytes (grep: ${WITHOUT}, read headers: $((WITHOUT2 + WITHOUT4)), find subclasses: ${WITHOUT3})"
echo "  SAVINGS: ${RATIO}x"
echo ""

# ── 2. Change signature: addConstraint ──
echo "=== 2. Signature change: addConstraint ==="
echo ""

WITH=$("$CORTEX" defs addConstraint --brief 2>/dev/null | bytes)
WITH2=$("$CORTEX" refs btDiscreteDynamicsWorld::addConstraint --brief 2>/dev/null | bytes)
WITH_TOTAL=$((WITH + WITH2))

WITHOUT=$(rg -n "addConstraint" src/ 2>/dev/null | bytes)
WITHOUT2=$(rg -n "addConstraint\b" --type cpp src/ 2>/dev/null | bytes)
WITHOUT_TOTAL=$((WITHOUT + WITHOUT2))

RATIO=$(echo "scale=1; $WITHOUT_TOTAL / $WITH_TOTAL" | bc 2>/dev/null || echo "?")
echo "  WITH:    ${WITH_TOTAL} bytes (defs: ${WITH}, refs: ${WITH2})"
echo "  WITHOUT: ${WITHOUT_TOTAL} bytes (grep all: ${WITHOUT}, grep refined: ${WITHOUT2})"
echo "  SAVINGS: ${RATIO}x"
echo ""

# ── 3. Extract interface: btCollisionWorld ──
echo "=== 3. Extract interface: btCollisionWorld ==="
echo ""

WITH=$("$CORTEX" defs btCollisionWorld --brief 2>/dev/null | bytes)
WITH2=$("$CORTEX" inspect btCollisionWorld --brief 2>/dev/null | bytes)
WITH_TOTAL=$((WITH + WITH2))

WITHOUT=$(rg -n "btCollisionWorld" src/ 2>/dev/null | bytes)
WITHOUT2=$(cat src/BulletCollision/CollisionDispatch/btCollisionWorld.h 2>/dev/null | bytes)
WITHOUT3=$(cat src/BulletCollision/CollisionDispatch/btCollisionWorld.cpp 2>/dev/null | bytes)
WITHOUT4=$(rg -n ": public btCollisionWorld" src/ 2>/dev/null | bytes)
WITHOUT_TOTAL=$((WITHOUT + WITHOUT2 + WITHOUT3 + WITHOUT4))

RATIO=$(echo "scale=1; $WITHOUT_TOTAL / $WITH_TOTAL" | bc 2>/dev/null || echo "?")
echo "  WITH:    ${WITH_TOTAL} bytes (defs: ${WITH}, inspect: ${WITH2})"
echo "  WITHOUT: ${WITHOUT_TOTAL} bytes (grep: ${WITHOUT}, .h: ${WITHOUT2}, .cpp: ${WITHOUT3}, subclasses: ${WITHOUT4})"
echo "  SAVINGS: ${RATIO}x"
echo ""

# ── 4. Move to namespace: btVector3 ──
echo "=== 4. Namespace move: btVector3 ==="
echo ""

WITH=$("$CORTEX" defs btVector3 --brief 2>/dev/null | bytes)
WITH_TOTAL=$WITH

WITHOUT=$(rg -c "btVector3" src/ 2>/dev/null | bytes)
WITHOUT2=$(rg -n "btVector3" src/ 2>/dev/null | head -250 | bytes)
WITHOUT3=$(cat src/LinearMath/btVector3.h 2>/dev/null | bytes)
WITHOUT_TOTAL=$((WITHOUT + WITHOUT2 + WITHOUT3))

RATIO=$(echo "scale=1; $WITHOUT_TOTAL / $WITH_TOTAL" | bc 2>/dev/null || echo "?")
echo "  WITH:    ${WITH_TOTAL} bytes (defs only — unambiguous, sed is enough)"
echo "  WITHOUT: ${WITHOUT_TOTAL} bytes (grep counts: ${WITHOUT}, grep sample: ${WITHOUT2}, read header: ${WITHOUT3})"
echo "  SAVINGS: ${RATIO}x"
echo ""

# ── 5. Replace inheritance: btDynamicsWorld ──
echo "=== 5. Replace inheritance: btDynamicsWorld ==="
echo ""

WITH=$("$CORTEX" defs btDynamicsWorld --brief 2>/dev/null | bytes)
WITH2=$("$CORTEX" refs btDynamicsWorld --brief 2>/dev/null | bytes)
WITH_TOTAL=$((WITH + WITH2))

WITHOUT=$(rg -n "btDynamicsWorld" src/ 2>/dev/null | bytes)
WITHOUT2=$(cat src/BulletDynamics/Dynamics/btDynamicsWorld.h 2>/dev/null | bytes)
WITHOUT3=$(rg -n ": public btDynamicsWorld" src/ 2>/dev/null | bytes)
WITHOUT4=$(cat src/BulletDynamics/Dynamics/btDiscreteDynamicsWorld.h src/BulletDynamics/Dynamics/btDiscreteDynamicsWorldMt.h src/BulletDynamics/Featherstone/btMultiBodyDynamicsWorld.h 2>/dev/null | bytes)
WITHOUT_TOTAL=$((WITHOUT + WITHOUT2 + WITHOUT3 + WITHOUT4))

RATIO=$(echo "scale=1; $WITHOUT_TOTAL / $WITH_TOTAL" | bc 2>/dev/null || echo "?")
echo "  WITH:    ${WITH_TOTAL} bytes (defs: ${WITH}, refs: ${WITH2})"
echo "  WITHOUT: ${WITHOUT_TOTAL} bytes (grep: ${WITHOUT}, base .h: ${WITHOUT2}, find subclasses: ${WITHOUT3}, read subclasses: ${WITHOUT4})"
echo "  SAVINGS: ${RATIO}x"
echo ""

echo "============================================"
echo "  Note: tokens ≈ bytes / 4"
echo "============================================"
