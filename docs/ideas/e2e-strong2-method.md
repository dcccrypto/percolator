# e2e-strong2: guarded-clause certificates (value facts for the checked-exit method)

Session `e2e-lean-strong-1121b`, branch `agent/J-20261121-ENG-E2E-PROOFS-STRONG2`, written
2026-11-21 about 15:05 UTC, before any proof work of this session (sections 1 to 6; section 7 was
added at the end and says what changed). Local Lean only; no Aristotle
job is submitted.

## 1. What is being attempted

Target text: our own engine, `dcccrypto/percolator`, the `src/v16.rs` of revision `c38b6b1e`
(the same file is at this branch's head; blob ids are pinned in `proofs/strong2/out/PIN.txt` by
the run script). The same script is then run on `aeyakovenko/percolator@08d4fd09` where time
allows, because the ledger rows are written about that text.

Rows:

1. **T18** `convert_released_pnl_to_capital_not_atomic`, `.header` and `.account`. The final
   checks are skipped when `converted == 0`. In our text there is also a lien release *before*
   the core (`src/v16.rs:25482`), so the row is not just "the core returned before writing".
2. **T15** `close_resolved_account_not_atomic` on the unpatched text: the `Ok(ProgressOnly)`
   returns that the sibling run (`proofs/novel`) had to exclude (`src/v16.rs:26594`, `:26614`
   for the account, `:26631`, `:26634`), and the callee contracts its reduction assumed.

## 2. What the sibling run could not do, and why

`proofs/novel` (session e2e-lean-fable-1121) proves "the last thing before `Ok` is a passing
check" from rustc's MIR with a certificate that Lean's kernel re-checks. Its abstract state knows
one kind of value fact: a local is a constant, or "if this local is 0 then these checks hold".
That is enough for `x?` after a validator and for nothing else. It cannot express:

- "the callee returned `Ok(v)` and `v == 0`, so the callee wrote nothing" (T18);
- "the callee returned `Ok(Some(_))`, so the callee validated" (T15, `:26594`);
- "the callee returned `Ok(None)`, so the callee wrote nothing" (T15, the lapsed-backing step);
- "this function returns `Ok(v)` only with `v != 0`" (the conversion core after its
  `if converted == 0 { return Err }` guards).

Toly's approach (Aristotle on a byte-level model of the compiled body) can express all of these
but pays for the whole memory model on every row.

## 3. The new parts

### 3.1 A clause domain instead of per-local facts

The abstract state becomes a list of **facts** (literals `local = n`, `local ≠ n`) and a list of
**guarded clauses**

    lit_1 ∧ … ∧ lit_n  ⟹  ⊥    or    lit_1 ∧ … ∧ lit_n  ⟹  (checks hs hold now, checks ps hold now if they held on entry)

over the function's locals. The sibling's `const n` and `cond hs ps` are the special cases with
zero or one guard. Comparisons become clauses (`c = (m == n)` is `c=0 ∧ m=n ⟹ ⊥` and
`c≠0 ∧ m≠n ⟹ ⊥`), so a later `switchInt(c)` gives a fact about `m` by one generic entailment
step. One entailment function (`entails`) is used for joins, for edges and for the goal; it is
proved sound once. The checker stays a `Bool` function evaluated by the kernel
(`decide +kernel`), same trust style as `checkCert_sound`, but it is a new core
(`proofs/strong2/lean/ClauseFlow/Core.lean`) with its own soundness proof; nothing of
`MirFlow/Core.lean` is edited.

### 3.2 Ghost payload locals, so a value crosses `?` and a call

Each local `_l` gets a ghost `P(l)`: the integer (or enum discriminant) carried by `_l` when
`_l` is `Ok(x)` / `Continue(x)`; unconstrained otherwise. `_0 = Ok(copy _5)` copies `_5` to
`P(0)`; `Try::branch` moves the payload when the source is `Ok`;
`copy ((_3 as Continue).0: u128)` reads it. This is what lets `converted` in the caller be the
same number as the callee's return payload.

### 3.3 Value-indexed callee summaries

A callee summary becomes a list of clauses over `(result discriminant, result payload)`:
`Ok ∧ payload = 0 ⟹ every check that held before the call holds after` (no write on that path),
`Ok ∧ payload = 1 ⟹ checks {0,1} hold` (validated before `Ok(Some)`),
`Ok ∧ payload = 0 ⟹ ⊥` (never returns `Ok(0)`). A summary is produced only by the callee's own
kernel-checked certificate for a goal made of those clauses; the Lean theorem `call_of_cert`
(new version) states that such a certificate justifies the caller's `call` instruction.
"No write" is not a new primitive: it is "each check is preserved", check by check, which the
opaque world already supports.

### 3.4 A mutation harness for the *checker* (kernel-evaluated)

The sibling mutated Rust source to test the translator. Here the certificates themselves are
attacked, inside Lean: for every theorem of this run a generated file states, as kernel-checked
theorems (`checkCert … = false`), that the checker rejects

- the program with any one validator call weakened (its `est` removed),
- the program with a write inserted before any one `Ok` return it has to prove,
- the program with any one value guard changed (comparison constant or switch key),

