# Aristotle job (first run, free-tier credits only)

## Run

- Submitted 2026-11-21 01:58:17 UTC, started 01:58:18Z, last update 03:24:48Z (wall time 1 h 26 m 30 s).
- Project id `576ace41-da17-48fa-9b7f-e4b277eded5a`, task `1222bad6-e17d-4d90-81e2-9a4ba0131dd8`.
- Name: `percolator-resolved-close-rc`. Web: https://aristotle.harmonic.fun/projects/576ace41-da17-48fa-9b7f-e4b277eded5a
- Upload: 6 files (21.3 KB) from `project/`; prompt `prompts/stage_e2e_rc_resolved_close_no_overpay.txt`.
- Command: `aristotle submit --prompt-file ../prompts/stage_e2e_rc_resolved_close_no_overpay.txt --project-dir . --name percolator-resolved-close-rc --json --no-questions`
- Status at 02:48 UTC: IN_PROGRESS, 7 properties all in progress, none resolved.
- Status at 03:46 UTC (confirmed by download at 03:48 UTC): `COMPLETE`, `all_properties_resolved`. Task status COMPLETE. Project `latest_run_status` COMPLETE. Counts: proved 7, refuted 0, in_progress 0, unresolved 0, unchecked 0.

## Credits

- Started with 500 free credits, 0 purchased.
- Used: 254.68 credits (2,546,818 small credits). Free left: 245.32 (2,453,182 small credits). Purchased: 0.
- Observed at 03:48:26Z. Next expiry of remaining free credits: 2026-12-21T01:00:00Z.
- Nothing was bought. The run ended with credits left; it did not hit OUT_OF_BUDGET.

## Verdicts (all PROVED, none refuted)

Aristotle's summary (`result/ARISTOTLE_SUMMARY.md`): Inv was strong enough as written; no statement was weakened. Each result is for slices and orders of any length. Sums are mathematical integers. "Does not panic" means no execution ends in a panic; termination is not claimed.

| Property | Aristotle name | Status | Last update (UTC) |
|---|---|---|---|
| RC1 | `Spec.rc1_spec` | PROVED | 03:14:12 |
| RC2 | `close_resolved_payout_tail.rc2_one_close_keeps_inv` | PROVED | 03:17:32 |
| RC3 | `close_resolved_payout_tail.rc3_no_err_no_panic` | PROVED | 03:17:33 |
| RC4 | `close_all.rc4_any_order_keeps_inv` | PROVED | 03:17:33 |
| RC5 | `close_all.rc5_full_order_pays_out` | PROVED | 03:17:33 |
| RC6 | `close_all.rc6_order_independent` | PROVED | 03:17:33 |
| RC7 | `claim_resolved_payout_topup_core.rc7_topup_keeps_inv` | PROVED | 03:14:12 |

- RC1 is a public Lean statement in `result/Spec/RC1.lean`. RC2–RC7 are stated in Rust prose in Aristotle's properties table, not as public Lean formulas (the public interface cannot name the `&mut` struct types). Their proofs are about the generated code, like RC1's.
- Aristotle did not return proof terms ("Proof source is not included in the download").

## Aristotle's own caveats (from the summary)

- RC2–RC7 assume the contract of `wide_mul_div_floor_u128` as a hypothesis. For RC1 it is a premise of `rc1_spec`, stated in the formula.
- The body of `wide_mul_div_floor_u128` and the rest of `src/wide_math.rs` are unverified.
- Also assumed: a standard Rust environment in which every object is smaller than 2^56 bytes (for `close_all`, slice byte sizes below 2^63).

## Our caveats (not Aristotle's)

- `src/resolved_payout/close_path.rs` is a hand transcription over plain values, not engine code. A proof about it is a proof about the slice. The four differences from `src/v16.rs` are listed in `project/aristotle/resolved_close_spec.md` (DIFF 1–4). Ledger row `RC.refine` remains NOT PREPARED: we have not shown that the transcription matches the engine entry points, nor that the engine reaches the payout tail only in a state satisfying Inv.
- The two engine operations that change the rate after the snapshot (`refine_resolved_unreceipted_bound_not_atomic`, `kernel_credit_post_snapshot_residual`) are not in the slice.
- These results were not independently rechecked with a local Lean kernel; only Aristotle's reported verdicts and the downloaded statements are on disk.

## Downloaded files

See `result/` (archive plus extracted Lean/docs; the six uploaded Rust/spec files already live in `project/`). Manifest: `result/MANIFEST.txt`.
