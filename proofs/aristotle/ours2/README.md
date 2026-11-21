# Resolved close and payout top-up on the engine's own text (ours2, v3 framing)

## Result so far

- **Nothing new is proved yet.** The v3 Aristotle job (project `b2fa5632`, task `42085a7d`, submitted 2026-11-21 04:53:00Z) has not returned results (see [JOB.md](JOB.md)).
- **F3 (`wide_mul_div_floor_u128`): proved (upstream Aristotle, add1d027; confirmed identical text).**
  Toly's `changed_fns.py`, run unchanged on `src/wide_math.rs` add1d027 → our c38b6b1e: the function's own text is the same, it reaches no changed or new function and names no changed type ([evidence](evidence/f3_changed_fns_add1d027_vs_c38b6b1e.txt)). No credits were spent on F3.
- **No T15 row (the full resolved close) is attempted** in v3. A whole-method proof of the close is out of this budget (ROADMAP: 3 of 32 such properties for about 1,970 credits). What v3 asks for instead, and which rows it does and does not close, is in [LEDGER-MAP.md](LEDGER-MAP.md).
- **None of Toly's 63 kernel proofs carry over to our v16.rs** by his rule: his `changed_fns.py` on d1fa39f6 → c38b6b1e reports 52 kernels needing a new proof and 11 removed ([evidence](evidence/v16_changed_fns_d1fa39f6_vs_c38b6b1e.txt)).

## What v3 asks Aristotle to prove

Following ROADMAP's first way forward (pure kernels over small records), on the crate exactly as committed at c38b6b1e (no file edited; built by [tools/make_upload.sh](tools/make_upload.sh), which checks every blob id):

| Label | Function | Toly row | What a proof would close |
|---|---|---|---|
| PR1.contract, PR1.nopanic | `V16Core::kernel_recompute_resolved_payout_rate` | PR1 | the whole row PR1 |
| P15.nopanic, P15.contract | `resolved_receipt_claimable_against_ledger` | P15 | P15, **given** the ledger-rate invariant (PR1 + PR2–PR4; PR2–PR4 not attempted). Uses F3 as a hypothesis. |
| OURS-K1.contract, .nopanic | `apply_resolved_payout_receipt_payment` | no row | a callee fact of the close and the top-up; closes no row |
| OURS-K2.contract, .nopanic | `capital_and_resolved_payout_to_external_out` + `validate` | no row | the token-flow check: passes exactly when the vault falls by capital paid + resolved paid = payout; closes no row |
| T14.header | `claim_resolved_payout_topup_not_atomic` (its two inner calls opaque) | T14.header | the whole row, if proved (path structure: it ends in `validate_shape()?`) |
| T14.account | same | T14.account | the whole row, if proved |

**A kernel proof is not a T15 proof.** PR1, P15, OURS-K1 and OURS-K2 are facts about functions the close calls; T15.header/scan/account/mirror stay open whatever v3 returns.

Spec: [v3/aristotle/resolved_close_kernels_spec.md](v3/aristotle/resolved_close_kernels_spec.md). Prompt: [v3/stage_v3_resolved_close_kernels.txt](v3/stage_v3_resolved_close_kernels.txt), in the style of Toly's `stage_s2_trade_pilot.txt` and `stage_b5_remaining_kernels.txt`; statements reuse his ledger wording, and F3's text is copied from his `specs/wide_math_spec.md`.

Checked locally (none of it is a proof): the upload builds with `cargo build --release` (overflow checks on); a random test of the four Part 1 statements, 2,000,000 cases each, found no counterexample ([v3/check_part1.rs](v3/check_part1.rs), run by [v3/run_check.sh](v3/run_check.sh) on a scratch copy, never uploaded). Mutating two expected conditions made the test fail, so it does exercise them.

## Lean statements

Toly's `Spec/*.lean` are separation-logic contracts over a Mir model that Aristotle generates from the compiled crate; they cannot be written for new functions without that generated vocabulary. So v3's statements are the spec file (his kernel-spec format and ledger wording). The earlier `statements/Spec/ResolvedClose2*.lean` are a hand model from run 2's framing: not used by v3, nothing shows they match the Rust.

## Earlier work in this directory (superseded framing)

- Run 2 (project `eb31e0d7`, task `c6bd990e`): cancelled after about 2 minutes, **nothing proved**; not resubmitted and not continued. Its files (`project/`, `gen_slice2.py`, `slice_diff2.py`, `SLICE-DIFF*.md`, `check/`, `statements/`, `prompts/`) are kept as a record. Its two findings from running the engine text still stand as test results, not proofs: run 1's invariant is not kept by a close once `clear_fully_diluted_resolved_receipt_if_terminal` is in the code, and run 1's RC6 is false on the engine text.
- Run 1 (`../ours/`, RC1–RC7): Aristotle reported them PROVED about a hand transcription (no proof terms returned, not rechecked); not about c38b6b1e and not of any T15/T14 row (see LEDGER-MAP).
- The independent audit [AUDIT.md](AUDIT.md) (commit a60260e6) is of run 2's framing at 0b7a6a4f, before v3. Its findings A1, A2, A4, A6, A7, A8, A11, A12 are about run 2's `Inv`, slice and statements, which v3 does not use; A3, A5, A9, A10 were corrected in the docs. It also confirms, independently, that F3's text is identical and that v16.rs has no `Cell`/`RefCell`/`UnsafeCell` (which the T14 statement relies on).

## Files

| Path | What |
|---|---|
| `STEER.md`, `STEER-ACK.md` | The change of plan (04:02 GMT) and its acknowledgement. |
| `LEDGER-MAP.md` | Every property, Toly's row id, status, evidence; T15 Ok-path reading. |
| `JOB.md` | Submission, polling, credits. |
| `v3/` | Spec, prompt, random test of Part 1. |
| `tools/changed_fns.py`, `tools/rs_scan.py` | Toly's scripts, byte-identical to `08d4fd09:proofs/`. |
| `tools/make_upload.sh` | Builds the upload from `git show c38b6b1e:…` and checks blob ids. |
| `evidence/` | changed_fns outputs, blob ids, close-path text status. |
