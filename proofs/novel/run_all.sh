#!/bin/sh
# Rebuild everything from the pinned engine revision. Usage: proofs/novel/run_all.sh
# Needs: git (with the pinned commit fetched), a nightly rustc, python3, and the Lean toolchain.
# Nothing is fetched from the network. Paths can be overridden by the environment.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
REV=${REV:-08d4fd09}
NIGHTLY=${NIGHTLY:-nightly-2026-07-16}
WORK=${WORK:-/Users/khubair/agent-work/e2e-proofs-novel-1121}
LEAN_TC=${LEAN_TC:-/Users/khubair/agent-work/e2e-proofs-1121/lean-tc/lean-4.34.0-darwin_aarch64}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$WORK/tgt}
# VARIANT=default: the deployed build. VARIANT=audit-scan: built with --features audit-scan
# (the build the fuzzers run), reported separately and never mixed with the default tables.
VARIANT=${VARIANT:-default}
SRC=$WORK/src-$REV
if [ "$VARIANT" = default ]; then
  OUT=$HERE/out; GENNS=Gen; FEAT=""; SCAN=""; MIRDIR=$HERE/mir; TAG=$REV
else
  OUT=$HERE/out-$VARIANT; GENNS=GenAuditScan; FEAT="--features $VARIANT"; SCAN="--scan-rows"; MIRDIR=$HERE/mir-$VARIANT; TAG=$REV-$VARIANT
fi

mkdir -p "$SRC" "$WORK/mir" "$OUT" "$MIRDIR" "$HERE/lean/MirFlow/$GENNS"
rm -rf "$SRC/src"
git -C "$REPO" archive "$REV" Cargo.toml Cargo.lock src | tar -x -C "$SRC"
V16=$(git -C "$REPO" rev-parse "$REV:src/v16.rs")
WM=$(git -C "$REPO" rev-parse "$REV:src/wide_math.rs")
LIB=$(git -C "$REPO" rev-parse "$REV:src/lib.rs")
RUSTC=$(rustc +$NIGHTLY --version)
FLAGS="cargo rustc -j 4 --release --lib $FEAT -- -Zunpretty=mir -Zinline-mir=no"
{
  echo "revision: $(git -C "$REPO" rev-parse "$REV")"
  echo "src/v16.rs blob: $V16"
  echo "src/wide_math.rs blob: $WM"
  echo "src/lib.rs blob: $LIB"
  echo "rustc: $RUSTC"
  echo "mir command: $FLAGS (variant: $VARIANT)"
  echo "lean: $("$LEAN_TC/bin/lean" --version)"
} > "$OUT/PIN.txt"

( cd "$SRC" && cargo +$NIGHTLY rustc -j 4 --release --lib $FEAT -- -Zunpretty=mir -Zinline-mir=no > "$WORK/mir/$TAG.mir" 2>/dev/null )
( cd "$SRC" && cargo +$NIGHTLY rustc -j 4 --release --lib $FEAT -- -Zunpretty=mir -Zinline-mir=no -Zmir-include-spans=yes > "$WORK/mir/$TAG.spans.mir" 2>/dev/null )
echo "full MIR dump sha256: $(shasum -a 256 "$WORK/mir/$TAG.mir" | cut -d' ' -f1) ($(wc -l < "$WORK/mir/$TAG.mir" | tr -d ' ') lines)" >> "$OUT/PIN.txt"

python3 "$HERE/mirflow/check_assumptions.py" "$SRC" > "$OUT/assumptions_check.txt"

rm -f "$HERE"/lean/MirFlow/$GENNS/*.lean "$MIRDIR"/*.mir
python3 "$HERE/mirflow/ledger_run.py" --mir "$WORK/mir/$TAG.mir" --spans-mir "$WORK/mir/$TAG.spans.mir" \
  --src "$SRC/src/v16.rs" --ledger "$HERE/ledger/proof_ledger_08d4fd09.tsv" \
  --gen "$HERE/lean/MirFlow/$GENNS" --mirdir "$MIRDIR" --out "$OUT" --genns $GENNS $SCAN \
  --pin "engine: aeyakovenko/percolator@$REV, src/v16.rs blob $V16; $RUSTC; $FLAGS"

( cd "$HERE/lean" && "$LEAN_TC/bin/lake" build 2>&1 | grep -v '^info: toolchain' > "$OUT/lake_build.log"; \
  tail -1 "$OUT/lake_build.log" )
( cd "$HERE/lean" && "$LEAN_TC/bin/lake" env "$LEAN_TC/bin/lean" MirFlow/All$GENNS.lean > "$OUT/axioms.txt" 2>&1 || true )
python3 "$HERE/mirflow/make_results.py" "$OUT" $GENNS > "$OUT/RESULTS.md"
python3 "$HERE/mirflow/frontier.py" --mir "$WORK/mir/$TAG.mir" --src "$SRC/src/v16.rs" \
  --fn close_resolved_account_not_atomic --check 0 > "$OUT/T15-header-frontier.md"
echo "done: see $OUT/RESULTS.md"
