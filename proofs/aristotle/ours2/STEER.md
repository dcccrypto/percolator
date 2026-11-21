# STOP. CHANGE OF PLAN (04:02 GMT, Khubair). Read this before writing more slice code or submitting.

Do not kill your work; pivot. Keep using this worktree and branch only. The credit rules are unchanged: submit only through `/Users/khubair/agent-work/e2e-proofs-1121/aristo.sh` (refuses below 120 free or if any credits were purchased); the watcher cancels below 30; never buy; never read the key yourself.

The "use Toly's version" rule now applies to the stronger proof. Toly (aeyakovenko/percolator @ `08d4fd09`) already runs Aristotle. Reuse his statements, prompt style, submit flow and revision-comparison method instead of our gen_slice2 framing where they overlap.

## Where to read (local, read-only)

A detached worktree is already at:

`/Users/khubair/agent-work/toly-percolator-08d4fd09`

HEAD is `08d4fd09`. You may also `git show 08d4fd09:proofs/...` from this repo (upstream is already fetched). **Do not modify that worktree, do not edit his `proofs/aristotle` (or anything else there), and do not push anything to his repo (no `git push upstream`, no writes to aeyakovenko).** Copy what you need into `proofs/aristotle/ours2/` on our branch.

Read first, in this order:

- `proofs/ROADMAP.md` — 181/624 pieces done (171 Aristotle, 10 Kani); 0/89 charter invariants closed. The trade, full refresh and full resolved close are recorded as not proved / beyond Kani's budget. Stateful Aristotle attempts: 3 of 32 properties, mostly because of the byte-level 9,419-byte account model. He lists three ways forward; prefer his first (pure kernels over small records, short read-call-write shell) where it applies, and say so.
- `proofs/proof_ledger.tsv` and `proofs/proof_ledger.md`
- `proofs/prompts/` (style: `stage_*.txt`, especially `stage_s2_trade_pilot.txt`, `stage_a_wide_math.txt`)
- `proofs/aristotle/statements/Spec/*.lean` (read only)
- `proofs/submit_batch.sh`, `proofs/watch_run.sh` — prompt style and continue-on-same-project flow. Our submit path stays `aristo.sh` (new project is fine; we are not continuing his Aristotle project).
- `proofs/changed_fns.py` + `proofs/rs_scan.py` — use this method to compare our pinned engine `c38b6b1e` (`src/v16.rs`, `src/wide_math.rs`) against his revisions (`add1d027`, `c030aa40`, `d1fa39f6`, and `08d4fd09`). A proof made against his text still stands for ours only when the function, its reached callees, and the types it names are unchanged. Run the script; do not re-implement it. Copy the scripts into `ours2/tools/` if you need to run them here.
- `proofs/kani_gap_map.py` / `specs/kani_gap_map_*.md` if present.

## What to aim at

1. **Primary:** the full resolved close, the exact gap ROADMAP lists as not proved. Map every property to his ledger row ids. The close entry point is **T15** (`close_resolved_account_not_atomic`: T15.header, T15.scan, T15.account, T15.mirror). Related: **T14** (`claim_resolved_payout_topup_not_atomic`). Also any P-row whose call site is on this path (e.g. **P15** on `resolved_receipt_claimable_against_ledger`, which rests on **F3**). If a property we care about has no row, say so and do not invent an id.
2. **Include F3** (`wide_mul_div_floor_u128`) in the job, but first run `changed_fns.py` (and a direct diff) of our `src/wide_math.rs` at `c38b6b1e` against `add1d027`. If the function and its callees are unchanged, his F3 already covers our text: tag F3 as **proved (upstream Aristotle, add1d027; confirmed identical text)** and do not spend credits re-proving it. If they changed, tag it **assumed** or submit it.
3. **Then, only if credits allow** (after the close job, still above the 120 floor, 0 purchased): the full refresh, ledger **T37** (`full_account_refresh_not_atomic`: T37.header/scan/account/mirror). Do not start the trade (T31 / TA.*) on this budget; ROADMAP already records the trade pilot (1 of 21, ~410 credits).
4. **Tag every result** with the matching `proof_ledger` / ROADMAP row and mark it **proved**, **assumed**, or **not attempted**. This includes run 1's RC1–RC7: map them if they correspond, and mark them as proofs of the transcription (see `../ours/SLICE-DIFF.md`), not of T15.

## What this means for gen_slice2

Stop spending time on a from-scratch framing that duplicates his. Reuse his Spec statements and prompt wording for T15/T14/F3/T37. If a kernel-over-small-records cut is the only affordable way to get a close-path result inside ~245 free credits (his own conclusion after 3/32 at byte level), do that, but keep every copied kernel byte-identical to `c38b6b1e`, generate it, and state clearly which ROADMAP rows a kernel proof does and does not close (a kernel proof is not a T15.header proof). You may keep useful work already in `ours2/` if it still serves this. Do not weaken claims.

Write `ours2/LEDGER-MAP.md`: each property, the Toly row id, status (proved / assumed / not attempted), evidence, and our commit SHA.

The auditor session still waits on `AUDIT-READY`. Leave it until the slice and statements follow this plan, then write that file with the commit SHA.

Still: no PR, nothing on main, nothing upstream, post nothing, no Toly reply, docs in ours2/README.md and JOB.md as before, plus the ledger map. Push only to `agent/J-20261121-ENG-E2E-PROOFS`.

When you have read this file, write a one-line file `ours2/STEER-ACK.md` containing: the UTC time, "accepted", and the first action you will take (e.g. "running changed_fns.py on wide_math and close_resolved_account_not_atomic").
