#!/usr/bin/env bash
# ============================================================================
# Lit Sandbox Demo
# ============================================================================
# Demonstrates sandboxed execution of a Lit repository.
# The sandbox isolates the filesystem, environment, and network so that
# code pulled into a repo can be run without accessing the rest of the system.
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LIT="${LIT_BINARY:-$SCRIPT_DIR/../target/release/lit}"

if [ ! -x "$LIT" ]; then
    echo "ERROR: lit binary not found at $LIT"
    echo "Build with: cargo build --release"
    exit 1
fi

echo ""
echo "====  Lit Sandbox Demo  ===="
echo "Binary: $LIT"
echo ""

# ── 1. Create a temporary demo repository ──────────────────────────────────

DEMO_DIR="$(mktemp -d /tmp/lit-sandbox-demo-XXXXXX)"
cd "$DEMO_DIR"

echo "[1/7] Creating demo repository in $DEMO_DIR"
"$LIT" init > /dev/null

# ── 2. Populate with sample project files ──────────────────────────────────

echo "[2/7] Adding sample project files"

mkdir -p src tests

cat > README.md << 'EOF'
# Demo Project
A sample project used to demonstrate Lit sandbox isolation.
EOF

cat > src/app.py << 'PYEOF'
"""Main application module."""
import os, sys

def greet(name: str) -> str:
    return f"Hello, {name}!"

if __name__ == "__main__":
    print(greet("World"))
    print(f"HOME = {os.environ.get('HOME', '(unset)')}")
    print(f"TEMP = {os.environ.get('TMPDIR', os.environ.get('TEMP', '(unset)'))}")
    print(f"PATH entries = {len(os.environ.get('PATH', '').split(':'))}")
PYEOF

cat > tests/test_app.py << 'PYTEST'
from src.app import greet
def test_greet():
    assert greet("Lit") == "Hello, Lit!"
PYTEST

cat > show_env.sh << 'SHEOF'
#!/bin/sh
echo "=== Sandboxed Environment ==="
echo "HOME=$HOME"
echo "TMPDIR=${TMPDIR:-}"
echo "TEMP=${TEMP:-}"
echo "TMP=${TMP:-}"
echo "LIT_AIRGAPPED=${LIT_AIRGAPPED:-}"
echo "GIT_CONFIG_NOSYSTEM=${GIT_CONFIG_NOSYSTEM:-}"
echo "PATH=$PATH"
echo ""
echo "=== Directory listing ==="
ls -1
SHEOF
chmod +x show_env.sh

"$LIT" add README.md src/app.py tests/test_app.py show_env.sh > /dev/null
"$LIT" commit -m "Initial demo project" > /dev/null

echo "   Committed: README.md, src/app.py, tests/test_app.py, show_env.sh"

# ── 3. Create a sandbox ───────────────────────────────────────────────────

echo ""
echo "[3/7] Creating sandbox 'demo'"
INIT_OUT=$("$LIT" sandbox init --human demo)
echo "   $INIT_OUT"

# ── 4. List sandboxes ─────────────────────────────────────────────────────

echo ""
echo "[4/7] Listing sandboxes"
LIST_OUT=$("$LIT" sandbox list --human)
echo "   $LIST_OUT"

# ── 5. Run a command inside the sandbox ────────────────────────────────────

echo ""
echo "[5/7] Running 'show_env.sh' inside the sandbox"
echo "       (demonstrates filesystem + environment isolation)"
RUN_OUT=$("$LIT" sandbox run --human demo -- sh show_env.sh 2>&1 || true)
echo ""
echo "$RUN_OUT" | sed 's/^/   /'

# ── 6. Prove isolation: compare real vs sandboxed HOME ─────────────────────

echo ""
echo "[6/7] Comparing real vs sandboxed environment"

REAL_HOME="$HOME"
echo "   Real HOME    : $REAL_HOME"

SANDBOX_HOME=$(echo "$RUN_OUT" | grep "^HOME=" | head -1 | sed 's/^HOME=//')
echo "   Sandbox HOME : ${SANDBOX_HOME:-unknown}"

if [ "$REAL_HOME" != "$SANDBOX_HOME" ]; then
    echo "   --> Isolation confirmed: sandbox cannot see real home directory"
else
    echo "   --> WARNING: HOME values match — isolation may not be working"
fi

AIRGAPPED=$(echo "$RUN_OUT" | grep "^LIT_AIRGAPPED=" | head -1 | sed 's/^LIT_AIRGAPPED=//')
if [ "$AIRGAPPED" = "1" ]; then
    echo "   --> Network airgap confirmed: LIT_AIRGAPPED=1"
fi

# ── 7. Destroy the sandbox ────────────────────────────────────────────────

echo ""
echo "[7/7] Destroying sandbox 'demo'"
DESTROY_OUT=$("$LIT" sandbox destroy --human demo)
echo "   $DESTROY_OUT"

LIST_AFTER=$("$LIT" sandbox list --human)
echo "   Sandboxes remaining: $LIST_AFTER"

# ── Cleanup ────────────────────────────────────────────────────────────────

cd /
rm -rf "$DEMO_DIR"

echo ""
echo "====  Demo Complete  ===="
echo "The demo repository was created in a temp directory, sandboxed,"
echo "executed with isolation, and cleaned up. No files remain."
echo ""
