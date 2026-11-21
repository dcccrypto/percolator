# NOVEL-REPORT: checked-exit certificates over rustc MIR

Session `e2e-lean-fable-1121`, branch `agent/J-20261121-ENG-E2E-PROOFS-NOVEL`, 2026-11-21,
04:42–05:55 GMT. A parallel track to the Aristotle work, not an implementation of it.

## 1. Result in short

- **T14.header and T14.account** (`claim_resolved_payout_topup_not_atomic`) are proved for the
  exact text at `aeyakovenko/percolator@08d4fd09`: every `Ok` return leaves a state on which
  `self.validate_shape()` and `account.validate_with_market(&self.as_view())` return `Ok`.
  The proof is checked by Lean's kernel. It rests on a trusted translation from rustc's MIR to
  a small abstract program and on nine written assumptions about Rust and the crate (section 5).
- **T15** (`close_resolved_account_not_atomic`, the full resolved close) is **not** proved whole
  on the engine's text, and cannot be by this method: three of its ten `Ok` returns are reached
  after writes with no check, which needs reasoning about values. What is proved:
  - header: the 7 `Ok` returns that the tool can tie to a passing `validate_shape`; the other 3
    are named by block and source line (`v16.rs:22970`, `:23037`, `:23040`);
  - account: 6 of the 10 (the fourth unchecked one is `v16.rs:23006`);
  - the whole row T15.header **if** six named callees each preserve `validate_shape`
    (a kernel-checked reduction; the six contracts are assumptions, not results);
  - both whole rows, with no exclusion and no assumption, on a copy of the text with a 7-line
    patch that adds the checks before the unchecked returns (section 6). The engine's 528 tests
    pass on that copy as on the original. The same holds for our own `c38b6b1e` (1,480 tests).
- The same run, unchanged, covers **32 `.header` rows and 11 `.account` rows** of Toly's ledger
  (all open there), including the trade entry point: T30/T31, and the statement of row
  **TB.checked**, which the trade pilot attempted and left open.
- **Aristotle credits used: 0.** Kani runs: 0. One full run takes about 15 seconds on this machine.
- Two rounds of independent audit of the trusted translator **found four real soundness holes**
  in it. None is triggered by the engine's code; all are fixed and the engine results did not change.
  This is the method's weak point and is described plainly in section 5.

## 2. The method in plain words

The engine validates its state at run time: an entry point ends in `self.validate_shape()?` and
often `account.validate_with_market(&self.as_view())?`. A ledger row of class `[final]` says only
"the last thing before `Ok` is that check". That is a fact about the order of calls, not about
the value of any byte. The Aristotle attempts proved it with a byte-level model of the 9,419-byte
account and of every borrow, which is where the 1,970 credits went (`summaries/s2.md`).

This method proves the same fact one level up:

1. Take the function's MIR from `rustc` itself (pinned nightly, `-Zunpretty=mir -Zinline-mir=no`).
   In MIR every `?` is an explicit branch and every call is one instruction.
2. Translate it mechanically into a tiny abstract program with ten instructions. The state is
   opaque ("everything reachable from the arguments"); the validators are opaque functions of it.
   An instruction either cannot write the state (it is given only plain values and shared
   references), or may write all of it, or is one of the validators. Rust's type system decides
   which; no memory model is needed.
3. A dataflow pass computes, per block, the fact "the state has not been written since the
   validator returned `Ok`". Lean does not trust that pass: it re-checks the result as a
   certificate (`checkCert … = true` by `decide +kernel`).
4. One Lean theorem, proved once (`checkCert_sound`, 700-line `Core.lean`, axioms `propext` and
   `Quot.sound`), says: if the certificate checks, every `Ok` return of the abstract program
   leaves a state on which the validators return `Ok`.
5. Callees are handled bottom-up. A callee the tool has certified becomes a one-line summary in
   its callers; `call_of_cert` is the Lean theorem that makes that step legitimate. A callee
   contract can also be *assumed*, and then it is listed in the theorem's file.

Method document (written and pushed before any proof): `docs/ideas/e2e-novel-proof-method.md`.
Exact rules: `proofs/novel/METHOD-RULES.md`.

