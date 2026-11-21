#!/bin/sh
# Experiment: apply patches/t15_checked_exits.patch (7 added lines) to a COPY of the pinned engine
# source and ask the tool for the whole rows T15.header and T15.account, with no exemption and no
# assumed callee contract. The engine source in this repository is not modified.
set -eu
HERE=$(cd "$(dirname "$0")/.." && pwd)
REPO=$(cd "$HERE/../.." && pwd)
REV=${REV:-08d4fd09}
NIGHTLY=${NIGHTLY:-nightly-2026-07-16}
WORK=${WORK:-/Users/khubair/agent-work/e2e-proofs-novel-1121}
LEAN_TC=${LEAN_TC:-/Users/khubair/agent-work/e2e-proofs-1121/lean-tc/lean-4.34.0-darwin_aarch64}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$WORK/tgt}
SRC=$WORK/src-$REV-t15patch
OUT=$HERE/out-t15-patch
rm -rf "$SRC"; mkdir -p "$SRC" "$OUT" "$HERE/lean/MirFlow/GenT15Patch"
git -C "$REPO" archive "$REV" Cargo.toml Cargo.lock src | tar -x -C "$SRC"
( cd "$SRC" && patch -s -p1 < "$HERE/patches/t15_checked_exits.patch" )
( cd "$SRC" && cargo +$NIGHTLY rustc -j 4 --release --lib -- -Zunpretty=mir -Zinline-mir=no > "$WORK/mir/$REV-t15patch.mir" 2>/dev/null )
python3 "$HERE/mirflow/batch.py" --mir "$WORK/mir/$REV-t15patch.mir" --src "$SRC/src/v16.rs" --out "$OUT/summaries.json" > "$OUT/batch.txt"
python3 - "$HERE" "$WORK/mir/$REV-t15patch.mir" "$OUT" "$REV" <<'PY'
import json, sys
here, mir, out, rev = sys.argv[1:5]
sys.path.insert(0, here + "/mirflow")
import extract as X
X.GEN_NS = "GenT15Patch"
summ = json.load(open(out + "/summaries.json"))["summaries"]
hdr, body = X.find_fn(X.parse_mir(mir), "close_resolved_account_not_atomic")[0]
info, lean = X.run_one(hdr, body, summ, [0, 1], [], "", "T15_patched",
                       "engine: aeyakovenko/percolator@%s WITH patches/t15_checked_exits.patch applied (NOT the engine's text)" % rev,
                       fn_mir_out=out + "/close_resolved_account_not_atomic.patched.mir")
json.dump(info, open(out + "/result.json", "w"), indent=1, sort_keys=True)
open(here + "/lean/MirFlow/GenT15Patch/T15_patched.lean", "w").write(lean)
print("open returns:", info["open_returns"], "open Ok sites:", info["open_ok_sites"], "blocks:", info["blocks"])
PY
cat > "$HERE/lean/MirFlow/AllGenT15Patch.lean" <<'L'
import MirFlow.GenT15Patch.T15_patched
#print axioms MirFlow.GenT15Patch.T15_patched.main
L
( cd "$HERE/lean" && "$LEAN_TC/bin/lake" build 2>&1 | grep -v '^info: toolchain' | tail -1 )
( cd "$HERE/lean" && "$LEAN_TC/bin/lake" env "$LEAN_TC/bin/lean" MirFlow/AllGenT15Patch.lean ) > "$OUT/axioms.txt" 2>&1
cat "$OUT/axioms.txt"
