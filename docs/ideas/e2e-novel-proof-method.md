# A cheaper method for the stateful whole-path proofs: checked-exit certificates over rustc MIR

Written 2026-11-21 (about 04:45–05:15 GMT), before any proof was built. Everything in the
"what exists" section is quoted from files at `aeyakovenko/percolator@08d4fd09`
(`proofs/ROADMAP.md`, `proofs/proof_ledger.tsv`, `proofs/aristotle/summaries/s2.md`) or observed
locally; everything in "the system" section is a design, not a result. Results go in
`NOVEL-REPORT.md`.

## 1. Users and demand

- **The engine's maintainer (upstream) and us (dcccrypto fork, v2.2 line).** ROADMAP at 08d4fd09:
  181 of 624 pieces done, 0 of 89 charter invariants closed. Phase 2 ("the engine cannot panic and
  keeps the header") is 9 of 95; phase 3 is 1 of 197. The ROADMAP says the plan for phases 2 to 4
  "has to change" and lists three ways forward, none chosen.
- **The concrete unmet demand** is the `T` layer of the ledger: for each of the 50 deployed entry
  points, four rows (`.header`, `.scan`, `.account`, `.mirror`). Of the `.header` rows of the
  deployed build, 28 are classed `[final]` ("every Ok path ends in `validate_shape()` after its
  last market write ... the obligation is that path structure (Aristotle, small)"), 3 `[mixed]`
  (T15 is one), 4 `[real]`, and the rest local/frame/delegating. All are open except two that have
  to be re-established.
- **Anyone re-proving after a code change.** ROADMAP: "Every engine change on the way costs a Kani
  run (about five hours) and re-proving the functions it touches". A method whose per-function
  cost is seconds of local CPU changes what a revision bump costs.

## 2. What exists, and why the 3-of-32 attempts failed

- **Aristotle on pure functions works**: 171 pieces, about 14 credits each.
- **Aristotle on methods that hold the market and an account does not, at this price**: 3 of 32
  properties for about 1,970 credits. The prover's own account (`summaries/s2.md`): "the size of
  the bodies and the number of `?` exits; the state model of the 9419-byte account; retags and
  reborrows; above all, shared borrows of the caller's local variables, which must be handed back
  on every exit". The trade body is "139 blocks, 296 locals, 25 calls and 23 `?` exits". ROADMAP's
  summary: "It is not the logic."
- **Kani**: verifies kernels and some entry points for all inputs, but "the trade, the full
  refresh and the full resolved close" are recorded as beyond its budget.
- **Our own earlier runs** (parent branch): run 1 proved 7 properties of a hand transcription, not
  of engine text; run 2 was cancelled; v3 follows ROADMAP's way 1 (kernels). None closes a T row.

**Diagnosis.** The failed attempts prove a *structural* fact (a `[final]` row says only "the last
thing before `Ok` is the runtime check") with a *memory-level* tool. The Aristotle model executes
the compiled body over bytes and borrow stacks, so it pays for the 9,419-byte account and for
every reborrow, although the fact being proved does not depend on the value of a single byte. The
cost is in the wrong layer. Rust's own type system has already proved the part that is expensive
there: a callee that receives only shared references to data without interior mutability cannot
change it.

## 3. Candidate methods

Scores are 1 (bad) to 5 (good); "gap risk" scores high when the risk of a hole in the argument is
low. They are my judgement, written before building anything.

| # | Method | Feasible today | Covers exact engine code | Reuse | Credit cost | Gap risk | Sum |
|---|---|---|---|---|---|---|---|
| M1 | **Refinement layer**: prove once that the byte layout equals a record model (`from_bytes`/`to_runtime` round trips), then do every proof over records | 2 | 4 | 4 | 2 | 4 | 16 |
| M2 | **Generated Lean model** of the engine (Charon/Aeneas, which turn safe Rust into pure functions with borrows compiled away) | 1 | 5 | 5 | 4 | 3 | 18 |
| M3 | **Per-field frame lemmas** ("this function writes only these fields"), composed along a path | 3 | 3 | 4 | 3 | 3 | 16 |
| M4 | **Kani-to-Lean bridge**: Kani-proved local facts imported as axioms with a written soundness argument | 4 | 4 | 3 | 5 | 2 | 18 |
| M5 | **Checked-exit certificates over rustc MIR** (new; section 4): a dataflow fact, "the state has not been written since `validate_shape` returned `Ok`", checked on the compiler's MIR of the exact function by a checker whose soundness is a Lean theorem; callee facts enter as summaries | 5 | 5 | 5 | 5 | 3 | 23 |
| M6 | **Kani shell harnesses**: stub every callee as a havoc of what it borrows mutably and `validate_shape` as a ghost predicate, let Kani walk the shell | 3 | 4 | 3 | 5 | 2 | 17 |

Notes on the scores.

- **M1** removes the byte model but not the borrow bookkeeping and the `?` exits, which s2.md
  ranks as the larger cost. It also needs Aristotle for the layout proof itself (zero-copy casts
  go through `bytemuck`).
- **M2** is the right long-term answer for the `[real]` rows, because it removes both costs by
  construction. Not feasible today: neither tool is installed here (`charon`, `aeneas` not found),
  the engine is one 25,350-line file with generics over `T`, `bytemuck` casts and const-generic
  arrays, and a first translation of a crate this size is days of work with an unknown end.
- **M3** exists upstream as text scans (`write_sets.py`, `header_writes.py`); turning those into
  proofs needs a semantics. M5 gives it one (same checker, finer facts), so M3 is M5's version 2.
- **M4** is sound only with a precise statement of what a Kani harness proved (bounds, stubs,
  features). It is an import format, not a walk of a body. M5 uses it as exactly that: a Kani
  fact becomes a callee summary.
- **M6** needs hand-written stubs for every callee (each one a place to make a mistake) and a
  memory model in CBMC; the result is a bounded-model-checking verdict, not a kernel-checked proof.

**Picked: M5**, with M4 as its import interface for callee facts and M3 as its next version.

## 4. The system proposed (M5)

### 4.1 The observation

The engine is built around runtime checks: an entry point ends in `self.validate_shape()?` and
often `account.validate_with_market(&self.as_view())?`. For such a function the ledger statement

> if the header predicate held before `f` and it returns Ok, it holds of the state it leaves

follows from three facts, none of which mentions a byte of the account:

1. on the path to that `Ok`, `validate_shape` was called on `self` and returned `Ok`;
2. nothing that can write the market ran after that call returned;
3. `validate_shape` is a deterministic function of the state it can read.

Facts 1 and 2 are a property of the control-flow graph plus, for every statement and call after
the check, the question "can this write the state?". Rust answers that question by types: a call
whose arguments carry no `&mut`, no raw pointer and no closure over one cannot write state behind
a shared reference (given no interior mutability; `src/lib.rs:8` has `#![deny(unsafe_code)]` and
the crate has no `Cell`/`UnsafeCell`/`static mut`; this is checked by a script, not assumed).

### 4.2 Pipeline

```
src/v16.rs @ pinned blob
   │  rustc (pinned nightly)  -Zunpretty=mir -Zinline-mir=no          [trusted: the compiler]
   ▼
MIR text of the function (kept in the repo, with its sha256)
   │  extract.py: transliterates blocks into a 10-instruction abstract IR;   [trusted: ~300 lines,
   │  anything it does not recognise becomes "may write everything"          conservative by default]
   ▼
Lean term  cfg : Cfg   +  certificate  inv : Block → abstract state   [certificate is untrusted]
   │  Lean kernel evaluates  checkCert cfg inv = true
   ▼
theorem  f_header : ∀ run of cfg, returns Ok → chk₀ (final world) = true      [kernel-checked]
```

- **Abstract IR (Lean).** A world `W` is an opaque type: everything reachable from the function's
  mutable arguments. Checks are opaque deterministic functions `chk k : W → Bool`
  (`k = 0`: `validate_shape` returns Ok; `k = 1`: `account.validate_with_market(&self.as_view())`
  returns Ok). Instructions: write the world (any assignment through a dereference, any call or
  drop that receives something that may carry `&mut`); a call that cannot write; a check call
  (sets its destination's discriminant to `Ok` iff `chk k w`); copy/set/read of an enum
  discriminant; `switchInt`; `Try::branch` and `FromResidual::from_residual` for `Result` (two
  facts about `core`, stated as part of the semantics); `return`. Every unmodelled value is
  nondeterministic, so the abstract program has at least the behaviours of the real one.
- **Checker (Lean).** `checkCert` verifies that the certificate is inductive (each block's
  abstract state implies each feasible successor's after the transfer function) and that every
  `return` block either knows the result is `Err`, or knows the required checks hold, or is in an
  explicit exemption list.
- **Soundness (Lean, proved once).** `checkCert cfg inv ex = true →` for every abstract execution
  from the entry that returns `Ok` at block `b`: all required checks hold of the final world, or
  `b ∈ ex`. Proved once for all functions; per function the proof is `by decide`.
- **Callee summaries.** A call instruction may carry a summary instead of "may write everything":
  *cannot write* (by signature), *returns Ok ⇒ check k holds* (the callee itself is certified by
  this tool: recursion on the call graph), or *preserves check k* (a hypothesis of the theorem,
  named after the callee; this is where a Kani verdict or an Aristotle kernel proof is plugged
  in, each with its own evidence line). The theorem for a `[mixed]` or `[real]` row is then
  conditional and says exactly on which callee contracts.

### 4.3 What it proves and what it does not

- Proves: `.header` rows of class `[final]` outright; `.account` rows for functions that end in
  `validate_with_market`; for `[mixed]` rows the checked paths outright and the rest conditional
  on a printed list of callee contracts.
- Does not prove: anything that needs the *value* of a field (`.scan` and `.mirror` deltas, the
  four `[real]` header rows, kernels). Those stay with Kani and Aristotle; this tool removes the
  body walk from them, because a kernel contract plus this tool's path facts is what s2.md calls
  "the pure step from the callee contracts' conclusions", already proved there for the trade.
- No-panic is out of scope: the statements are "if it returns Ok".

## 5. Version 1 and later

**v1 (today):** extractor, Lean IR, checker, soundness theorem; T14.header and T14.account;
T15.header and T15.account with exemptions or callee hypotheses for the unchecked `ProgressOnly`
returns; a run over every `[final]` header row to measure how many the tool certifies with no
change; mutation tests (insert a write after the check, remove the check: the certificate must be
rejected).

**Later:** (a) place-sensitive facts ("field X not written") for the frame halves of `.scan` and
`.mirror` (this is M3 with a semantics); (b) summaries generated bottom-up over the call graph
instead of by hand; (c) the extractor replaced by a rustc driver reading MIR as data
(`rustc_public`), removing the text parser from the trusted base; (d) Aeneas (M2) for the
`[real]` rows.

## 6. Risks

1. **The extractor is trusted.** A misclassified statement would make a false theorem. Mitigation:
   default-deny (unknown ⇒ may write); the MIR text is committed next to the generated Lean so a
   reader can check the transliteration; mutation tests. Residual risk is stated in the report.
2. **The Rust-level assumptions** (shared references are read-only; `validate_shape` is
   deterministic; `rustc`'s MIR is what is compiled) are assumptions, not theorems. They are the
   same kind Toly's ledger lists as "the prover's model of Rust", and weaker than a full memory
   model in one way (no byte layout assumed) and stronger in another (the aliasing guarantee is
   used, not re-derived).
3. **A `[final]` row may be cheaper than it looks to Aristotle too.** Then the saving is small
   per row. The pilot numbers say otherwise for bodies with many exits, but only one row
   (T14.header) is small enough to compare directly.
4. **MIR is generic** (`T` unresolved). A property of the generic MIR holds for every
   instantiation, including `T = ()`; trait-dispatched calls are treated by signature only.
5. **Scope creep in claims.** A certificate says "the check passed on the returned state". It
   does not say the check is the right predicate; that is the ledger's own definition of H.

## 7. Success metrics

- T14.header and T14.account: kernel-checked theorems about MIR of `v16.rs` blob `a4674830`
  (08d4fd09), zero credits.
- T15.header / T15.account: kernel-checked, with the exact set of unchecked returns and the exact
  callee contracts they need, as theorem hypotheses.
- Number of the 28 `[final]` header rows certified by the unchanged tool, and wall time.
- Every mutation test rejected.
- Aristotle credits used: target 0.
