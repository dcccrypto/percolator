# Ledger map: our resolved-close work against Toly's `proof_ledger.tsv` (aeyakovenko/percolator @ 08d4fd09)

Engine text: ours `c38b6b1e` (`src/v16.rs` blob `7d3edfe8`, `src/wide_math.rs` blob `97c6b16d`).
Row ids are his (`proofs/proof_ledger.tsv` at `08d4fd09`). Where a property has no row, the table
says "no row" and the label is ours (`OURS-…`); no id was invented in his scheme.

Status words: **proved** (a proof exists that covers our text, with the evidence named),
**submitted** (in the v3 Aristotle job, no result yet), **assumed** (used as a hypothesis, not proved
here), **not attempted**. Our commit SHA is the commit that holds the statement or evidence.

Statuses are as of the commit that last changed this file; results of the v3 job are filled in
when they exist (see [JOB.md](JOB.md)).

## The rows on the resolved-close path

| Toly row | Function (our line) | Status | Evidence | Our commit |
|---|---|---|---|---|
| F3 | `wide_mul_div_floor_u128` (wide_math.rs:1623) | **proved (upstream Aristotle, add1d027; confirmed identical text)** | His ledger: `Spec.WideMath.F3_wide_mul_div_floor_u128 (engine add1d027…)`. His `changed_fns.py` (copied verbatim, `tools/`) on wide_math.rs add1d027 → c38b6b1e: F3 own text `same`, reaches no changed/new function, names no changed type, "kernels that need a new proof: 0" ([evidence/f3_changed_fns_add1d027_vs_c38b6b1e.txt](evidence/f3_changed_fns_add1d027_vs_c38b6b1e.txt)). His wide_math.rs is one blob (`e57bba6b`) at add1d027, c030aa40, d1fa39f6 and 08d4fd09, so the same holds against each. The file differs from ours only in `floor_div_signed_conservative_i128` (comment/attribute lines), `wide_signed_mul_div_floor_from_k_pair` and a new `wide_signed_mul_div_floor_with_carry_from_k_pair`. No credits spent on F3. | see README |
| PR1 | `V16Core::kernel_recompute_resolved_payout_rate` (v16.rs:2688) | **submitted** (v3 Part 1, `PR1.contract`, `PR1.nopanic`) | Our text differs from his (changed_fns: `changed`), so nothing of his carries over; the row was open there anyway. Random test 2,000,000 cases, no counterexample (`v3/check_part1.rs`; a test, not a proof). | see README |
| P15 | `resolved_receipt_claimable_against_ledger` (v16.rs:25252; F3 call at 25263) | **submitted** (v3 Part 1, `P15.nopanic`, `P15.contract`) | Own text same as his d1fa39f6. The statement is conditional on H_rate (`1 <= den`, `num <= den`), which is the row's own "class ledger" reason. If proved, P15 holds **given** the ledger-rate invariant; that invariant is PR1 (submitted) plus PR2–PR4 (not attempted). F3 is used as a hypothesis (proved upstream, above). Random test 2,000,000 cases, no counterexample. | see README |
| PR2, PR3, PR4 | writers of `header.resolved_payout_ledger`; `payout_snapshot_captured`; receipts | **not attempted** | Whole-program write-set statements, not a kernel. Until they are proved, the ledger-rate invariant that P15 needs is assumed at the close and top-up call sites. | — |
| P07 | `kernel_resolved_receipt_can_still_gain` | **not applicable to our text** | The function does not exist at c38b6b1e (changed_fns: `removed`). | — |
| T14.header | `claim_resolved_payout_topup_not_atomic` (v16.rs:26535) | **submitted** (v3 Part 2) | Own text same as his d1fa39f6. Body: `preflight?; core?; validate_shape()?; account.validate_with_market(&self.as_view())?; Ok(payout)` (v16.rs:26539–26543). Both validators take `&self`; the ViewMut versions delegate to the read-only view (v16.rs:6865, 10378). Statement is the row's own. Asked at `T = ()` (as in his trade pilot): the methods sit in `impl<'a, T> MarketGroupV16ViewMut<'a, T>` with the program's wrapper type as `T`, and the engine crate itself never instantiates the top-up, so a proof at `T = ()` is about that instantiation; that the body does not depend on `T` is a reading, not proved. | see README |
| T14.account | same | **submitted** (v3 Part 2) | Same path structure; the last call is `validate_with_market` on the final state. | see README |
| T14.scan | same | **not attempted** | Needs contracts of the core's writes (vault, receipt) against `validate_shape_full_audit_scan`. | — |
| T14.mirror | same | **not attempted** | Needs the core's write set against part M. OURS-K1 and OURS-K2 are the two pure facts the core uses (receipt payment, vault falls by exactly the payout); they do not close the row. | — |
| T15.header | `close_resolved_account_not_atomic` (v16.rs:26580) | **not attempted** | Stateful, ~120-line body with about 20 calls; ROADMAP records 3 of 32 such properties for ~1,970 credits, beyond the ~124 credits above the submit floor. Source reading below (not a proof). | — |
| T15.scan | same | **not attempted** | as above | — |
| T15.account | same | **not attempted** | as above; the reading below finds one Ok path that ends in `validate_shape` without `validate_with_market` | — |
| T15.mirror | same | **not attempted** | as above. PR1, P15, OURS-K1 and OURS-K2 are callee facts of the close's tail (rate, claimable amount, receipt payment, token-flow check). **A proof of them is not a proof of any T15 row.** | — |
| T37.header/scan/account/mirror | `full_account_refresh_not_atomic` | **not attempted** | Plan item 3 ("only if credits allow" after the close job). Decided after the v3 job ends. | — |
| T31, TA.*, TF.*, TB.*, TC* | the trade | **not attempted** | Out of this budget by plan. | — |

