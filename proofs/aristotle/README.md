# proofs/aristotle — end-to-end invariants (prepared, NOT submitted)

Layout follows upstream aeyakovenko/percolator `proofs/` (statements under
`aristotle/statements/Spec/`, Lean toolchain pinned in `aristotle/statements/lean-toolchain`,
task texts as `stage_*.txt`, a TSV ledger with upstream's columns
`id layer target status evidence method statement`). Upstream keeps prompts in `proofs/prompts/`
and the ledger in `proofs/proof_ledger.tsv`; here both live under `proofs/aristotle/` so this
batch is self-contained and does not collide with upstream files on a later merge.

| File | What |
|---|---|
| `statements/Spec/E2EModel.lean` | Abstract N-account model and theorems A1..A7 (conservation), C1..C2 (senior solvency), D1..D2 (exact resolved payout, any order after the snapshot). All `sorry`. |
| `statements/lean-toolchain` | `leanprover/lean4:v4.34.0` (copied from upstream). |
| `prompts/stage_e2e_a_conservation.txt` | Prove A1..A7; refine deposit/withdraw/trade/refresh/liquidate/close to the model steps. |
| `prompts/stage_e2e_b_solvent_not_liquidated.txt` | The cert the liquidation gate reads equals the spec deficit for every portfolio. |
| `prompts/stage_e2e_c_senior_solvency.txt` | Prove C1..C2; every value-writing entry point preserves `c_tot + insurance <= vault`. |
| `prompts/stage_e2e_d_resolved_payout.txt` | Prove D1..D2; refine `close_resolved_account_not_atomic` and the claimable computation. |
| `ledger.tsv` | One row per piece, status `stated, not submitted` / `prompt ready, not submitted`. |

Nothing here has been typechecked (no Lean toolchain on the preparing machine) or submitted
(no Aristotle account or key). Each prompt says what Kani already proves so Aristotle targets
only the unbounded gap.
