#!/usr/bin/env bash
# Re-run "timed-out" proofs ONE AT A TIME in strict isolation.
# Kills cbmc before and after each run so orphaned solver processes
# (which escape `timeout`'s process group) can't poison measurements.
set -uo pipefail
cd /home/anatoly/percolator

LOG_DIR=/home/anatoly/percolator/kani_isolated_retry
mkdir -p "$LOG_DIR"
RESULT="$LOG_DIR/results.tsv"
BUDGET_S=${BUDGET_S:-900}
echo -e "proof\tcbmc_time_s\twall_s\tstatus" > "$RESULT"

PROOFS=(
    proof_v16_persisted_wire_rejects_i128_min_market_k_long
    proof_v16_persisted_wire_rejects_i128_min_market_f_short
    proof_v16_source_credit_rate_partial_backing_yields_partial_rate
    proof_v16_source_credit_rate_full_backing_caps_rate
    proof_v16_resolved_payout_readiness_b_stale_blocker
    proof_v16_persisted_wire_rejects_noncanonical_account_bool
)

for proof in "${PROOFS[@]}"; do
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
    printf "[%s] %s -> %s (cbmc=%ss wall=%ss)\n" "$(date +%H:%M:%S)" "$proof" "$status" "$cbmc_t" "$wall"
    pkill -KILL -f cbmc 2>/dev/null
done

echo "========================================="
column -t -s$'\t' "$RESULT"
echo "========================================="
