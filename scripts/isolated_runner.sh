#!/usr/bin/env bash
# Generic isolated proof runner: one proof at a time, kill cbmc + the whole
# timeout/cargo-kani/kani-driver chain before AND after each (orphans escape
# `timeout`'s process group and poison the next measurement).
# Usage: LOG_DIR=<dir> BUDGET_S=<s> bash isolated_runner.sh   (reads $LOG_DIR/proofs.txt)
set -uo pipefail
cd /home/anatoly/percolator

LOG_DIR=${LOG_DIR:?set LOG_DIR}
RESULT="$LOG_DIR/results.tsv"
BUDGET_S=${BUDGET_S:-900}
[ -f "$RESULT" ] || echo -e "proof\tcbmc_time_s\twall_s\tstatus" > "$RESULT"

cleanup_kani() {
    pkill -9 -f 'timeout --kill-after' 2>/dev/null
    pkill -9 -f 'cargo-kani'           2>/dev/null
    pkill -9 -f 'kani-driver'          2>/dev/null
    pkill -9 cbmc                      2>/dev/null
}

mapfile -t PROOFS < "$LOG_DIR/proofs.txt"
n=0; total=${#PROOFS[@]}
for proof in "${PROOFS[@]}"; do
    n=$((n+1))
    if cut -f1 "$RESULT" | grep -qxF "$proof"; then
        printf "[%s] (%d/%d) %s -> SKIP (already done)\n" "$(date +%H:%M:%S)" "$n" "$total" "$proof"
        continue
    fi
    cleanup_kani; sleep 1
    logf="$LOG_DIR/${proof}.log"
    start=$(date +%s)
    if timeout --kill-after=30 "$BUDGET_S" cargo kani --tests --features fuzz --exact \
        --harness "$proof" --output-format regular > "$logf" 2>&1; then
        status="PASS"
    else
        ec=$?
        if [ $ec -eq 124 ] || [ $ec -eq 137 ]; then status="TIMEOUT"; else status="FAIL($ec)"; fi
    fi
    end=$(date +%s); wall=$((end - start))
    cbmc_t=$(grep -oE "Verification Time: [0-9.]+s" "$logf" | grep -oE "[0-9.]+" | head -1)
    cbmc_t=${cbmc_t:-NA}
    printf "%s\t%s\t%s\t%s\n" "$proof" "$cbmc_t" "$wall" "$status" >> "$RESULT"
    printf "[%s] (%d/%d) %s -> %s (cbmc=%ss wall=%ss)\n" \
        "$(date +%H:%M:%S)" "$n" "$total" "$proof" "$status" "$cbmc_t" "$wall"
    cleanup_kani
done
echo "====="; column -t -s$'\t' "$RESULT"; echo "====="