when given the original certificate, and the Python fixpoint is rerun on each mutant to show no
other certificate is found either. Survivors, if any, are listed with the reason (a mutant can
be semantically harmless). The claim made is the measured kill table, not "all wrong
certificates fail".

### 3.5 Refutation by a test on the real text

For returns that are reached after writes with no check (`:26631`, `:26634`) the statement "the
header predicate is preserved" is a property of the engine's arithmetic. Before trying to prove
it, a Rust test on the unmodified engine source searches for a state with `validate_shape() ==
Ok` before the call, `Ok(ProgressOnly)` through those lines, and `validate_shape() != Ok`
after. Found: the row as literally stated (hypothesis: header predicate only) is refuted, and
the test is committed. Not found: nothing is claimed; the contracts stay on the work list with
the exact obstacle.

### 3.6 Kani bridge (only if time remains)

An assumed summary and a certified one are the same object, so a Kani verdict can be imported as
a clause. It is sound only if the harness quantifies over all states of the fields the callee
reads; the existing harnesses fix concrete shapes. Not attempted unless a callee is small enough
for a fully symbolic header; any import would be listed as ASSUMED-FROM-KANI with its bound.

## 4. Why this is new relative to the two existing approaches

| | Toly (08d4fd09 ledger, Aristotle) | sibling `proofs/novel` | this run |
|---|---|---|---|
| value facts | full (byte model) | constants and one-guard conditionals on a call result | Horn clauses over local values and result payloads |
| callee summary | per-function contract in separation logic | `pres` / `est` lists | clauses indexed by the returned value |
| T18 | open ("found when read") | open (stated limit) | target |
| unchecked `ProgressOnly` returns of T15 | class "real", open | excluded, or 7-line patch | proved where a value fact suffices; searched for a counterexample where it does not |
| test of the checker | none | source mutations (translator) | certificate/program mutants rejected by kernel evaluation |

Abstract interpretation with certificates and Horn-clause domains are standard; what is new here
is the combination on this engine's MIR with a kernel-checked soundness proof and the ghost
payload across `?`.

## 5. Risks

1. **The trusted translator grows.** New rules: payload ghosts, comparisons against constants,
   `Option::Some/None` discriminants, reading a `Continue` payload. Each can be wrong in the same
   way the sibling's four holes were. Mitigation: the rules are written in
   `proofs/strong2/RULES.md` before results are claimed; negative tests (Rust functions built to
   fool each rule) must yield "not proved"; one independent hole-hunt before the report.
2. **A stale ghost.** If some assignment to `_l` is translated without resetting `P(l)`, a
   payload fact survives a change of the value. Mitigation: the reset is done in one place (a
   post-pass over every instruction that assigns `l`), and is an audit target.
3. **Integer modelling.** A local's value is a `Nat`; comparisons are only translated for
   unsigned and `bool` operands against non-negative constants; anything else is `havoc`.
4. **Soundness proof does not close in time.** Then no theorem is claimed; the abstract program
   and certificate are reported as unchecked.
5. **Value facts about memory** (`account.header.pnl < 0`, the bitmap) are out of reach: the
   domain is about locals, not fields. Returns that depend on them stay open and are named.

## 6. Success metrics

- T18.header and T18.account: kernel-checked on the unpatched text, no excluded block, no
  assumed callee; axioms printed.
- T15: number of `Ok` returns proved on the unpatched text (sibling: header 7/10, account 6/10 on
  08d4fd09; on our text 3 and 4 excluded sites); number of assumed callee contracts in the
  whole-row reduction (sibling: 5 header / 6 account on our text).
- Mutation harness: mutants generated, killed, survivors explained.
- Hole-hunt: negative tests run, unsound answers (target 0), holes found and fixed.
- Refutation search: either a committed failing-property test or an explicit "none found" with
  what was searched.
- Every assumption listed in the theorem files and in `RESULT.md`.

## 7. Addendum, written after the work (what was done differently)

- 3.1 to 3.4 were done as planned (`proofs/strong2/lean/ClauseFlow/Core.lean`, `valflow/`).
  `entails` got two rounds of unit propagation, and comparisons also rewrite existing clauses;
  both are in the soundness proof.
- Not planned, added during the work: **rule R9** (variant index of a plain crate enum, used to
  state "the close returned `Closed`"), and **rule R10** (a read-set frame: which header fields
  and whether slot contents each runtime check can read, so that a write to an unread region
  keeps the check). R10 is a second trusted analysis (`valflow/readset.py`); it is what closes the
  `[frame]` rows.
- 3.5 gave witnesses, not a proof: see `tests/strong2_t15_progress_only_shape.rs` and
  `proofs/strong2/T15-WORKLIST.md`.
- 3.6 (Kani bridge) was not attempted. Reason found when read: under `cfg(kani)` the engine's
  `validate_shape` also runs the full audit scan (`src/v16.rs:10063`), so a Kani harness cannot
  evaluate the default-build header predicate through the engine's own function; a harness-side
  copy of the predicate would be a model, not the engine text.
- Risk 1 materialised as predicted: the independent audit found two holes in the assumption
  scans and one latent hole in a pattern (`proofs/strong2/RULES.md`, section 7). None is
  triggered by the engine text; all are fixed with regression tests.
