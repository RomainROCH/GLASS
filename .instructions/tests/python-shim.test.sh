#!/bin/bash
# Canary test: verify the ./python shim is operational.
# Run from GLASS repo root:  bash .instructions/tests/python-shim.test.sh

set -uo pipefail
cd "$(git rev-parse --show-toplevel)"

PASS=0
FAIL=0
WARN=0

check() {
    local label="$1"; shift
    if "$@" &>/dev/null; then
        echo "  ✅ $label"
        ((PASS++))
    else
        echo "  ❌ $label"
        ((FAIL++))
    fi
}

warn() {
    echo "  ⚠️  $1"
    ((WARN++))
}

echo "=== Python shim canary test ==="
echo ""

# 1. Shim exists and is executable
check "./python shim exists"      test -f ./python
check "./python is executable"    test -x ./python

# 2. sync_wgpu.py exists
check "sync_wgpu.py exists"      test -f sync_wgpu.py

# 3. uv is available (prerequisite for ./python)
if command -v uv &>/dev/null; then
    check "uv is on PATH"            command -v uv
    check "./python --version works"  ./python --version
    check "sync_wgpu.py syntax OK"    ./python -c "import ast; ast.parse(open('sync_wgpu.py').read())"
else
    warn "uv is not on PATH — skipping runtime checks (install: curl -LsSf https://astral.sh/uv/install.sh | sh)"
fi

# 4. Bare python should NOT work (confirms isolation)
if python --version &>/dev/null 2>&1; then
    warn "bare 'python' is on PATH (unexpected — isolation not enforced)"
else
    echo "  ✅ bare 'python' is NOT on PATH (isolation confirmed)"
    ((PASS++))
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed, $WARN warnings ==="
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
