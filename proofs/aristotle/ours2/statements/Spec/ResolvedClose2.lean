import Spec.ResolvedClose2Model

/-!
# Resolved close on the engine text never overpays (RCs1..RC6s)

STATEMENTS ONLY: every theorem ends in `sorry`. Nothing here is proved. The file typechecks
(with the `sorry` warnings) against Lean core, toolchain in `../lean-toolchain`.

Labels are those of `project/aristotle/resolved_close_spec.md` and of
`prompts/stage_e2e_rcs_resolved_close_engine_text.txt`. F3 (`wide_mul_div_floor_u128`) is a
property of `src/wide_math.rs` and has no statement here: the model uses its contract
(`grossOf`, `claimableNow`).
-/

namespace Spec.ResolvedClose2

/-- A default account, for `getD`. -/
def Acct.zero : Acct := ⟨0, 0, 0, 0, 0, Receipt.empty⟩

/-- RCs1. With the snapshot captured, the receipts (present and cleared) are together owed at
most the snapshot residual at the ledger's rate, and what was paid is at most what is owed. -/
theorem RCs1_receipts_owed_at_most_residual (s : St) (cl : List (Nat × Nat)) (h : Inv s cl)
    (hc : s.market.captured = true) :
    sumGross s.market.ledger (R s cl) ≤ s.market.ledger.residual ∧
    sumPaid (R s cl) ≤ sumGross s.market.ledger (R s cl) := by
  sorry

