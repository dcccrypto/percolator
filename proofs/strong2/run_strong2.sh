#!/bin/sh
# Rebuild the strong2 results from a pinned engine revision. Usage: proofs/strong2/run_strong2.sh
# Needs: git (with the revision fetched), the pinned nightly rustc, python3, the Lean toolchain.
# Nothing is fetched from the network; no Aristotle call. Paths can be overridden by the environment.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
REV=${REV:-c38b6b1e}
ENGINE=${ENGINE:-dcccrypto/percolator}
NIGHTLY=${NIGHTLY:-nightly-2026-07-16}
WORK=${WORK:-/Users/khubair/agent-work/e2e-proofs-1121/scratch/s2work}
LEAN_TC=${LEAN_TC:-/Users/khubair/agent-work/e2e-proofs-1121/lean-tc/lean-4.34.0-darwin_aarch64}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/Users/khubair/agent-work/e2e-proofs-1121/tgt-strong2-mir}
SRC=$WORK/src-$REV
if [ "$REV" = c38b6b1e ]; then OUT=$HERE/out; GENNS=Gen; MIRDIR=$HERE/mir; SIB=out-c38b6b1e
else OUT=$HERE/out-$REV; GENNS=GenR$REV; MIRDIR=$HERE/mir-$REV; SIB=out; fi
MUT=${MUT---mutants}
GENDIR=$HERE/lean/ClauseFlow/$GENNS
mkdir -p "$SRC" "$WORK/mir" "$OUT" "$MIRDIR" "$GENDIR"
git -C "$REPO" archive "$REV" Cargo.toml Cargo.lock src | tar -x -C "$SRC"
V16=$(git -C "$REPO" rev-parse "$REV:src/v16.rs")
RUSTC=$(rustc +$NIGHTLY --version)
FLAGS="cargo rustc -j 4 --release --lib -- -Zunpretty=mir -Zinline-mir=no"
{
  echo "engine: $ENGINE"
  echo "revision: $(git -C "$REPO" rev-parse "$REV")"
  echo "src/v16.rs blob: $V16"
  echo "src/wide_math.rs blob: $(git -C "$REPO" rev-parse "$REV:src/wide_math.rs")"
  echo "src/lib.rs blob: $(git -C "$REPO" rev-parse "$REV:src/lib.rs")"
  echo "rustc: $RUSTC"
  echo "mir command: $FLAGS"
  echo "lean: $("$LEAN_TC/bin/lean" --version)"
} > "$OUT/PIN.txt"
( cd "$SRC" && cargo +$NIGHTLY rustc -j 4 --release --lib -- -Zunpretty=mir -Zinline-mir=no > "$WORK/mir/$REV.mir" 2>/dev/null )
( cd "$SRC" && cargo +$NIGHTLY rustc -j 4 --release --lib -- -Zunpretty=mir -Zinline-mir=no -Zmir-include-spans=yes > "$WORK/mir/$REV.spans.mir" 2>/dev/null )
echo "full MIR dump sha256: $(shasum -a 256 "$WORK/mir/$REV.mir" | cut -d' ' -f1) ($(wc -l < "$WORK/mir/$REV.mir" | tr -d ' ') lines)" >> "$OUT/PIN.txt"

# assumptions A1..A9 of the sibling method (its script, unchanged) and A10..A12 of this one
python3 "$HERE/../novel/mirflow/check_assumptions.py" "$SRC" "$WORK/mir/$REV.mir" > "$OUT/assumptions_check.txt"
( cd "$SRC" && cargo +$NIGHTLY rustc -j 4 --release --lib -- -Zunpretty=expanded > "$WORK/mir/$REV.expanded.rs" 2>/dev/null )
python3 "$HERE/valflow/check_assumptions2.py" "$SRC" "$WORK/mir/$REV.mir" "$WORK/mir/$REV.expanded.rs" >> "$OUT/assumptions_check.txt"

# rule R9: the enum table is read from the MIR of probe functions compiled in a scratch copy
# (pinned source + probes appended to src/v16.rs); the analysed MIR above is of the pinned source alone
PROBE=$WORK/probe-$REV
rm -rf "$PROBE"; mkdir -p "$PROBE/src"
cp "$SRC/Cargo.toml" "$SRC/Cargo.lock" "$PROBE/"; cp "$SRC"/src/*.rs "$PROBE/src/"
python3 "$HERE/valflow/enum_probe.py" gen "$SRC/src/v16.rs" "$PROBE/src/v16.rs"
( cd "$PROBE" && cargo +$NIGHTLY rustc -j 4 --release --lib -- -Zunpretty=mir -Zinline-mir=no > "$WORK/mir/$REV.probe.mir" 2>"$WORK/mir/$REV.probe.err" )
python3 "$HERE/valflow/enum_probe.py" table "$WORK/mir/$REV.probe.mir" "$PROBE/src/v16.rs" > "$OUT/ENUMS.txt"
tail -1 "$OUT/ENUMS.txt"

# generated files are rewritten by run2.py; stale modules of an earlier run are removed by it
python3 "$HERE/valflow/run2.py" --mir "$WORK/mir/$REV.mir" --spans-mir "$WORK/mir/$REV.spans.mir" \
  --src "$SRC/src/v16.rs" --enum-probe-mir "$WORK/mir/$REV.probe.mir" --enum-probe-src "$PROBE/src/v16.rs" --gen "$GENDIR" --mirdir "$MIRDIR" --out "$OUT" --genns $GENNS --clean $MUT --sibling "$HERE/../novel/$SIB/ledger_results.tsv" \
  --pin "engine: $ENGINE@$REV, src/v16.rs blob $V16; $RUSTC; $FLAGS" > "$OUT/run.txt"
cat "$OUT/run.txt"
( cd "$HERE/lean" && PATH="$LEAN_TC/bin:$PATH" lake build 2>&1 | grep -v '^info: toolchain' > "$OUT/lake_build.log"; tail -1 "$OUT/lake_build.log" )
( cd "$HERE/lean" && PATH="$LEAN_TC/bin:$PATH" lake env lean ClauseFlow/All$GENNS.lean > "$OUT/axioms.txt" 2>&1 || true )
python3 "$HERE/valflow/make_results2.py" "$OUT" $GENNS > "$OUT/RESULTS.md"
echo "done: see $OUT/RESULTS.md"
