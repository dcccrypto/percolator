import Spec.ResolvedCloseModel

/-!
# Resolved close never overpays, for any number of accounts and any close order

STATEMENTS ONLY: every theorem ends in `sorry`. Nothing here is proved. The file typechecks
(with the `sorry` warnings) against Lean core, toolchain in `../lean-toolchain`.

Labels RC1..RC6 are the ones used in `../../prompts/stage_e2e_rc_resolved_close_no_overpay.txt`
and in `project/aristotle/resolved_close_spec.md`.
-/

namespace Spec.ResolvedClose

/-- RC1 (the arithmetic core). At the ledger's rate the receipts are together owed at most the
snapshot residual: `Σ floor(face_i * num / den) ≤ snapshot_residual`, for any number of
receipts. Hence what has been paid so far is at most the residual too. -/
theorem RC1_receipts_owed_at_most_residual (s : St) (h : Inv s) :
    sumGross s.pool.ledger s.accts ≤ s.pool.ledger.residual ∧
    sumPaid s.accts ≤ sumGross s.pool.ledger s.accts := by
  sorry

/-- RC2 (one close). If the close of account `i` returns, then: the invariant still holds; the
vault dropped by exactly the payout; the payout is the account's whole capital plus exactly what
was added to its receipt (so the `min(.., vault)` cap did not cut the payout: no capital is
written off unpaid); the account is left with no capital and no pnl; if it has a receipt, the
receipt is now paid up to its gross amount at the rate; and the rate, the residual and the
insurance are unchanged. -/
theorem RC2_close_step (s s' : St) (i payout : Nat) (h : Inv s)
    (hstep : closeStep s i = some (s', payout)) :
    Inv s' ∧
    s'.pool.vault + payout = s.pool.vault ∧
    payout + sumPaid s.accts = (s.accts[i]?.getD ⟨0, 0, Receipt.empty⟩).capital + sumPaid s'.accts ∧
    s'.pool.cTot + (s.accts[i]?.getD ⟨0, 0, Receipt.empty⟩).capital = s.pool.cTot ∧
    s'.accts.length = s.accts.length ∧
    (∀ a', s'.accts[i]? = some a' →
        a'.capital = 0 ∧ a'.pnl = 0 ∧
        (a'.receipt.present = true → a'.receipt.paid = gross a'.receipt s'.pool.ledger)) ∧
    (∀ j, j ≠ i → s'.accts[j]? = s.accts[j]?) ∧
    s'.pool.insurance = s.pool.insurance ∧
    s'.pool.ledger.residual = s.pool.ledger.residual ∧
    s'.pool.ledger.rateNum = s.pool.ledger.rateNum ∧
    s'.pool.ledger.rateDen = s.pool.ledger.rateDen := by
  sorry

/-- RC3 (the close does not get stuck in the payout tail). From an invariant state whose vault
fits in `u128`, closing an account with `pnl ≥ 0` returns, provided that, if the account has no
receipt yet, its scaled positive pnl is within the ledger's unreceipted bound. -/
theorem RC3_close_step_returns (s : St) (i : Nat) (a : Acct) (h : Inv s)
    (hv : s.pool.vault ≤ U128_MAX) (hi : s.accts[i]? = some a) (hpnl : 0 ≤ a.pnl)
    (hbound : a.receipt.present = true ∨
              a.pnl.toNat * BOUND_SCALE ≤ s.pool.ledger.unreceiptedNum) :
    (closeStep s i).isSome = true := by
  sorry

/-- RC4 (any number of accounts, any order, repeats allowed). If closing the accounts `order`
one after the other returns, then: the invariant holds at the end; the vault dropped by exactly
the total paid; the total paid is the capital that left plus what was added to the receipts;
everything ever paid against receipts is at most the snapshot residual; and the vault still
holds the remaining capital and the whole insurance. -/
theorem RC4_close_all (s s' : St) (order : List Nat) (total : Nat) (h : Inv s)
    (hrun : closeAll s order = some (s', total)) :
    Inv s' ∧
    s'.pool.vault + total = s.pool.vault ∧
    total + sumCap s'.accts + sumPaid s.accts = sumCap s.accts + sumPaid s'.accts ∧
    sumPaid s'.accts ≤ s.pool.ledger.residual ∧
    s'.pool.cTot + s'.pool.insurance ≤ s'.pool.vault ∧
    s'.pool.insurance = s.pool.insurance ∧
    (∀ i ∈ order, ∀ a', s'.accts[i]? = some a' → a'.capital = 0 ∧ a'.pnl = 0) := by
  sorry

/-- RC5 (everyone closed). If `order` contains every account index, then after it `c_tot` is 0,
the total paid is all the capital plus what was added to the receipts, and that is at most the
capital plus the unpaid part of the snapshot residual; the insurance is still in the vault. -/
theorem RC5_close_everyone (s s' : St) (order : List Nat) (total : Nat) (h : Inv s)
    (hall : ∀ i, i < s.accts.length → i ∈ order)
    (hrun : closeAll s order = some (s', total)) :
    s'.pool.cTot = 0 ∧
    total + sumPaid s.accts = sumCap s.accts + sumPaid s'.accts ∧
    total + sumPaid s.accts ≤ sumCap s.accts + s.pool.ledger.residual ∧
    s'.pool.insurance ≤ s'.pool.vault := by
  sorry

/-- RC6 (the order does not matter). Two orders that are permutations of each other give the
same final state and the same total. -/
theorem RC6_order_independent (s : St) (o1 o2 : List Nat) (r : St × Nat) (h : Inv s)
    (hperm : o1.Perm o2) (hrun : closeAll s o1 = some r) :
    closeAll s o2 = some r := by
  sorry

/-- RC7 (top-up). A receipt top-up from an invariant state keeps the invariant, takes exactly
its payout from the vault and adds exactly its payout to the receipt. -/
theorem RC7_topup_step (s s' : St) (i payout : Nat) (h : Inv s)
    (hstep : topupStep s i = some (s', payout)) :
    Inv s' ∧
    s'.pool.vault + payout = s.pool.vault ∧
    sumPaid s'.accts = sumPaid s.accts + payout ∧
    sumCap s'.accts = sumCap s.accts := by
  sorry

end Spec.ResolvedClose