## Properties with no row in his ledger

| Our label | Function (our line) | Status | Why it is on the path | Our commit |
|---|---|---|---|---|
| OURS-K1 | `apply_resolved_payout_receipt_payment` (v16.rs:7181) | **submitted** (v3 Part 1) | Called by the close (v16.rs:26665) and the top-up core (v16.rs:26505). His nearest kernel, K13 `kernel_resolved_payout_step`, does not exist in our text. Random test 2,000,000 cases, no counterexample. | see README |
| OURS-K2 | `TokenValueFlowProofV16::capital_and_resolved_payout_to_external_out` (v16.rs:7860) + `validate` (v16.rs:7896) | **submitted** (v3 Part 1) | The close's last token check (v16.rs:26692–26699) and the top-up core's (v16.rs:26516–26523). Says the check passes exactly when the vault falls by `capital_paid + resolved_paid` and that equals the payout. His K49 (`debit`) is a callee; our `debit` text differs from his. Random test 2,000,000 cases, no counterexample. | see README |

## Earlier runs

| Run | Labels | Toly row | Status for the engine |
|---|---|---|---|
| Run 1 (`../ours/`) | RC1–RC7 | **no row** | Aristotle reported PROVED (no proof terms returned, not rechecked locally) about a **hand transcription** of the close path ([../ours/SLICE-DIFF.md](../ours/SLICE-DIFF.md)), not about `c38b6b1e`. They are proofs of the transcription, **not** of T15 or T14. RC2/RC4/RC5/RC7 rest on an invariant that run 2's random search shows is not kept by the engine's close; RC6 is false on the engine text; RC3 does not carry over ([README](README.md) of run 2, section Result). |
| Run 2 (project eb31e0d7, task c6bd990e) | RCs1, RCs2, RCs4, RCs5, RCs7, RC3s, RC6s, F3 | **no row** (F3 is F3) | Cancelled after ~2 minutes; **nothing proved**. Not resubmitted and not continued. Its framing (`project/`, `gen_slice2.py`) is superseded by v3. |

## Source reading of T15's Ok paths on our text (reading, not a proof)

`close_resolved_account_not_atomic`, v16.rs:26580–26702. Each `Ok` return and what precedes it:

| Line | Return | Check before it, with no write in between |
|---|---|---|
| 26594 | `ProgressOnly` after a source preparation | Inside the callee: each `Ok(Some(_))` of `prepare_one_source_domain_for_resolved_close_not_atomic` follows `validate_shape()?` and `validate_with_market(..)?` (v16.rs:26412–26414, 26438–26440, 26450–26452); between the callee's return and 26594 only `account_has_source_liens(&account.as_view())` runs. |
| 26605 | `ProgressOnly` after a lapsed-backing expiry | `validate_shape()?` and `validate_with_market(..)?` (26603–26604) |
| 26614 | `ProgressOnly` after a B chunk | `validate_shape()?` only (26613). **No `validate_with_market`**: relevant to T15.account. |
| 26631 | `ProgressOnly` after fee sync, negative-PnL settlement, bankruptcy and leg detach | **none** (the ledger's "class real") |
| 26634 | `ProgressOnly` when positive payout is not ready | **none** (same writes before it as 26631) |
| 26641 | `ProgressOnly` after one source realization | `validate_shape()?` and `validate_with_market(..)?` (26639–26640) |
| 26702 | `Closed { payout }` | `validate_shape()?` and `validate_with_market(..)?` (26700–26701) |

This agrees with his T15.header reading on d1fa39f6 (the two unchecked `ProgressOnly` returns after
`settle_negative_pnl_from_principal_not_atomic` / `settle_resolved_bankruptcy_negative_pnl` /
`detach_solvent_active_legs_for_resolved_close`). It is a reading of the text, not a proof.
