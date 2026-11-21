# Independent audit of `proofs/aristotle/ours2/` (and the run-1 claims in `ours/`)

Auditor: a separate session. It did not build this work and did not see the builder's conversation. It ran no cargo, Kani or Lean. It changed nothing but this file.

- Audited tree: branch `agent/J-20261121-ENG-E2E-PROOFS` at `0b7a6a4f`. The upload `ours2/project/` is unchanged since `1d52f624` (the commit named in `AUDIT-READY` and `JOB.md`); `git diff 1d52f624 HEAD -- proofs/aristotle/ours2/project` is empty.
- Ground truth: `git show c38b6b1e:src/v16.rs` (blob `7d3edfe8…`, 31,045 lines), `git show c38b6b1e:src/wide_math.rs` (blob `97c6b16d…`), `git show add1d027:src/wide_math.rs`, `git show upstream/master:proofs/proof_ledger.tsv`.
- Method: I wrote my own scripts in `/tmp` (not the builder's `gen_slice2.py` or `slice_diff2.py`). One matches every slice line to maximal runs of engine lines. The other parses each `// ---- src/v16.rs:A-B ----` block, compares it with engine lines A..B, and lists every line outside blocks and markers. I read every non-engine line by hand.
- `ours2/result/` does not exist. Run 2 was cancelled before any property was recorded (`ours2/JOB.md`), so **run 2 has proved nothing**. Sections 2 and 5 are about what the statements would establish if they were proved.

## Findings

| id | severity | where | evidence | suggested fix |
|---|---|---|---|---|
| A1 | material | `ours2/project/aristotle/resolved_close_spec.md:100-107` (`Inv`), `statements/Spec/ResolvedClose2Model.lean:333-345` | **`Inv` does not stay true under the engine's own ledger writers, so it cannot be the invariant the engine keeps without strengthening.** `kernel_credit_post_snapshot_residual` (v16.rs:2712-2725), called through `credit_post_snapshot_residual_not_atomic` (11626-11638) after the snapshot (11613-11621, 16225, 16297), raises `snapshot_residual` by `released`. It leaves `vault`, `c_tot` and `insurance` alone. Inv's `backed` (`c_tot + insurance + (snapshot_residual - P) <= vault`) has no term that shrinks by `released`. Concrete Inv state: captured, vault 100, c_tot 0, insurance 0, earnings 0, `source_fresh_backing_total_num = 60*BOUND_SCALE`, snapshot_residual 100, exact = unreceipted = 0, rate 1/1, no receipts, CL empty. It satisfies every clause, and a release of 60 makes `backed` read `160 <= 100`. The engine likely never reaches such a state, because at capture the snapshot already nets out earnings and fresh backing (10602-10611, 25181). But showing that needs a stronger clause: `c_tot + insurance + backing_provider_earnings_total + source_fresh_backing_total_num/BOUND_SCALE + (snapshot_residual - P) <= vault`. The same gap applies to pre-tail value moves from fresh backing into capital (realize/conversion path, 26209-26232). | Strengthen `backed` with the two senior terms the engine's `residual()` and `validate_header_aggregate_totals` (10069-10089) use. Say in the docs that today's `Inv` is not inductive over the engine's other writers. |
| A2 | material | spec §"Not in the crate", `ours2/README.md:11`, prompt line 4 ("for any number of accounts and any order of closes ...") | **RCs4, RCs5 and RC6s compose only back-to-back tails.** In the engine, every close first runs engine lines 26585-26642, and other instructions run between closes. Those lines are absent, and they write the ledger and the money totals: `refine_resolved_unreceipted_bound_not_atomic` via realize (26283-26294, 26549-26578), the post-snapshot credit via lapsed-backing expiry (26597-26606 → 11600-11622), fee sync (26616-26620), principal and bankruptcy settlement (26621-26624), source-claim conversion (26209-26232). So the plain claim "any number of accounts, any order" holds for the engine only together with a lemma that nobody has stated or proved: each of those steps keeps `Inv`, and `Inv` holds at resolution. Also, with `mirror` (`c_tot` = sum of capital over `A`), `A` must contain every account holding capital, and RCs4 assumes `pnl >= 0` for all of them. A state where an unclosed account still has negative pnl and capital is therefore outside RCs4. | State RCs2 (one tail keeps `Inv`) as the composable result. Name the missing lemma per absent step. Map it to upstream `T15.mirror`/`T15.header` (both `open` in `upstream/master:proofs/proof_ledger.tsv`). Do not word RCs4 as covering engine close sequences. |
| A3 | material | `resolved_close_spec.md:197-203`, `prompts/stage_e2e_rcs_resolved_close_engine_text.txt:6`, `ours/README.md:5` | **The Kani coverage is overstated.** The spec cites "the three harnesses of `tests/proofs_e2e_resolved_close.rs` (one asset, two accounts, 8-bit amounts)" as coverage, and the prompt tells Aristotle "Kani covers this path for one or two accounts with bounded amounts". That file is not at `c38b6b1e` (it is on this branch). Its four `#[kani::proof]` harnesses (lines 141, 155, 219, 256) are all marked "Did not finish on the 2026-11-21 machine" and sit behind `--features e2e-long` (commits `5a35c176` 02:47 and `e94ea592` 02:58, before the spec commit `36005883` at 04:09). The four `tests/proofs_v16.rs` harnesses the spec names exist (`kani-list.json`), but this branch records no run result for them, and this work did not run them. | Reword: "Kani harnesses exist for parts of this path (receipt payment, claimable monotonicity, top-up, two receipts; `tests/proofs_v16.rs`); no result is recorded here; the whole-path e2e close harnesses did not finish." |
| A4 | material | `resolved_close_spec.md:107`, RCs4 "`c_tot + insurance <= vault`" | **The properties only protect capital and insurance.** The engine treats `backing_provider_earnings_total` and fresh counterparty backing as senior claims too (comment at 10596-10601; check at 10069-10089). The final `validate_shape()` that would enforce this is a STUB-OK (`engine_slice.rs:739-741`). So no statement, proved or not, says a close leaves those two amounts covered by the vault. | Add "`c_tot + insurance + earnings + fresh/BOUND_SCALE <= vault` is kept" to RCs2/RCs4 (this follows from the A1 strengthening). |
| A5 | minor | `ours/README.md:5`, `ours/JOB.md` "Verdicts (all PROVED …)" | "seven properties … were proved" repeats Aristotle's reported verdict. `ours/result/MANIFEST.txt` says no proof terms were returned, and nothing was rechecked with a Lean kernel (`ours/JOB.md` says so further down). | "Aristotle reported RC1-RC7 PROVED about the transcription; it returned no proof terms; not rechecked locally." |
| A6 | minor | `ours2/README.md:13` ("Two findings from running the engine text") | The search ran the slice (engine text plus stubs plus harness) from random `Inv` states with small values (`check/src/main.rs:24-27`, 137-146), not from engine-reachable states. I checked both findings by hand on the slice: the spec's 632/166/466 example breaks run 1's `backed`, and a 2-account example breaks run 1's RC6 (faces 1 and 1, residual 1, rate 1/2: whichever account closes last is cleared). | "Found by running the slice's engine text from random states satisfying `Inv`." |
| A7 | minor | `ours2/README.md:11`, `SLICE-DIFF.md` D2 | "the 58 engine lines before the close tail are absent, so `pnl >= 0` is a precondition" undersells them. Those lines also write the ledger, the snapshot, vault/c_tot/insurance and the account (see A2). They are also where the engine guarantees `pnl > 0 ⇒ resolved_positive_payout_ready()` (26633-26634) and the source-claim condition (26636-26642). SLICE-DIFF D2(b) says part of this; the README does not. | List the written fields in the README line. |
| A8 | minor (an understatement) | `engine_slice.rs:718-724`, `SLICE-DIFF.md` D4(c) | STUB-PATH is stated more narrowly than the engine allows. By reading, the engine reaches the tail with no prior receipt only if `realize_one_source_domain_for_resolved_close_not_atomic` returned false (no occupied domain with a nonzero bound, 26184-26203) or `account_has_source_claims` is false (26636-26642). Both scans stop at the same sparse tail, and `is_occupied` includes `source_claim_bound_num != 0` (28149-28150). So the source-claim sum is 0, which is the stub's path. Receipts are created only in the tail (26647, the sole caller of 25203), and the same call then sets pnl to 0. This is an argument, not a proof. | Optionally add it to D4(c) as an argument. Keep the caveat. |
| A9 | minor | `ours2/JOB.md:10-11` vs `ours2/STEER-ACK.md` | JOB.md says the STEER files "were not acted on … Waiting for the user's decision". STEER-ACK.md (04:14:54) says "accepted". The record contradicts itself. | Update JOB.md. |
| A10 | minor | `ours/ledger.tsv` | Every row still reads `PREPARED, NOT SUBMITTED`, though run 1 completed (`ours/JOB.md`). `ours/README.md:3` says the folder "is not changed apart from this note", but README and JOB were edited in later commits (`d84c3434`, `30a018c8`, `84f46ce0`). | Update the ledger statuses to "Aristotle reported PROVED (transcription)" and drop the "not changed" sentence. |
| A11 | minor | `SLICE-DIFF.md:5` "five engine functions are stubbed" | The slice stubs 9 engine functions: 4 behavioural (`validate_shape`, `validate_with_market`, `sync_unattributed_loss_lock`, `burn_account_source_claim_bound_num`) and 5 STUB-DEAD. It also rewrites `as_view` without `markets` (`engine_slice.rs:732-736`). README line 11 lists them correctly. | Fix the count. |
| A12 | minor | `statements/Spec/ResolvedClose2Model.lean:347-351` | `Fits` bounds neither `pnl` (an `Int`, not i128), nor `cTot`, nor the ledger and receipt fields, so the hand model admits values Rust cannot hold. `Option` merges `Err` with panic, so the spec's "no execution panics" clauses (RCs2, RCs7) have no Lean counterpart. These files are only a reader's reference (Aristotle writes its own Lean), and the header says so. | Add the bounds, or note the omission. |
| A13 | minor | Ok-only statements (RCs2/4/5/7) | The tail writes `vault`/`c_tot` (`engine_slice.rs:1280-1293`) before calls that can still fail (`set_account_pnl`, flow `validate`), and `create_resolved_payout_receipt_if_needed` writes `payout_halted = true` and then returns `Err` (v16.rs:25218-25222). Reading Ok-only results as "never pays more" assumes the caller reverts every write on `Err`. The slice cannot show that. | State that assumption once. |

## 1. Slice vs engine

- **`src/wide_math.rs`.** Checked with `cmp` against `c38b6b1e:src/wide_math.rs`: no difference.
- **`src/engine_slice.rs`.** 35 blocks, 885 lines. Each block matches its stated range of `c38b6b1e:src/v16.rs` exactly (0 mismatches). Outside the blocks there are only the 5 header comment lines, blank lines, and text between `>>> NOT ENGINE` / `<<< NOT ENGINE` markers (233 inner lines, 87 marker lines). So the claim "only the listed lines are non-engine text": checked, found no difference.
- **Non-engine text, all of it**:
  - the two `use` lines;
  - `impl … {` / `}` wrappers. The engine has `impl<'a, T> MarketGroupV16ViewMut<'a, T>` (10377); the slice drops `T`. Every copied method range lies inside that one engine impl (10377 through 26702, no column-0 `}` in between), inside `impl V16Core` (1164), or inside `impl<'a> PortfolioV16View<'a>` (6474), as the wrappers say;
  - the 5 `#[repr(C)] #[derive(...)]` lines without the bytemuck derives;
  - stub structs and the views;
  - 9 stub functions and `as_view`;
  - the 4-line wrapper signature of the tail. It drops the `fee_rate_per_slot` parameter, which only the absent lines 26617-26621 use;
  - `pub mod harness;`.
- **Close tail.** `engine_slice.rs:1250-1309` equals v16.rs:26643-26702 exactly. Checked, found no difference.
- **Stub header structs.**
  - `MarketGroupV16HeaderAccount` (`engine_slice.rs:426-448`): the 21 fields have the same names, types and relative order as v16.rs:9447-9494 (`V16PodU128`, `V16PodU64`, `u8`, `ResolvedPayoutLedgerV16Account`).
  - `PortfolioAccountV16Account` (460-467): its 6 fields match 28182-28219.
  - `HealthCertV16Account.valid: u8` matches 27934. `PortfolioSourceDomainV16Account.source_claim_bound_num: V16PodU128` matches 28129. `SourceCreditStateV16`'s two `u128` fields match 6021-6022.
  - The views match 6226-6234 minus `markets`. `PortfolioV16View`/`PortfolioV16ViewMut` are engine text (6298-6304).
  - Checked field types, found no difference. The dropped fields are not named by any copied line (it compiles per the builder; not rerun by me).
- **Constants.** `BOUND_SCALE = 1_000_000_000_000` (`lib.rs:21`) equals engine `src/lib.rs:25`. `V16_TOKEN_VALUE_CLASS_COUNT = 17` and the class enum are engine text. The release profile has `overflow-checks = true`, as in the engine's `Cargo.toml`. Checked, found no difference. `PORTFOLIO_SOURCE_DOMAIN_CAP` is not used by slice code.
- **Stubs can only turn `Err` into `Ok`.**
  - Engine `validate_shape` (10378-10380 → 9996…) and `validate_with_market` (6865-6867 → 6475…) take `&self`. There is no interior mutability in v16.rs (no `Cell`/`RefCell`/`UnsafeCell`). They are read-only and return `Result`, so the `Ok(())` stub only removes `Err` exits. They run last in the tail.
  - Engine `sync_unattributed_loss_lock` (15318-15339) writes only `liquidation_lock` and `loss_provenance`. Neither field is in the slice, and nothing in the slice reads them.
  - The burn stub equals the engine path for `burn_num == 0` or a source-claim sum of 0 (12893-12900); see A8 for why the tail is on that path.
  - The STUB-DEAD bodies sit only under `if let Some(domain) = increase_domain`. `set_account_pnl` passes `None` (15223), and the tail calls it with 0, so they are unreachable.
  - Checked, found no hidden value-moving effect.
- **Anything that makes a property easier than in the engine.** I found no added guard, no removed branch inside copied text, no narrower type, and no extra assumption in the slice code. Run 1's added `pnl < 0 ⇒ Err` guard is gone. What does make the slice easier is what it leaves out: the pre-tail lines and the `Inv` precondition (A1, A2). The harness (`harness.rs:34-53`) stops at the first `Err`. Engine runs where one close fails and others proceed are covered only as `close_all` over the sub-order of successful closes.

## 2. Statements vs intent

- **RCs1** (spec, Lean `RCs1_…`): `G ≤ snapshot_residual` and `P ≤ G` under `Inv`. The direction is right. The rate formula is `num = min(residual·BS, exact+unreceipted)`, `den = exact+unreceipted`, `gross = floor(face·num/den)`, and with `cover` this gives `Σgross ≤ (exact/BS)·num/den ≤ residual`. Checked for off-by-one and direction, found no difference.
- **RCs2/RCs7**: about `Ok` runs plus a no-panic clause. `payout + P = capital + P'` and `vault' + payout = vault` are the right form of "the cap never cuts, no capital zeroed unpaid". I checked by hand that the `min(.., vault)` cap cannot bind under `Inv`: `capital ≤ c_tot` and `claimable ≤ residual − P`.
- **RCs4/RCs5**: "any number, any order" is quantified correctly over `order : List Nat` and `A` of any length. The limits are in A2.
- **RC6s**: restated correctly. I reproduced the reason by hand (A6).
- **Vacuity.** The hypotheses are jointly satisfiable. The spec's own example state satisfies `Inv`, and so does a state with no snapshot, `c_tot + insurance ≤ vault` and no receipts. I found no contradictory clause.
- **Plausibility as an engine invariant.**
  - `fresh`: no receipt before capture. True, because `payout_snapshot_captured` is written only at 25184 (to 1), and receipts are created only after `initialize_resolved_payout_ledger_if_needed`.
  - `rate`: every ledger writer ends in `kernel_recompute_resolved_payout_rate` (25169-25175, 25200, 25244, 26566, 2724).
  - `live`: `payout_halted` is set only on an `Err` exit (25218-25222).
  - `refine_resolved_unreceipted_bound_not_atomic` keeps `Inv` by reading: it only raises the rate (`fraction_ge` check, 26567-26575).
  - `kernel_credit_post_snapshot_residual` does not keep `backed` (A1).
- **Does anything prove the engine keeps `Inv`?** No. Nothing in `ours/` or `ours2/` proves that the engine reaches the tail in an `Inv` state, or that any non-tail engine operation keeps `Inv`. The docs say so (`ours/README.md`, SLICE-DIFF D2), and A1 shows `Inv` must be strengthened before it can be shown.
- **Run 1** (`ours/result/Spec/RC1.lean`, `Model.lean:259-274`): RC1 is a real statement about the engine-identical functions `kernel_recompute_resolved_payout_rate` and `resolved_receipt_claimable_against_ledger`. I matched `ours/project/src/resolved_payout.rs` against the engine: the same blocks, apart from the wrapper and unit-struct lines. Its `Inv` has the same `backed` shape, so A1 applies. RC2-RC7 are about the transcription.

## 3. `wide_mul_div_floor_u128`

- `diff add1d027:src/wide_math.rs c38b6b1e:src/wide_math.rs`:
  - ours 1-1422 = upstream 1-1422;
  - ours 1423-1425 add two comment lines and `#[allow(dead_code)]` on `floor_div_signed_conservative_i128`;
  - ours 1426-1632 = upstream 1423-1629;
  - ours 1633-1723 change `wide_signed_mul_div_floor_from_k_pair` and add `wide_signed_mul_div_floor_with_carry_from_k_pair`;
  - ours 1724-2217 = upstream 1677-2170.
- `wide_mul_div_floor_u128` (ours 1623-1629) calls `mul_div_floor_u256` (1468), `U512::mul_u256`, `div_rem_by_u256`, `U256::from_u128` and `try_into_u128`. All of them are in the identical ranges. Nothing after 1632 is an `impl` that could change method resolution (the items after it are free functions and `mod tests`).
- Upstream's `Cargo.toml` at `add1d027` also has `overflow-checks = true`. The upstream prompt scope is "src/wide_math.rs as compiled without cfg(kani)", which is how this slice compiles.
- So upstream row F3 (`proved`, Aristotle, `Spec.WideMath.F3_wide_mul_div_floor_u128`, engine `add1d027`) is about the same text: checked, found no difference.
- The ours2 docs say "not proved here … we did not re-run or check that proof" (`ours2/README.md`, F3 bullet), and the spec gives the line ranges correctly. That is accurate. The remaining caveats are that it rests on upstream's record, not on a proof checked here, and that the prompt still asks Aristotle to prove F3 again.

## 4. Claims that overstate, with corrected wording

1. `ours/README.md:5`: "seven properties (RC1–RC7) were proved on Harmonic's Aristotle" → "Aristotle reported RC1–RC7 PROVED about a hand transcription; no proof terms were returned and nothing was rechecked locally." (A5)
2. `ours/README.md:5`: "Kani covers the same path for one or two accounts with bounded amounts." → "Kani harnesses exist for parts of this path (`tests/proofs_v16.rs`); no result is recorded here, and the whole-path e2e close harnesses did not finish." (A3) The same correction applies to `resolved_close_spec.md:197-203` and prompt line 6.
3. Prompt line 4: "I want to know that, for any number of accounts and any order of closes, the shares together never exceed the pool" → add "for consecutive runs of the close tail from a state satisfying `Inv`; the steps before the tail and other instructions are not covered." (A2)
4. `ours2/README.md:13`: "Two findings from running the engine text" → "… from running the slice's engine text from random states satisfying `Inv`." (A6)
5. `ours2/README.md:11`: "the 58 engine lines before the close tail are absent, so `pnl >= 0` is a precondition" → "… are absent. They write the ledger, the snapshot, vault, c_tot, insurance and the account. `pnl >= 0` and `Inv` at the tail are assumptions." (A7)
6. `ours2/JOB.md:10`: "they were not acted on" → record the later STEER-ACK "accepted". (A9)
7. `SLICE-DIFF.md:5`: "five engine functions are stubbed" → "nine engine functions are stubbed (four behavioural, five STUB-DEAD) and `as_view` is rewritten." (A11)
8. `ours/ledger.tsv`: `PREPARED, NOT SUBMITTED` → the run-1 outcome. (A10)

Checked and found accurate: `ours2/README.md:5` ("Nothing is proved by run 2"), the README's F3 bullet, and SLICE-DIFF D2's "NOT CLOSED".

## 5. Verdict

**Today no proof covers the engine's close path.** Run 2 proved nothing, because it was cancelled. Run 1's results, as reported by Aristotle and not rechecked, split into three kinds:

- **Engine text, in effect:** RC1, about two pure engine functions (`kernel_recompute_resolved_payout_rate`, `resolved_receipt_claimable_against_ledger`), assuming the `wide_mul_div_floor_u128` contract.
- **Engine text, via upstream:** F3 is covered by upstream's Aristotle record on byte-identical text (confirmed here).
- **Not engine code:** everything else in run 1 is about a hand transcription.

**If run 2's statements were proved as written,** they would cover the exact engine text of the tail of `close_resolved_account_not_atomic` (v16.rs:26643-26702) and everything it calls:

- receipt creation with snapshot capture;
- claimable amount, receipt payment, receipt clearing;
- `set_account_pnl(_inner)`;
- the token-flow check;
- the byte codecs;
- and the top-up core (26492-26525).

They would hold under these conditions:

- for `Ok` runs;
- from states satisfying `Inv` with `pnl >= 0`;
- with the 4 behavioural stubs (only harmless for `Ok` runs, given A8's argument for the burn stub);
- and with the F3 contract, which the upstream record discharges.

They would **not** cover:

- engine lines 26580-26642 (mode check, source-domain preparation, lapsed-backing expiry and its post-snapshot credit, B-chunk, fee sync, principal and bankruptcy settlement, leg detach, realize/refine);
- that `Inv` holds at resolution or is kept by any other engine operation. As written it is not kept by `kernel_credit_post_snapshot_residual` (A1);
- protection of backing-provider earnings and fresh backing (A4);
- the wrapper's revert-on-`Err`;
- close sequences interleaved with other engine steps (A2).

**What a skeptical reader would say:** "You proved that a 60-line tail is safe if it starts from a state you assume. The 58 lines before it, which move money and change the payout rate, are exactly what decides whether that assumption holds. Your assumption is not even closed under one of the engine's own rate updates. So 'a full resolved close never pays out more than the pool' is still a conjecture about the engine. It is well supported by the tail argument and the random search, but it is not proved. It maps to upstream rows `T15.*`/`T14.*`, which stay `open`."
