# Stage B1 (J-20261120-ENG-PORT-TOLYNETTINGd, final run)

Start: agent/J-20261120-ENG-PORT-TOLYNETTINGc @ 99b1f870. Rule: Toly's code and numbers win where his code
replaces R1; fork-only behaviour (S10 provider_principal, #299 provenance, insurance routing outside netting) is
not bent.

All runs: `node ~/agent-worker/with-lock.mjs reserved -- bash -c 'CARGO_TARGET_DIR=/tmp/jnetting-target cargo test -j 4 --release --test <files> --no-fail-fast'`.

## 1. The 17 tests of EXISTING-FAILS.md on 99b1f870

Run of the 5 affected files on 99b1f870 (`stageB1-head-99b1f870-5files.txt`):

| file | base 696fd81b | Stage A head (EXISTING-FAILS.md) | 99b1f870 |
|---|---|---|---|
| gh292_att_bprime | 11/0 | 10 passed / 1 failed | 11 passed / 0 failed |
| gh292_att_d | 7/0 | 6 / 1 | 7 / 0 |
| gh292_locked_liquidation_recovery | 14/0 | 4 / 10 | 14 / 0 |
| liquidations_edge | 25/0 | 24 / 1 | 25 / 0 |
| multi_asset_portfolio_cross_edge | 24/0 | 23 / 1 (+3 in a 15-test binary, see note) | 24 / 0 |

All 17 pass on 99b1f870, so **no test was rewritten** (0 lines of "old value -> new value"): none of them asserts
an R1 number that Toly's code replaces. They all protect the fork's #299 loss provenance and the insurance
routing that follows from it (`realized_by` = the leg's loss domain). Stage A had dropped that provenance when
7d2e243d replaced the R1 call (`realized_by = None`); 99b1f870 put it back. No real conflict remains among the 17.

Note: EXISTING-FAILS.md lists one binary with 12 passed / 3 failed; that is the Stage A partial default run
and not one of the five files above (the 17 names all map to the five files).

## 2. Ports in this run

| upstream | result | adaptation |
|---|---|---|
| 6666a2ab #252 own-face cancel | ported (`0d7eac45`) | src hunks apply as written except the new fn's anchor (upstream places it before the S1 loser-paid helper from d5485eab, held), placed after `net_slot_face_at_par_not_atomic`. `tests/kf_world/mod.rs` kept as the fork's version (already a superset, from #362). Test file byte-identical to upstream. |
| 487b49c0 #175 loss-domain booking | ported (`736c0239`) | Upstream's remainder call `apply_signed_kf_delta_to_pnl(-remaining, None)` + `book_kf_loss_support_for_source_domain_not_atomic`. Here: `apply_haircut_bounded_close_loss_to_pnl(remaining, Some(source_domain))` (stored rate, as upstream; `Some` only keeps #299 provenance) and the consumed counterparty support is passed as `support_paid` to the fork's existing `reserve_new_capital_backed_loss_for_source_domain_not_atomic`, which books it through `book_realized_loss_backing_for_domain_not_atomic` (the fork's copy of the same destination rule), skipped after the payout snapshot as upstream. This replaces 99b1f870's call, which priced the remainder at R1's netting rate (`KfLossNettingV16`, reading `kf_pending_credit`, which 121b1cfb no longer maintains). Not ported: the spec accounting-table row (no such table here) and the two Kani harnesses (they call `src/v16_kani_api.rs`, removed in this fork). |
| d5485eab, c8a85ce9, 72bfb065, abe4c54c (S1) | held | c3988149 is an ancestor of d5485eab, and d5485eab's test calls `resolved_counterparty_backing_surplus` / `withdraw_resolved_counterparty_backing_surplus_not_atomic` (2 call sites), which only c3988149 adds (0 definitions in `src/v16.rs`). So they do not apply without c3988149. |
| 1683b11d and prerequisites (#250 c7ae2256+596b3d98, #206 c85b148b, c3988149, #172 adf5cb64) | held | as instructed. |

## 3. Counts after the ports (HEAD)

RESULTS_PLACEHOLDER
