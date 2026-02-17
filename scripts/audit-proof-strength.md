# Kani Proof Strength Audit Prompt

Use this prompt to analyze Kani proof harnesses for weakness, vacuity, or collapse into unit tests.

---

Analyze each Kani proof harness for weakness. For every proof, determine:

1. **Input classification**: Is each input to the function-under-test concrete (hardcoded),
   symbolic (kani::any with kani::assume bounds), or derived (computed from other inputs)?
   A proof where ALL function inputs are concrete is a unit test, not a proof.

2. **Branch coverage**: Read the function-under-test and list every conditional branch
   (if/else, match arms, min/max, saturating ops that could clamp). For each branch,
   determine whether the proof's input constraints ALLOW the solver to reach both sides.
   Flag any branch that is locked to one side by concrete values or overly tight assumes.

3. **Invariant strength**: What does the proof actually assert?
   - valid_state() is weaker than canonical_inv() — flag proofs that use the weaker check
     when canonical_inv exists.
   - Post-condition assertions (like "pnl >= 0") without the full invariant are incomplete.
   - Assertions gated behind `if result.is_ok()` without a non-vacuity check on the Ok path
     may be vacuously true if the solver always takes the Err path.

4. **Vacuity risk**: Can the solver satisfy all kani::assume constraints AND reach the
   assertions? Watch for:
   - Contradictory assumes that make the proof trivially true
   - assume(canonical_inv(...)) on a hand-built state that might not satisfy it
   - assert_ok! on a path that might always error given the constraints

5. **Symbolic collapse**: Even with kani::any(), check if derived values collapse the
   symbolic range. Example: if vault = capital + insurance + pnl, and pnl is symbolic,
   but capital and insurance are concrete and large, the haircut ratio h may always be 1,
   never exercising the h < 1 branch.

For each proof, output:
- **STRONG**: symbolic inputs exercise all branches, canonical_inv checked, non-vacuous
- **WEAK**: symbolic inputs but misses branches or uses weaker invariant (list which)
- **UNIT TEST**: concrete inputs, single execution path
- **VACUOUS**: assertions may never be reached

Include specific recommendations to strengthen any non-STRONG proof.
