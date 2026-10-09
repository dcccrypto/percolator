#!/usr/bin/env python3
"""Builds engine_core.tsv: one row per pre-planned mutant (design rev2 R4.5 + rev2.1).
Each row: id, file, line(s), original (exact text, \\n-escaped), replacement, target harness,
expected kill. The original is the exact text of the cited lines, widened upward by whole lines
until it occurs exactly once in the file. Apply with apply_mutant.py on a COPY (never stash)."""
import sys, os
F = "src/v16.rs"
src = open(F).read()
L = src.split("\n")
rows = []
def mk(mid, a, b, fn, target, kill):
    lo = a
    while True:
        orig = "\n".join(L[lo-1:b])
        if src.count(orig) == 1: break
        lo -= 1
        assert lo > 0
    rep = fn(orig)
    assert rep != orig, mid
    rows.append((mid, F, f"{lo}-{b}" if lo != b else str(b), orig, rep, target, kill))
def sub(old, new):
    def f(t):
        assert t.count(old) >= 1, (old, t[-200:])
        i = t.rfind(old)
        return t[:i] + new + t[i+len(old):]
    return f
M = "proof_v22_s10_move_conserves_bounds_exact_total(_m)"
mk("S10-M1", 15121, 15121, sub("|| b_src.expiry_slot <= now", "|| false"), M, "totality assert r.is_ok() on the lapsed-source state")
mk("S10-M2", 15127, 15128, lambda t: t.replace("!self.loss_domain_accepts_realized_backing(dst)?", "false").replace("|| self.backing_bucket_for_domain(dst)?.status == BackingBucketStatusV16::Impaired", "|| false"), M, "totality assert on the dst-Impaired state")
mk("S10-M3", 15103, 15103, sub(") && no_loser_cash(", ") || no_loser_cash("), M, "E-S10-6 exactness (loser cash in one domain only must still move)")
mk("S10-M4", 15120, 15120, sub("|| b_src.status != BackingBucketStatusV16::Fresh", "|| false"), M, "totality assert (MAY BE EQUIVALENT: classify if it survives, do not chase)")
mk("S10-E1", 5743, 5743, sub("= true;", "= false;"), M, "exactness/bound fr'(src) >= min(fr, pp) with consumed > 0")
mk("S10-E4", 5757, 5757, sub("let next = if add {", "let next = if !add {"), "proof_v22_s10_mirror_setter", "field' == field + d")
mk("S10-N2", 5760, 5760, sub("c.get().saturating_sub(delta_num)", "c.get().checked_sub(delta_num).ok_or(V16Error::CounterUnderflow)?"), "proof_v22_s10_mirror_setter", "subtract asserts Ok (saturation cover)")
mk("S10-F1", 15143, 15143, sub("if moved < S10_MIN_MOVE_ATOMS * BOUND_SCALE {", "if moved == 0 {"), M, "exactness: 0 < x < MIN*BS must move 0")
mk("S10-F2", 15142, 15142, sub("/ BOUND_SCALE * BOUND_SCALE;", ";"), M, "x % BS == 0 / exactness")
mk("S10-B1", 15149, 15149, sub("*s10_budget -= 1;", "/* mutant S10-B1 */"), M, "budget' == budget - 1")
mk("S10-X1", 15134, 15137, sub(".saturating_sub(s_src.positive_claim_bound_num),", ".saturating_sub(0),"), M, "av'(src) >= cl(src) / exactness")
mk("S10-X2", 15139, 15141, sub(".saturating_sub(V16Core::available_backing_num_for_source_credit_state(s_dst)?);", ".saturating_sub(0);"), M, "av'(dst) <= cl(dst) / exactness")
mk("S10-O1", 15142, 15142, sub("/ BOUND_SCALE * BOUND_SCALE;", "/ BOUND_SCALE * BOUND_SCALE + BOUND_SCALE;"), "proof_v22_s10_move_is_idempotent_m", "second call moves nothing / bound")
mk("S10-H1", 18272, 18273, lambda t: t.replace("            && asset.stored_pos_count_long == 0\n            && asset.stored_pos_count_short == 0", ""), "proof_v22_s10_retry_hook_guard_m", "stored-count skip => identity")
mk("S10-R1", 10207, 10210, lambda t: t.replace("            && bucket.valid_liened_backing_num == 0\n            && bucket.consumed_liened_backing_num == 0\n            && bucket.impaired_liened_backing_num == 0", ""), "proof_v22_s10_wholly_empty_reset", "pp unchanged when not wholly empty")
mk("S10-G1", 18175, 18177, sub("            s10_grant\n", "            0\n"), "proof_v22_s10_grant_only_refresh_moves", "cover 'Refresh with a grant moves' unsatisfied")
mk("S10-G2", 18175, 18175, sub("if matches!(request.action, PermissionlessCrankActionV16::Refresh) {", "if true {"), "proof_v22_s10_grant_only_refresh_moves", "EXPECTED TO SURVIVE (10-09 ruling); a kill goes back to the reviewer")
mk("X1-M1", 23152, 23153, sub('#[cfg(not(feature = "x1-mutant-noclamp"))]\n                    self.clamp_kf_pending_credit_to_claims(domain)?;', '// mutant X1-M1: clamp removed'), "proof_v22_x1_fee_refinement_ok_state", "kf_pending_credit differs on the F2 cover")
mk("X1-M2", 23128, 23128, sub("key > last_key", "key < last_key"), "proof_v22_x1_fee_refinement_ok_state", "loss_stale_active differs (2-asset cover)")
mk("X1-M3", 23163, 23165, sub("        account.header.health_cert.valid = 0;\n", "        // mutant X1-M3\n"), "proof_v22_x1_fee_refinement_ok_state", "account differs")
mk("X1-M4", 23115, 23117, sub("        if decode_market_mode(self.header.mode)? != MarketModeV16::Live {\n            return Err(V16Error::LockActive);\n        }", "        // mutant X1-M4"), "proof_v22_x1_fee_refinement_result", "non-Live: results differ")
mk("X1-M5", 14706, 14706, sub("v.max(0) as u128", "v as u128"), "proof_v22_pending_credit_reader_clamped", "negative stored cover")
mk("X1-M6", 14706, 14706, sub("Ok(core::cmp::min(v.max(0) as u128, claims))", "Ok(v.max(0) as u128)"), "proof_v22_pending_credit_reader_clamped", "above-claims cover")
mk("CAP-M1", 6015, 6021, sub("            if slot >= active_leg_cap {", "            if false {"), "proof_v22_leg_cap_validator", "non-empty leg at slot cap accepted")
mk("CAP-M2", 22220, 22230, sub("        if requests.len() > config.max_portfolio_assets as usize {", "        if false {"), "proof_v22_batch_len_refused_fork_threshold", "len == cap+1: Err(InvalidConfig) and no mutation")
mk("CAP-M3", 22337, 22346, sub("        if requests.len() > config.max_portfolio_assets as usize {", "        if false {"), "proof_v22_batch_len_refused_with_fee", "len == cap+1: Err(InvalidConfig) and no mutation")
mk("ADL-M1", 1133, 1135, sub("        let mut asset = asset;\n", "        let mut asset = asset;\n        leg.a_basis = match leg.side { SideV16::Long => asset.a_long, SideV16::Short => asset.a_short };\n"), "proof_v22_attached_legs_have_unit_a", "resize keeps a scaled a_basis")
P = "proof_v22_w4_postcondition_predicate"
mk("W4-E1", 28697, 28697, sub("vault_after == vault_before", "true"), P, "vault clause alone false => predicate true")
mk("W4-E2", 28698, 28698, sub("&& insurance_after.checked_sub(insurance_before) == Some(total)", "&& true"), P, "insurance clause alone false")
mk("W4-E3", 28699, 28699, sub("&& capital_after <= capital_before", "&& true"), P, "capital clause alone false")
mk("W4-E4", 28700, 28700, sub("&& c_tot_after <= c_tot_before", "&& true"), P, "c_tot clause alone false")
mk("W4-E5", 28701, 28701, sub("&& capital_before - capital_after == c_tot_before - c_tot_after", "&& true"), P, "equal-fall clause alone false")
mk("W4-I", 28698, 28698, sub("&& insurance_after.checked_sub(insurance_before) == Some(total)", "&& true"), "proof_v22_w4_insurance_credit_branch_refused", "insurance-credit branch returns Ok(t > 0)")
mk("W4-L", 23536, 23536, sub("|| Self::account_has_source_liens(account)", ""), "proof_v22_w4_repay_engine_level", "liened => Err")
mk("W4-C", 23585, 23585, sub("if capacity == 0 || total > capacity {", "if capacity == 0 {"), "proof_v22_w4_repay_engine_level", "t <= capacity")
mk("W4-V", 23524, 23525, sub("if decode_market_mode(self.header.mode)? != MarketModeV16::Live\n            || decode_bool", "if decode_bool"), "proof_v22_w4_repay_engine_level", "Resolved => Err")
mk("W4-S", 23606, 23608, sub("        if amount_b != 0 {\n            self.charge_account_backing_fee_not_atomic(account, domain_b, 0, domain_b, amount_b)?;\n        }", "        // mutant W4-S: second sweep leg skipped"), "proof_v22_w4_repay_engine_level", "cover 'both sweep legs non-zero and Ok' unsatisfied")
mk("REM-M1", 27331, 27331, sub(".checked_mul(ADL_ONE)?;", ".checked_mul(ADL_ONE)?.checked_add(1)?;"), "proof_v22_rem_fast_equals_wide", "fast != wide")
mk("REM-M2", 27327, 27327, sub("if reduced_remainder < 0 {", "if false {"), "proof_v22_rem_fast_equals_wide", "negative-quotient cover differs")
mk("REM-M3", 27307, 27307, sub("let reduced_carry = if carry == 0 {", "let reduced_carry = if true {"), "proof_v22_rem_partition_invariance", "q == q1 + q2")
mk("REM-M4", 15445, 15445, sub("if leg.k_rem_num >= den || leg.f_rem_num >= den {", "if false {"), "proof_v22_rem_settle_keeps_rem_below_den", "rem == den => Err(InvalidLeg)")
mk("REM-M6", 1058, 1058, sub(".checked_mul(2)?", ".checked_mul(1)?"), "proof_v22_kf_hidden_loss_bound", "exact formula (2-leg rounding witness)")
mk("S9-M1", 3834, 3842, sub("core::cmp::max(state.credit_rate_num, rate),", "rate,"), "proof_v22_s9_protective_rate", "result >= stored")
mk("S9-M2", 3832, 3833, sub("        if denominator == 0 || available >= denominator {\n            return Ok(CREDIT_RATE_SCALE);", "        if denominator == 0 {\n            return Ok(0);\n        }\n        if available >= denominator {\n            return Ok(CREDIT_RATE_SCALE);"), "proof_v22_s9_protective_rate", "pending >= claims => SCALE")
# REM-M5: attach starts at a non-zero remainder
i = next(k for k in range(1398, 1480) if L[k-1].strip() == "k_rem_num: 0,")
mk("REM-M5", i, i, sub("k_rem_num: 0,", "k_rem_num: 1,"), "proof_v22_rem_attach_resize", "attach k_rem == 0")
esc = lambda s: s.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n")
with open("kani/mutants/v22/engine_core.tsv", "w") as o:
    o.write("id\tfile\tline\toriginal\treplacement\ttarget\texpected_kill\n")
    for r in rows:
        o.write("\t".join(esc(str(x)) for x in r) + "\n")
print(len(rows), "rows")