## 3. What is proved, and about what

Pin (from `proofs/novel/out/PIN.txt`): revision `08d4fd09bf8f5c7acec4dedc060fe66eab21431d`,
`src/v16.rs` blob `a4674830cfcd5eeaa6cb39696a0620db7c5b0a9b`, `rustc 1.99.0-nightly (d0babd8b6
2026-07-15)`, default features, Lean 4.34.0. That revision was chosen over our `c38b6b1e` because
the close-path functions exist in Toly's text (`src/v16.rs:22905`, `:22947`) and his ledger rows
are about it. The engine source is not modified; the MIR text of every function a theorem is about
is committed in `proofs/novel/mir/` with its sha256 in the Lean file.

"Proved" below means: the Lean module compiled and its theorem's axioms were printed as
`[propext, Quot.sound]` (`out/lake_build.log`, `out/axioms.txt`), and the statement is about the
abstract program produced from that MIR. It is a statement about the Rust function only together
with section 5.

### T14 `claim_resolved_payout_topup_not_atomic` (`src/v16.rs:22905–22914`)

`MirFlow.Gen.S_claim_resolved_payout_topup_not_atomic.main`: for every execution that returns
with discriminant 0 (`Ok`), `chk 0 w1 = true ∧ chk 1 w1 = true`, where `chk 0` is
"`self.validate_shape()` returns Ok on the final state" and `chk 1` is
"`account.validate_with_market(&self.as_view())` returns Ok on the final state".
MIR: 26 blocks, 29 locals, sha256 `10c21dba…9ad9`. No callee summary is used, no exclusion.

- **T14.header**: the row's statement ("if it held before and the call returns Ok, it holds
  after") follows; the theorem is stronger (it holds after whether or not it held before).
- **T14.account**: likewise for the one account the function is given.
- T14.scan and T14.mirror: not addressed (they need values).

`proofs/novel/out/T14-side-by-side.md` lists the 26 MIR blocks next to their abstract
instructions, so this one translation can be checked by eye.

### T15 `close_resolved_account_not_atomic` (`src/v16.rs:22947–23122`)

MIR: 313 blocks, 341 locals, sha256 `f7bfa0fd…0f72`. Ten `Ok(..)` assignments (source lines
22970, 22982, 22991, 23006, 23018, 23037, 23040, 23048, 23060, 23122).

| Theorem | Says | Status |
|---|---|---|
| `T15_header_checked_paths.main` | every `Ok` return that does not pass through bb22, bb143, bb152 (the `Ok(ProgressOnly)` at `v16.rs:22970`, `:23037`, `:23040`) leaves `validate_shape` Ok | proved |
| `T15_account_checked_paths.main` | the same for `validate_with_market`, excluding also bb77 (`v16.rs:23006`, where only `validate_shape` is called) | proved |
| `T15_header_conditional.main` | **if** `expire_one_lapsed_source_backing_for_resolved_active_leg_not_atomic`, `settle_resolved_account_kf_not_atomic`, `settle_resolved_account_side_effects_after_kf_not_atomic`, `settle_negative_pnl_from_principal_not_atomic`, `settle_resolved_bankruptcy_negative_pnl`, `detach_solvent_active_legs_for_resolved_close` each preserve `validate_shape` on their `Ok` returns, then every `Ok` return of the close preserves it | proved as a reduction; the six contracts are **assumed** |
| `T15_account_conditional.main` | the same for the account check, with seven assumed callees | proved as a reduction; **assumed** contracts |

The tool found the unchecked returns by itself; they are the ones the ledger's T15.header note
and our earlier hand reading name. The return at 22970 is checked inside the callee
(`prepare_one_source_domain_for_resolved_close_not_atomic` validates before every `Ok(Some(_))`);
the tool does not tell `Ok(Some(_))` from `Ok(None)`, so it uses that callee only for
preservation, which is enough for the conditional header theorem.

