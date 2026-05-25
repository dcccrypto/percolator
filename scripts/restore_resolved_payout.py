#!/usr/bin/env python3
"""Replace the 8 resolved_payout split proofs + helper (lines 8716-8806, 1-based)
with the original single proof recovered from git b1ee2a1. The splits all time
out, so reverting to the single original (also a known timeout) keeps the suite
honest instead of carrying 8 redundant timeouts."""

PATH = 'tests/proofs_v16.rs'
START = 8716   # 'fn resolved_payout_readiness_blocked_by(' (1-based)
END = 8806     # closing '}' of pending_domain_loss_blocker proof (1-based, inclusive)

ORIGINAL = '''#[kani::proof]
#[kani::unwind(130)]
#[kani::solver(cadical)]
fn proof_v16_resolved_payout_readiness_uses_exact_counters_and_bounds() {
    let blocker: u8 = kani::any();
    kani::assume(blocker < 8);
    let (market, account_id, owner) = concrete_ids();
    let mut group = MarketGroupV16::new(market, V16Config::public_user_fund(1, 0, 1)).unwrap();
    let mut account =
        PortfolioAccountV16::empty(ProvenanceHeaderV16::new(market, account_id, owner));
    group.vault = 10;
    account.pnl = 10;
    group.pnl_pos_tot = 10;
    set_junior_bound(&mut group, 10);
    group.resolve_market_not_atomic(1).unwrap();
    match blocker {
        0 => group.b_stale_account_count = 1,
        1 => group.stale_certificate_count = 1,
        2 => group.negative_pnl_account_count = 1,
        3 => group.assets[0].stored_pos_count_long = 1,
        4 => group.assets[0].stored_pos_count_short = 1,
        5 => group.assets[0].stale_account_count_long = 1,
        6 => group.assets[0].stale_account_count_short = 1,
        _ => group.pending_domain_loss_barriers[1] = 1,
    }

    let vault_before = group.vault;
    let pnl_pos_before = group.pnl_pos_tot;
    let bound_before = group.pnl_pos_bound_tot;
    let account_pnl_before = account.pnl;
    let outcome = group.close_resolved_account_not_atomic(&mut account, 0);

    kani::cover!(blocker == 0, "v16 resolved readiness B-stale blocker");
    kani::cover!(
        blocker == 6,
        "v16 resolved readiness stale short-count blocker"
    );
    kani::cover!(
        blocker == 7,
        "v16 resolved readiness pending-domain-loss barrier blocker"
    );
    assert_eq!(outcome, Ok(ResolvedCloseOutcomeV16::ProgressOnly));
    assert_eq!(group.vault, vault_before);
    assert_eq!(group.pnl_pos_tot, pnl_pos_before);
    assert_eq!(group.pnl_pos_bound_tot, bound_before);
    assert_eq!(account.pnl, account_pnl_before);
    assert!(!group.payout_snapshot_captured);
}
'''

lines = open(PATH).read().split('\n')
# sanity check anchors
assert lines[START-1].startswith('fn resolved_payout_readiness_blocked_by('), repr(lines[START-1])
assert lines[END-1] == '}', repr(lines[END-1])
new = lines[:START-1] + ORIGINAL.split('\n')[:-1] + lines[END:]
open(PATH, 'w').write('\n'.join(new))
print(f'replaced lines {START}-{END} ({END-START+1} lines) with original ({ORIGINAL.count(chr(10))} lines)')
