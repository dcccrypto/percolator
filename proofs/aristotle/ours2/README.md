# Resolved close on the engine's own text: second Aristotle run

## Result

**Nothing is proved by run 2.** The job was submitted on 2026-11-21 at 04:10:12 UTC and was cancelled from outside this session about two minutes later, before Aristotle recorded any property. About 1 free credit was used (245 → 244); nothing was bought. It was not resubmitted: see [JOB.md](JOB.md). Run 1's results ([../ours/](../ours/README.md)) stand, and are about a hand transcription.

What exists and was checked locally (none of it is a proof):

- **The slice** ([project/](project/), 6 files, 136,668 bytes). `src/wide_math.rs` is the engine's file. `src/engine_slice.rs` is 885 engine lines in 35 blocks, each byte-identical to its line range of `src/v16.rs` at `c38b6b1e`, plus 319 marked non-engine lines. `gen_slice2.py --check` and `slice_diff2.py` both report every block identical. It compiles with `rustc --edition 2021 --crate-type lib --emit=metadata -C overflow-checks=on` and `cargo check --release`, no warnings.
- **Byte-identical engine text in the slice:** the byte wrappers and codecs, the snapshot capture (`initialize_resolved_payout_ledger_if_needed` and `residual`), receipt creation, the claimable amount, receipt payment, receipt clearing (`clear_fully_diluted_resolved_receipt_if_terminal`), the top-up, `set_account_pnl`/`set_account_pnl_inner`, the token-flow check with `validate`, and the close tail (engine 26643-26702).
- **Still stubbed or absent** ([SLICE-DIFF.md](SLICE-DIFF.md)): `validate_shape` and `validate_with_market` return `Ok(())`; `sync_unattributed_loss_lock` returns `Ok(())`; `burn_account_source_claim_bound_num` returns `Ok(())`, which is the engine's behaviour only for an account with no source-claim bound left; five functions in a branch the path cannot take are `panic!` stubs; the account and market structs keep only the fields the path names; the 58 engine lines before the close tail are absent, so `pnl >= 0` is a precondition.
- **Run 1's differences:** D1 closed, D3 closed, D4 partly closed, D2 not closed. Details and reasons in [SLICE-DIFF.md](SLICE-DIFF.md).
- **Two findings from running the engine text** (`check/`, a random search, about 1.25 million closes; a test, not a proof):
  1. Run 1's invariant is not kept by a close once the engine's receipt clearing is in the code. Example: one account, `vault = 632`, `c_tot = 166`, `insurance = 466`, `snapshot_residual = 104`, receipt face 351 paid 104 at rate 104/351, nothing unreceipted. The close pays 166 and clears the receipt; run 1's `backed` then needs `570 <= 466`. The invariant here carries a list of cleared receipts, and no counterexample to RCs1, RCs2, RCs4, RCs5, RCs7, RC3s, RC6s was found with it.
  2. Run 1's RC6 ("any order gives the same accounts") is false on the engine text: in 1,403 of 249,633 order pairs the receipts differed, always as "cleared in one order, present and fully paid in the other". Totals and the market header were equal in all pairs.

  These are not engine bugs as far as this work shows: no close overpaid in the search. They are corrections to what run 1's statements say about the engine.
- **F3 (`wide_mul_div_floor_u128`): not proved here.** Text check (`tools/wide_math_text_check.sh`): our `src/wide_math.rs` and upstream `add1d027` are identical on our lines 1-1422, 1426-1631 (which contain the function, at 1623-1629) and 1724-2217; the differences are three comment/attribute lines at 1423-1425 and the k-pair functions at 1632-1723, which nothing before line 1636 names. Upstream's ledger records F3 as proved by Aristotle on `add1d027`; we did not re-run or check that proof. Kani here covers the function for 8-bit inputs only (`proof_v17_wide_mul_div_floor_stub_correct`); no new Kani attempt was made (the 16-bit harness is documented in `tests/proofs_v17_fork.rs` as intractable, and we had no cheaper structured idea).
- **Our Lean statements** (`statements/Spec/ResolvedClose2Model.lean`, `ResolvedClose2.lean`) typecheck with Lean 4.34.0 core: 7 theorems, 7 `sorry` warnings. They are a hand model; nothing shows it matches the Rust.

## Files

| Path | What |
|---|---|
| `project/` | The upload. |
| `project/aristotle/resolved_close_spec.md` | Stubs, invariant `Inv(M, A, CL)`, properties RCs1, RCs2, RCs4, RCs5, RCs7, F3, RC3s, RC6s, what Kani covers. |
| `prompts/stage_e2e_rcs_resolved_close_engine_text.txt` | The task text that was sent. |
| `SLICE-DIFF.md` | Status of D1-D4; A: byte comparison of every block; B: every non-engine line; C: diff hunks. Regenerate with `python3 slice_diff2.py --write`. |
| `gen_slice2.py` | Builds the two generated files from `git show c38b6b1e:...`; `--check` compares. |
| `check/` | Random search on the uploaded crate: `cargo run --release -- 400000` (set `CARGO_TARGET_DIR` outside the repo). |
| `tools/frame_scan.py` | Text scan behind the statement about the burn stub. |
| `tools/wide_math_text_check.sh` | The F3 text comparison. |
| `statements/` | Our reference statements in Lean (all `sorry`). |
| `JOB.md` | Submission, cancellation, credits. |

## Links

- Branch: https://github.com/dcccrypto/percolator/tree/agent/J-20261121-ENG-E2E-PROOFS/proofs/aristotle/ours2
- Slice, spec, prompt, statements: https://github.com/dcccrypto/percolator/commit/36005883de7d82b549842b8144db768138157010
- Job submitted: https://github.com/dcccrypto/percolator/commit/a72e1bd6
- Cancellation recorded: https://github.com/dcccrypto/percolator/commit/30a018c8
