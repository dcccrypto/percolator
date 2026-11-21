# proofs/strong2: guarded-clause certificates over rustc MIR

Sessions `e2e-lean-strong-1121b` and its continuation `e2e-lean-strong-1121c` (2026-11-21). Local Lean 4.34.0 only; no Aristotle job. It builds
on the sibling directory `proofs/novel` (checked-exit certificates) and does not edit it.

Start with `out/RESULTS.md` (what is kernel-checked on our text, `dcccrypto/percolator@c38b6b1e`)
and `RULES.md` (what is trusted). Method and its risks: `docs/ideas/e2e-strong2-method.md`.

| Path | What |
|---|---|
| `lean/ClauseFlow/Core.lean` | abstract semantics, the certificate checker, `checkCert_sound`, `call_of_cert` (axioms `propext`, `Quot.sound`) |
| `lean/ClauseFlow/Gen/` | generated modules for `c38b6b1e`: one per certified function (`S_<fn>`, theorems `main`, `row`), and `T15_*` |
| `lean/ClauseFlow/GenMut/` | mutation harness for the checker: kernel-checked verdicts on mutants of each module |
| `lean/ClauseFlow/GenR08d4fd09/`, `GenR08d4fd09Mut/` | the same for `aeyakovenko/percolator@08d4fd09` |
| `valflow/extract2.py` | TRUSTED translator `T2` (rules R1–R10) and, below the marked line, the untrusted certificate search |
| `valflow/enum_probe.py` | rule R9 table: untrusted proposer of probe functions, TRUSTED reader of their MIR (`table`, `read_probe`); `valflow/enum_probe_unit.py` attacks the reader |
| `valflow/readset.py` | TRUSTED read-set analysis of the two runtime checks (rule R10) |
| `valflow/run2.py` | untrusted driver: summaries bottom-up, ledger rows, T15 partial rows and reductions |
| `valflow/mutate.py` | mutant generator for the checker harness |
| `valflow/check_assumptions2.py` | scans behind assumptions A10–A14 and a stricter re-scan of A6/A9 |
| `valflow/negative_tests.py`, `valflow/neg_cases/` | hole-hunt: attack functions compiled into a scratch copy of the source |
| `mir/`, `mir-08d4fd09/` | MIR text of every function a theorem is about (sha256 in the Lean file) |
| `out/`, `out-08d4fd09/` | `PIN.txt`, `ENUMS.txt`, `t15-witness/` (test logs), `RESULTS.md`, `ledger_results.tsv`, `READSET.md`, `MUTANTS.md`, `NEGATIVE-TESTS.md`, `assumptions_check.txt`, `lake_build.log`, `axioms.txt`, `run.json` |
| `T15-WORKLIST.md` | what is still open for T15, with the exact obstacle for each item |
| `tests/strong2_t15_progress_only_shape.rs` (repository root) | a Rust test on the unmodified engine: witnesses against T15.header / T15.account as literally stated, from pre-states built with field pokes (reachability not shown) |

Reproduce:

```
proofs/strong2/run_strong2.sh                                               # c38b6b1e (our text)
REV=08d4fd09 ENGINE=aeyakovenko/percolator proofs/strong2/run_strong2.sh    # Toly's text
SKIP=audit_b11 python3 proofs/strong2/valflow/negative_tests.py <src-dir> <scratch> nightly-2026-07-16
cargo test -j 4 --test strong2_t15_progress_only_shape -- --nocapture --test-threads=1
```

"Kernel-checked" in the tables means: the module compiled, and `#print axioms` of its `main`
printed exactly `[propext, Quot.sound]` (`out/axioms.txt`). A theorem is about the abstract program
produced from the MIR; it is a statement about the Rust function only together with `RULES.md`
and the sibling's `METHOD-RULES.md`.
