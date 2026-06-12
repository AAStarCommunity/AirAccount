#!/bin/bash
# MX93 Full Deploy Pipeline: git pull → build CA → restart → smoke test
# Run ON THE BOARD: bash /root/AirAccount/scripts/mx93/deploy.sh [branch]
# Claude calls this via serial after a code fix on Mac + push to GitHub.
set -eo pipefail

BRANCH="${1:-main}"
PROJECT="/root/AirAccount"
LOG="/tmp/deploy-$(date +%Y%m%d-%H%M%S).log"

c_green='\033[0;32m'; c_red='\033[0;31m'; c_cyan='\033[0;36m'; c_nc='\033[0m'
step() { echo -e "${c_cyan}[$(date +%H:%M:%S)] $*${c_nc}"; }
ok()   { echo -e "${c_green}✓ $*${c_nc}"; }
fail() { echo -e "${c_red}✗ $*${c_nc}"; exit 1; }

echo "═══════════════════════════════════════════════════"
echo "  AirAccount MX93 Deploy  branch=$BRANCH"
echo "  Log: $LOG"
echo "═══════════════════════════════════════════════════"

cd "$PROJECT"

# ── 1. Git pull ─────────────────────────────────────────
step "1/4  git pull origin $BRANCH"
if command -v git &>/dev/null; then
    git fetch origin "$BRANCH" 2>&1 | tee -a "$LOG"
    git checkout "$BRANCH"     2>&1 | tee -a "$LOG"
    git pull origin "$BRANCH"  2>&1 | tee -a "$LOG"
    ok "Code up to date ($(git log -1 --format='%h %s'))"
else
    echo "WARN: git not installed — skipping pull (run scripts/mx93/install-git.sh first)"
fi

# ── 2. Build CA (release) ───────────────────────────────
step "2/4  cargo build --release --bin kms-api-server"
cargo build --release --bin kms-api-server 2>&1 | tee -a "$LOG" | grep -E 'Compiling kms|Finished|error'
ok "Build complete: $(ls -lh target/release/kms-api-server | awk '{print $5, $6, $7, $8}')"

# ── 3. Restart service ──────────────────────────────────
step "3/4  systemctl restart kms-api"
systemctl restart kms-api
sleep 4
systemctl is-active kms-api >/dev/null && ok "kms-api is active" || fail "kms-api failed to start"

# ── 4. Smoke test ───────────────────────────────────────
step "4/4  smoke test"
bash "$PROJECT/scripts/mx93/test-smoke.sh" localhost:3000

echo ""
ok "Deploy complete — $(git log -1 --format='%h %s' 2>/dev/null || echo 'done')"