**No T15 row is closed by this.** T15.header and T15.account stay open on the engine's text
until the callee contracts are proved by a method that reasons about values, or the text changes
(section 6). `out/T15-header-frontier.md` pushes the six contracts down the call graph to 16
functions that write the market in their own bodies; that list is a map of where value reasoning
is needed, not a list of true lemmas (a leaf on its own need not preserve the predicate).

### The other rows the same run covers (`proofs/novel/out/RESULTS.md`)

| | Count | Which |
|---|---|---|
| `.header` rows proved | **32** | 27 of the 28 `[final]` rows and all 5 `[delegates]` rows |
| of which "holds after" (`est`) | 23 | |
| of which "preserved" (`pres`, the ledger's statement) | 9 | functions with an early `Ok` before any market write, e.g. T59 |
| `.account` rows proved | **11** | 8 `est`, 3 `pres` |
| `[final]` row not proved | 1 | T18: the check is skipped when `converted == 0`; that this implies no write is a value fact (the ledger's own note says the same) |
| not attempted by design | | the `[real]`, `[mixed]`, `[local]`, `[frame]`, `[dispatch]` header rows; every `.scan` and `.mirror` row |

The trade: `MirFlow.Gen.S_execute_batch_with_fee_loss_stale_scoped_not_atomic.main` (62 blocks)
proves that every `Ok` return of the batch trade entry point follows a passing `validate_shape`
and `validate_with_market` of both accounts with no write after them. That is T30.header,
T30.account, T31 by delegation, and the English statement of **TB.checked** (ledger: "attempted,
open" after the trade pilot). It is not a proof of his Lean term `Spec.Trade.tb_checked`, which
is stated in a byte-level separation logic; it is the same property at a different level, with
the assumptions of section 5 instead of a memory model.

Whole-engine pass: 334 methods of `MarketGroupV16ViewMut` take the market; 178 get a summary;
2 are not translated (a reference argument is reborrowed mutably). 44 Lean modules.

## 4. What the statements do not say

- Nothing about panics, compute limits or termination: every statement is "if it returns Ok".
- Nothing about whether `validate_shape` is the right predicate. That is the ledger's definition
  of H.
- A `*_checked_paths` theorem is about executions that avoid the listed blocks. It says nothing
  about the others.
- A `*_conditional` theorem has its assumptions built into the abstract program; they are listed
  in the file header and in `RESULTS.md`, marked ASSUMED.

## 5. What is assumed, and how far to trust it

1. **rustc's MIR** has the behaviours of the function. Toly's Aristotle model is generated from
   the compiled crate too; this is the same kind of trust.
2. **Nine Rust-level assumptions**, A1–A9 in `METHOD-RULES.md`: no `unsafe` blocks; no interior
   mutability, mutable statics or raw pointers; the validators are deterministic; the two
   by-value view types hold only shared references; `T` holds no references; two facts about
   `core`'s `?` on `Result`; `bytemuck`'s safe API; unwinding never returns normally; printed names denote what the tool takes them for. A1–A6
   and A9 are checked by text scans of the source and of the MIR (`out/assumptions_check.txt`, 17
   PASS, 0 FAIL). Scans are evidence, not proof.
3. **The translator** (`mirflow/extract.py`, class `Translator`): that every execution of the MIR
   is an execution of the abstract program. Lean cannot check this. It is the weakest link:
   - Mutation tests: 18 edits of T14 (writes after the check, check removed, result ignored,
     inverted, moved, closures, reborrows, `mem::swap`, harmless edits). 0 unsound answers
     (`out/MUTATION-TESTS.md`).
   - Independent audit, round 1 (`proofs/novel/AUDIT-EXTRACTOR.md`): a separate agent, read-only,
     told to break it. It **reproduced two holes**: (1) a reborrow of the market stored into
     memory behind another argument made later writes look harmless; (2) a string literal
     containing `<` or `[` hid a `&mut` argument from the parser. It also listed four risks.
     Its scans found neither pattern in the engine's MIR (0 pointer-valued stores through a
     pointer; 0 mis-split calls of 12,848). All were fixed; the summary table and the generated
     Lean were identical before and after. Its 37 attack functions are now a regression test
     (`out/AUDIT-REGRESSION.md`, 0 unsound).
   - Audit round 2, on the fixed translator: **two more holes**. (3) `Ok(Err::<(), E>)`, whose
     payload is the function item `Err`, was read as `Err`; (4) a crate item named like a trusted
     one (`Try`, `PortfolioV16View`, …) would be read as the trusted one, because rustc prints the
     shortest unambiguous path. Again neither occurs in the engine (auditor's scans of the source
     and of the 41 certified functions). Fixed: bracket-matched recognition, and a ninth
     assumption A9 checked by script. Regression test: 40 cases, 0 unsound.
   - N-version check: a third agent wrote a second translator from `METHOD-RULES.md` alone, without
     reading `extract.py`, and compared all 41 functions, 2,627 blocks. It reported no block where
     `extract.py` is less conservative than the rules; the 186 textual differences are
     equivalent or more conservative. Its files are in `/tmp`, not in the repository.
   Four holes in two rounds, all in text- and name-based recognition, means more may exist. The honest reading: the engine results
   above are as good as the translator on the MIR shapes the engine actually has, which two
   independent reviews and two test suites have exercised, and no better.

## 6. An option the result suggests (not applied to the engine)

T15 is `[mixed]` only because three `ProgressOnly` returns skip the check, and one calls
`validate_shape` without `validate_with_market`. `proofs/novel/patches/t15_checked_exits.patch`
adds the missing calls (7 lines). On a copy with the patch:

- `MirFlow.GenT15Patch.T15_patched.main`: every `Ok` return leaves both checks Ok. No exclusion,
  no assumed contract. Kernel-checked. So with this patch T15.header and T15.account become
  rows of class `[final]`.
- `cargo test --release --features fuzz,audit-scan`: 528 passed, 0 failed, on the original and on
  the patched tree (`out-t15-patch/TESTS.md`). A first run of the patched tree reused the
  baseline's binaries and was discarded; the numbers are from a rerun that compiled the patch.

The same was done for our own text: `patches/t15_checked_exits_c38b6b1e.patch` (7 lines),
`MirFlow.GenT15PatchRc38b6b1e.T15_patched.main` kernel-checked, and 1,480 passed / 0 failed /
19 ignored on both trees with identical per-test results (`out-t15-patch-c38b6b1e/TESTS.md`).
That run is with default features plus `fuzz,audit-scan`; which features our wrapper enables was
not checked.

This is a test result, not a proof that behaviour is unchanged: an added check turns an `Ok`
into an `Err` on any state that fails it, and it costs compute units. The ledger notes that the
program's CloseResolved handler already validates the shape after the call (row W19), so in the
deployed composition the header check is duplicated, not new. Whether to take the patch is the
engine owner's decision. It is the smallest form of ROADMAP's first way forward for this entry
point: 7 lines instead of moving logic into kernels.

## 7. Two more runs of the same tool

- **`--features audit-scan` build** (the build the fuzzers run; `out-audit-scan/RESULTS.md`).
  There `validate_shape_audit_scan` is a real check: 39 header rows, and the T15.header reduction
  needs only two assumed callees (`settle_resolved_bankruptcy_negative_pnl`,
  `detach_solvent_active_legs_for_resolved_close`). In that build `validate_shape` also runs the
  full per-asset scan (`src/v16.rs:9378–9380`, read, not proved), so the same 39 theorems are
  the `.scan` rows **for that build only**. They say nothing about the deployed build, where
  the scan is not run.
- **Our own text, `dcccrypto/percolator@c38b6b1e`** (`out-c38b6b1e/RESULTS.md`): 33 header rows,
  12 account rows, 47 modules, 15 seconds. T15.header there needs five assumed callees; its
  unchecked returns are `v16.rs:26594`, `:26631`, `:26634` (and `:26614` for the account). The
  assumption check caught three `static AtomicU64` in our source; they are behind
  `cfg(feature = "x1-diff")` and absent from the MIR of the analysed build, which is what the
  check now reads.

A revision bump therefore costs one run of the script, not a re-proof by hand.

## 8. Credits and tools

| | Before | After | Used by this session |
|---|---|---|---|
| Aristotle free credits | 244 (04:41:01Z, shared budget file) | 202 (05:24:44Z, `aristo.sh balance`) | **0**: no job was submitted by this session; the change is not ours |

Recorded in `/Users/khubair/agent-worker/aristotle-budget.md`. Project `eb31e0d7` was not touched.
Kani: not run. The six callee contracts T15.header needs must hold for every state; the existing
harnesses fix concrete market shapes (`src/v16_proofs.rs:2685–2707` keeps one fixture "avoiding
three full validation scans in one CBMC query"), so their verdicts cannot be imported as
all-states summaries. The import interface exists (an assumed summary is the same object as a
certified one) and is unused.

## 9. Why this is new compared with Toly's approach

| | Toly at 08d4fd09 | This track |
|---|---|---|
| Model of a stateful method | Aristotle's byte-level model of the compiled body: account bytes, retags, borrows handed back on every exit | An abstract program over an opaque state; ten instructions; borrows appear only as "can this value write?" |
| Who walks the body | The prover, per property, per method (trade: 139 blocks, 296 locals; walk left open) | A dataflow pass; Lean re-checks its answer as a certificate by evaluation |
| Cost of a `[final]` row | "Aristotle, small" in the ledger; 3 of 32 stateful properties for about 1,970 credits so far | Seconds of local CPU; 0 credits; 32 header and 11 account rows in one run |
| Cost of a new revision | Re-prove the changed functions | Rerun the script (done for `c38b6b1e`) |
| ROADMAP's three ways forward | (1) refactor to kernels, (2) a model tied by tests, (3) pay for byte-level proofs | A fourth: keep the code, prove the structural half by certificate, and hand value reasoning a precise list of callee contracts |
| What it cannot do | Nothing in principle; cost is the limit | Anything that needs a value (`[real]` rows, `.scan`, `.mirror`, kernels) |
| Trusted base | Aristotle's Rust model and its unreturned proof terms | rustc's MIR, nine assumptions, a 1,000-line translator (audited twice, four holes found and fixed), Lean's kernel |

The two are complementary. s2.md says that for the trade "the pure step from the callee
contracts' conclusions to all five properties is proved" and "the body walk is open". This tool
is a body walk. Its `call` instruction with `pres`/`est` lists is the place where a kernel
contract proved by Aristotle or Kani plugs in.

What is not new: abstract interpretation, proof-carrying certificates and reading MIR are
standard techniques. The contribution is putting them on this engine's runtime-check design, with
a kernel-checked soundness proof, at the ledger's row ids.

## 10. Limits and what would come next

- No value reasoning (T18; `Ok(Some)` against `Ok(None)`; the `[real]` rows).
- The translator is trusted. Next step: read MIR as data from a rustc driver (`rustc_public`)
  instead of parsing text: resolved paths and types from the compiler would remove the class of
  error behind all four holes. Not done: the compiler's development libraries are not installed
  here and nothing was downloaded.
- Place-sensitive facts ("this function never writes field X") would give the frame halves of
  `.scan` and `.mirror`; the Lean core would need a state with fields.
- The assumed contracts of the T15 reduction are the work list for Kani or Aristotle.

## 11. Reproduce, and where things are

```
proofs/novel/run_all.sh                                        # 08d4fd09, deployed build
VARIANT=audit-scan proofs/novel/run_all.sh                     # audit-scan build
REV=c38b6b1e ENGINE=dcccrypto/percolator proofs/novel/run_all.sh
proofs/novel/experiments/t15_patch.sh
python3 proofs/novel/mirflow/mutation_tests.py   <src> <scratch> nightly-2026-07-16
python3 proofs/novel/mirflow/audit_regression.py <src> <scratch> nightly-2026-07-16
```

Files: `proofs/novel/README.md`. Progress notes with times: `proofs/novel/NOTES.md`.
Commits on the branch: `a8c79cbd` (method doc), `13da3407`, `89124830`, `055ae9ab`, `e4ce2a80`,
`ef66b6a7`, and the commit that adds this report. Nothing was pushed anywhere else; no PR, no post.
