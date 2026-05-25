#!/usr/bin/env bash
# Re-run ALL audit timeouts ONE AT A TIME in strict isolation, cleaning up cbmc
# before and after each so orphaned solvers can't poison the next measurement.
# Budget 1800s: anything that still times out here is genuinely intractable.
set -uo pipefail
cd /home/anatoly/percolator

LOG_DIR=/home/anatoly/percolator/kani_isolated_full
mkdir -p "$LOG_DIR"
RESULT="$LOG_DIR/results.tsv"
BUDGET_S=${BUDGET_S:-900}
# Resume-friendly: keep prior results, only add header if file is new.
[ -f "$RESULT" ] || echo -e "proof\tcbmc_time_s\twall_s\tstatus" > "$RESULT"

# Proof list: every TIMEOUT in the master audit that still exists as a proof fn.
mapfile -t PROOFS < "$LOG_DIR/proofs.txt"

n=0
total=${#PROOFS[@]}
for proof in "${PROOFS[@]}"; do
    n=$((n+1))
    # Skip proofs already recorded (resume after restart).
    if cut -f1 "$RESULT" | grep -qxF "$proof"; then
        printf "[%s] (%d/%d) %s -> SKIP (already done)\n" "$(date +%H:%M:%S)" "$n" "$total" "$proof"
        continue
    fi
    pkill -KILL -f cbmc 2>/dev/null; sleep 1
    logf="$LOG_DIR/${proof}.log"
    start=$(date +%s)
    if timeout --kill-after=30 "$BUDGET_S" cargo kani --tests --features fuzz --exact \
        --harness "$proof" --output-format regular > "$logf" 2>&1; then
        status="PASS"
    else
        ec=$?
        if [ $ec -eq 124 ] || [ $ec -eq 137 ]; then status="TIMEOUT"; else status="FAIL($ec)"; fi
    fi
    end=$(date +%s)
    wall=$((end - start))
    cbmc_t=$(grep -oE "Verification Time: [0-9.]+s" "$logf" | grep -oE "[0-9.]+" | head -1)
    cbmc_t=${cbmc_t:-NA}
    printf "%s\t%s\t%s\t%s\n" "$proof" "$cbmc_t" "$wall" "$status" >> "$RESULT"
    printf "[%s] (%d/%d) %s -> %s (cbmc=%ss wall=%ss)\n" \
        "$(date +%H:%M:%S)" "$n" "$total" "$proof" "$status" "$cbmc_t" "$wall"
    pkill -KILL -f cbmc 2>/dev/null
done

echo "========================================="
column -t -s$'\t' "$RESULT"
echo "========================================="
