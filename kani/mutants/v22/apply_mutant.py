#!/usr/bin/env python3
"""apply_mutant.py <manifest.tsv> <mutant id> <repo copy dir>: applies ONE mutant to a COPY of the
repo (a dedicated mutant worktree built from the proof commit; never the release tree, never
`git stash`). Fails unless the original text occurs exactly once."""
import sys, os
man, mid, root = sys.argv[1:4]
un = lambda s: s.replace("\\n", "\n").replace("\\t", "\t").replace("\\\\", "\\")
for line in open(man).read().splitlines()[1:]:
    f = line.split("\t")
    if f[0] != mid: continue
    path = os.path.join(root, f[1]); orig, rep = un(f[3]), un(f[4])
    s = open(path).read()
    assert s.count(orig) == 1, f"{mid}: original text not unique/absent in {path}"
    open(path, "w").write(s.replace(orig, rep)); print(f"applied {mid} to {path}"); sys.exit(0)
sys.exit(f"no mutant {mid}")
