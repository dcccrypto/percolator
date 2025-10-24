#!/bin/bash
# Run all edge case Kani proofs
# These proofs test boundary conditions and corner cases
# Expected runtime: ~60 seconds total (16 proofs)

set -e

echo "Running edge case Kani proofs..."
echo "=================================="
echo ""

PROOFS=(
    # Zero value edge cases (5 proofs)
    "edge_zero_principal_bootstrap"
    "edge_zero_slope_no_withdrawals"
    "edge_zero_deficit_noop"
    "edge_zero_pnl_socialization"
    "edge_zero_reserved_pnl"

    # Reserved PnL interactions (3 proofs)
    "edge_reserved_pnl_blocks_socialization"
    "edge_reserved_pnl_throttle_interaction"
    "edge_reserved_pnl_conservation"

    # Total wipeout scenarios (2 proofs)
    "edge_total_wipeout_massive_deficit"
    "edge_exact_deficit_balance"

    # 3-user scenarios (3 proofs)
    "edge_3users_all_winners"
    "edge_3users_mixed_pnl"
    "edge_3users_sequential_ops"

    # Extreme boundaries (3 proofs)
    "edge_max_principal_deposit"
    "edge_exact_throttle_cap"
    "edge_multi_socialization_accumulation"
)

TIMEOUT=60  # 60 seconds per proof
PASSED=0
FAILED=0
TOTAL=${#PROOFS[@]}

for proof in "${PROOFS[@]}"; do
    echo "Running: $proof"

    if timeout $TIMEOUT cargo kani -p proofs-kani --harness "$proof" 2>&1 | grep -q "VERIFICATION:- SUCCESSFUL"; then
        echo "✅ $proof PASSED"
        ((PASSED++))
    else
        echo "❌ $proof FAILED or TIMEOUT"
        ((FAILED++))
    fi

    echo ""
done

echo "=================================="
echo "Edge Case Proof Results:"
echo "  Total:  $TOTAL"
echo "  Passed: $PASSED"
echo "  Failed: $FAILED"
echo ""

if [ $FAILED -eq 0 ]; then
    echo "🎉 All edge case proofs passed!"
    exit 0
else
    echo "⚠️  Some proofs failed or timed out"
    exit 1
fi
