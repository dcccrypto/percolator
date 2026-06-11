#!/usr/bin/env bash
# Kani function-contract layer runner — ISOLATED from the main proof suite.
#
# Why isolated: -Z function-contracts slows kani-compiler ~5x crate-wide, so
# it must never touch the per-proof suite budget. This script uses (a) the
# `contracts` cargo feature to gate the contract attrs/harnesses, (b) the CLI
# -Z flag (NOT Cargo.toml, so plain `cargo kani` never sees it), and (c) a
# separate CARGO_TARGET_DIR so the contracts build cache never thrashes the
# suite's. Compile is bounded (~15-25 min); each leaf check then solves in
# seconds. Budget is compile-inclusive.
#
# Usage: bash scripts/contracts_runner.sh [BUDGET_S]   (reads contracts/proofs.txt)
set -uo pipefail
cd "$(dirname "$0")/.."

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target/contracts}"
LOG_DIR="${LOG_DIR:-kani_contracts}"
BUDGET_S="${1:-1800}"
mkdir -p "$LOG_DIR"
RESULT="$LOG_DIR/results.tsv"
[ -f "$RESULT" ] || echo -e "contract_harness\twall_s\tstatus" > "$RESULT"

cleanup() {
    pkill -9 -f 'cargo-kani'  2>/dev/null
    pkill -9 -f 'kani-driver' 2>/dev/null
    pkill -9 -x cbmc          2>/dev/null
}

mapfile -t HARNESSES < "$LOG_DIR/proofs.txt"
for h in "${HARNESSES[@]}"; do
    [ -z "$h" ] && continue
    if cut -f1 "$RESULT" | grep -qxF "$h"; then
        echo "[$(date +%H:%M:%S)] $h -> SKIP (done)"; continue
    fi
    cleanup; sleep 1
    logf="$LOG_DIR/${h}.log"
    start=$(date +%s)
    if timeout --kill-after=30 "$BUDGET_S" cargo kani -Z function-contracts \
        --features fuzz,contracts --harness "$h" --output-format terse \
        > "$logf" 2>&1; then
        status="PASS"
    else
        ec=$?
        if [ $ec -eq 124 ] || [ $ec -eq 137 ]; then status="TIMEOUT"; else status="FAIL($ec)"; fi
    fi
    wall=$(( $(date +%s) - start ))
    grep -qE "Complete - .* 0 failures" "$logf" || [ "$status" = PASS ] || true
    printf "%s\t%s\t%s\n" "$h" "$wall" "$status" >> "$RESULT"
    printf "[%s] %s -> %s (%ss)\n" "$(date +%H:%M:%S)" "$h" "$status" "$wall"
    cleanup
done
echo "====="; column -t -s$'\t' "$RESULT"
