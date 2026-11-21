# Progress notes (session e2e-lean-fable-1121, branch agent/J-20261121-ENG-E2E-PROOFS-NOVEL)

Times are `date -u` output at the moment of writing.

- 04:42 start. Read ROADMAP, ledger rows T14/T15, s2.md at 08d4fd09. The close-path functions exist
  in Toly's text at 08d4fd09 (`src/v16.rs:22905`, `:22947`), so that revision is the target, not c38b6b1e.
- 04:44 Probed rustc MIR (`-Zunpretty=mir -Zinline-mir=no`, nightly-2026-07-16) for T14: 26 blocks, the
  `?` operator is an explicit `Try::branch` + `switchInt`. Feasible.
- 04:46 Method doc committed and pushed (a8c79cbd) before any proof.
- 04:53 `MirFlow/Core.lean`: semantics, checker, soundness theorem compile; axioms `[propext, Quot.sound]`.
- 04:55 First T14 certificate rejected by Lean (all returns merge in one block; the join lost the
  link between "Ok" and "checked"). Fixed in the checker, not in the CFG: conditional facts
  ("if this local is Ok then the checks hold") and a weakening rule, re-proved sound.
- 04:57 T14 (both checks) kernel-checked. T15: one shared return block, so exemptions became
  "executions that avoid these blocks" in the Lean semantics (re-proved).
- 04:58 T15 open Ok sites found by the tool: bb22, bb143, bb152 (header), plus bb77 (account).
  Span-annotated MIR maps them to v16.rs:22970, 23037, 23040, 23006: the same returns the ledger
  and our earlier hand reading name.
- 05:01 Bottom-up callee summaries over all 334 market methods: 178 get a summary, 2 not translated.
- 05:03 `run_all.sh`: 44 Lean modules, build completed, 18.5 s wall for the whole pipeline.
  32 header rows and 11 account rows of the ledger kernel-checked; T15.header reduced to 6 named
  callee contracts. T18 (`[final]` in the ledger) is NOT certified: its check is skipped when
  `converted == 0`, which needs a value fact, as the ledger's own note says.
- 05:05 TB.checked (trade pilot row, "attempted, open") is the statement the tool proves for
  `execute_batch_with_fee_loss_stale_scoped_not_atomic` (est checks 0, 1, 2).
- 05:07 Mutation tests: 18 mutations of T14, 0 unsound answers. Independent adversarial audit of
  the extractor started (separate agent, read-only).
