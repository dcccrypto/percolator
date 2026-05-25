#!/usr/bin/env python3
"""Reorder kani_isolated_full/proofs.txt: likely-light proofs first, heavy engine
families last, so the isolated audit reclaims fast passes before grinding the
genuinely-heavy liquidation/crank/refresh cluster."""
import sys

PATH = 'kani_isolated_full/proofs.txt'
HEAVY_KEYS = [
    'permissionless', 'liquidat', 'refresh', 'accrual', 'crank', 'recovery',
    '_kf_', 'positive_kf', 'quantity_adl', 'adl_', 'rebalance', 'settlement',
    'settle', 'repeated_account_b', 'same_epoch', 'sequential_kf', 'side_reset',
    'sign_flip', 'partial_liquidation', 'worst_case', 'price_accrual',
    'released_pnl', 'source_backed_conversion', 'stale_open_close',
    'pending_obligation', 'per_asset_slot', 'released',
]

proofs = [l.strip() for l in open(PATH) if l.strip()]
def is_heavy(p):
    return any(k in p for k in HEAVY_KEYS)

light = [p for p in proofs if not is_heavy(p)]
heavy = [p for p in proofs if is_heavy(p)]
open(PATH, 'w').write('\n'.join(light + heavy) + '\n')
print(f'reordered: {len(light)} light-first, {len(heavy)} heavy-last')
print('--- first 15 (light) ---')
for p in light[:15]:
    print('  ', p)
