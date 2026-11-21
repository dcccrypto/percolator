# proofs/novel: checked-exit certificates over rustc MIR ("mirflow")

A parallel track to the Aristotle work in `proofs/aristotle/`. Method and scoring:
[docs/ideas/e2e-novel-proof-method.md](../../docs/ideas/e2e-novel-proof-method.md).
What is trusted and the exact translation rules: [METHOD-RULES.md](METHOD-RULES.md).
Results and their limits: [NOVEL-REPORT.md](../../NOVEL-REPORT.md) at the repository root.

Target text: `aeyakovenko/percolator@08d4fd09`, `src/v16.rs` blob `a4674830` (pin in
`out/PIN.txt`). The engine source in this repository is not modified by anything here.

## Reproduce

```
proofs/novel/run_all.sh                      # deployed build: tables in out/, Lean in lean/MirFlow/Gen
VARIANT=audit-scan proofs/novel/run_all.sh   # audit-scan build: out-audit-scan/, lean/MirFlow/GenAuditScan
proofs/novel/experiments/t15_patch.sh        # T15 on a patched copy: out-t15-patch/, lean/MirFlow/GenT15Patch
python3 proofs/novel/mirflow/mutation_tests.py <src dir> <scratch dir> nightly-2026-07-16
```

Each run takes about 15 seconds here (the mutation tests about 70). They need the pinned commit
in the local git object store, a nightly `rustc` (for `-Zunpretty=mir`), `python3`, and Lean
4.34.0; paths are environment variables at the top of `run_all.sh`. No network, no credits.

## Layout

| Path | What |
|---|---|
| `lean/MirFlow/Core.lean` | The abstract semantics, the certificate checker, `checkCert_sound`, `call_of_cert`. Hand-written, 700 lines, axioms `propext`, `Quot.sound`. |
| `lean/MirFlow/Gen/*.lean` | Generated, deployed build: one module per function (`S_<fn>`), plus four for T15. Each has the abstract program, the certificate, `theorem cert := by decide +kernel`, and `theorem main`. |
| `lean/MirFlow/GenAuditScan/` | The same for the `audit-scan` build. |
| `lean/MirFlow/GenT15Patch/` | T15 on the patched copy. |
| `mirflow/extract.py` | MIR text → abstract program (trusted) and certificate (untrusted) → Lean. |
| `mirflow/batch.py` | Callee summaries, bottom-up from the empty table. |
| `mirflow/ledger_run.py` | Runs over the `T` rows of the ledger; T15 exemptions and reductions. |
| `mirflow/frontier.py` | Reduces T15.header to leaf obligations (a map, not a proof). |
| `mirflow/check_assumptions.py` | Text scans behind assumptions A1–A4. |
| `mirflow/mutation_tests.py` | 18 mutations of T14; the tool must refuse the broken ones. |
| `mir/`, `mir-audit-scan/` | The MIR text of every function a theorem is about (sha256 in each Lean header). |
| `ledger/proof_ledger_08d4fd09.tsv` | Copy of Toly's ledger (blob `7c9b7688`), read only for row ids and function names. |
| `out/RESULTS.md` | Row-by-row table, deployed build. `out/axioms.txt`, `out/lake_build.log`: Lean's output. |
| `out/MUTATION-TESTS.md`, `out/assumptions_check.txt`, `out/T15-header-frontier.md` | As named. |
| `patches/t15_checked_exits.patch`, `out-t15-patch/` | The experiment of section 6 of the report. |
| `AUDIT-EXTRACTOR.md` | Independent adversarial review of the trusted translation and what was done about it. |
| `NOTES.md` | Progress notes with times. |