/-- RCs2 (one close). If the close of account `i` (with `pnl ≥ 0`) returns, then for some `cl'`
that is `cl` or `cl` with one pair appended: the invariant holds; the vault dropped by exactly the
payout; the payout is the account's whole capital plus exactly what was added to the paid total
(so the `min(.., vault)` cap did not cut it); the account is zeroed; a receipt that is still
present is paid up to its gross amount; other accounts and the insurance are unchanged; and if the
snapshot was already captured, the residual and the rate are unchanged. -/
theorem RCs2_close_step (s s' : St) (cl : List (Nat × Nat)) (i payout : Nat) (h : Inv s cl)
    (hf : Fits s) (hpnl : 0 ≤ (s.accts[i]?.getD Acct.zero).pnl)
    (hstep : closeStep s i = some (s', payout)) :
    ∃ cl', (cl' = cl ∨ ∃ r, cl' = cl ++ [r]) ∧
    Inv s' cl' ∧
    s'.market.vault + payout = s.market.vault ∧
    payout + sumPaid (R s cl) = (s.accts[i]?.getD Acct.zero).capital + sumPaid (R s' cl') ∧
    s'.market.cTot + (s.accts[i]?.getD Acct.zero).capital = s.market.cTot ∧
    s'.accts.length = s.accts.length ∧
    (∀ a', s'.accts[i]? = some a' →
        a'.capital = 0 ∧ a'.pnl = 0 ∧ a'.reservedPnl = 0 ∧ a'.feeCredits = 0 ∧ a'.certValid = 0 ∧
        (a'.receipt.present = true → a'.receipt.paid = grossOf a'.receipt.face s'.market.ledger)) ∧
    (∀ j, j ≠ i → s'.accts[j]? = s.accts[j]?) ∧
    s'.market.insurance = s.market.insurance ∧
    (s.market.captured = true →
      s'.market.ledger.residual = s.market.ledger.residual ∧
      s'.market.ledger.rateNum = s.market.ledger.rateNum ∧
      s'.market.ledger.rateDen = s.market.ledger.rateDen) := by
  sorry

/-- RCs4 (any number of accounts, any order, repeats allowed). -/
theorem RCs4_close_all (s s' : St) (cl : List (Nat × Nat)) (order : List Nat) (total : Nat)
    (h : Inv s cl) (hf : Fits s) (hpnl : ∀ a ∈ s.accts, 0 ≤ a.pnl)
    (hrun : closeAll s order = some (s', total)) :
    ∃ ext, Inv s' (cl ++ ext) ∧
    s'.market.vault + total = s.market.vault ∧
    total + sumCap s'.accts + sumPaid (R s cl) = sumCap s.accts + sumPaid (R s' (cl ++ ext)) ∧
    s'.market.cTot + s'.market.insurance ≤ s'.market.vault ∧
    s'.market.insurance = s.market.insurance ∧
    (s'.market.captured = true → sumPaid (R s' (cl ++ ext)) ≤ s'.market.ledger.residual) ∧
    (∀ i ∈ order, ∀ a', s'.accts[i]? = some a' → a'.capital = 0 ∧ a'.pnl = 0) := by
  sorry

/-- RCs5 (everyone closed). -/
theorem RCs5_close_everyone (s s' : St) (cl : List (Nat × Nat)) (order : List Nat) (total : Nat)
    (h : Inv s cl) (hf : Fits s) (hpnl : ∀ a ∈ s.accts, 0 ≤ a.pnl)
    (hall : ∀ i, i < s.accts.length → i ∈ order)
    (hrun : closeAll s order = some (s', total)) :
    ∃ ext, Inv s' (cl ++ ext) ∧
    s'.market.cTot = 0 ∧
    total + sumPaid (R s cl) = sumCap s.accts + sumPaid (R s' (cl ++ ext)) ∧
    s.market.insurance ≤ s'.market.vault ∧
    (s'.market.captured = true →
      total + sumPaid (R s cl) ≤ sumCap s.accts + s'.market.ledger.residual) := by
  sorry

/-- RCs7 (top-up). -/
theorem RCs7_topup_step (s s' : St) (cl : List (Nat × Nat)) (i payout : Nat) (h : Inv s cl)
    (hf : Fits s) (hstep : topupStep s i = some (s', payout)) :
    ∃ cl', (cl' = cl ∨ ∃ r, cl' = cl ++ [r]) ∧
    Inv s' cl' ∧
    s'.market.vault + payout = s.market.vault ∧
    sumPaid (R s' cl') = sumPaid (R s cl) + payout ∧
    sumCap s'.accts = sumCap s.accts ∧
    s'.market.cTot = s.market.cTot ∧ s'.market.insurance = s.market.insurance ∧
    (∀ j, j ≠ i → s'.accts[j]? = s.accts[j]?) ∧
    (∀ a a', s.accts[i]? = some a → s'.accts[i]? = some a' →
        a'.capital = a.capital ∧ a'.pnl = a.pnl) := by
  sorry

/-- RC3s (the tail does not fail, in the slice). Under `Inv`, `pnl ≥ 0`, the receipt condition
and the aggregate condition of the spec, the close of account `i` returns. -/
theorem RC3s_close_step_returns (s : St) (cl : List (Nat × Nat)) (i : Nat) (a : Acct)
    (h : Inv s cl) (hf : Fits s) (hi : s.accts[i]? = some a) (hpnl : 0 ≤ a.pnl)
    (hreceipt : a.receipt.present = true ∨ a.pnl = 0 ∨
      (s.market.captured = true ∧
        a.pnl.toNat * BOUND_SCALE ≤ s.market.ledger.unreceiptedNum) ∨
      (s.market.captured = false ∧
        a.pnl.toNat * BOUND_SCALE ≤ s.market.pnlPosBoundTotNum ∧
        residualOf s.market * BOUND_SCALE ≤ U128_MAX))
    (hagg : a.pnl.toNat ≤ s.market.pnlPosTot ∧ s.market.pnlPosTot * BOUND_SCALE ≤ U128_MAX) :
    (closeStep s i).isSome = true := by
  sorry

/-- Two receipts are equal, or one is empty and the other is present and paid up to its gross
amount (the engine cleared one and not the other). -/
def receiptsAgree (l : Ledger) (r1 r2 : Receipt) : Prop :=
  r1 = r2 ∨
  (r1 = Receipt.empty ∧ r2.present = true ∧ r2.paid = grossOf r2.face l) ∨
  (r2 = Receipt.empty ∧ r1.present = true ∧ r1.paid = grossOf r1.face l)

/-- RC6s (the order matters only for which paid receipts were cleared). -/
theorem RC6s_order_independent (s s1 : St) (cl : List (Nat × Nat)) (o1 o2 : List Nat)
    (total : Nat) (h : Inv s cl) (hf : Fits s) (hpnl : ∀ a ∈ s.accts, 0 ≤ a.pnl)
    (hperm : o1.Perm o2) (hrun : closeAll s o1 = some (s1, total)) :
    ∃ s2, closeAll s o2 = some (s2, total) ∧
    s2.market = s1.market ∧ s2.accts.length = s1.accts.length ∧
    (∀ (j : Nat) (a1 a2 : Acct), s1.accts[j]? = some a1 → s2.accts[j]? = some a2 →
        a1.capital = a2.capital ∧ a1.pnl = a2.pnl ∧ a1.reservedPnl = a2.reservedPnl ∧
        a1.feeCredits = a2.feeCredits ∧ a1.certValid = a2.certValid ∧
        receiptsAgree s1.market.ledger a1.receipt a2.receipt) := by
  sorry

end Spec.ResolvedClose2
