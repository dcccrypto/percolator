module

public import Mathlib

/-!
# Mathematical model of the resolved-close path

An executable model, over mathematical integers, of the Rust functions of
`src/resolved_payout.rs` and `src/resolved_payout/close_path.rs` that the
properties RC1..RC7 of `aristotle/resolved_close_spec.md` are about, together
with the invariant `Inv` of that document.

A Rust `V16Result<T>` is modelled by `Option T`: `none` stands for "returns
`Err(_)`" (the error variant and the state left behind by an `Err` are not
modelled), `some x` for "returns `Ok(x)`".

`u128` values are natural numbers below `2 ^ 128`, `i128` values integers in
`[-2 ^ 127, 2 ^ 127)`, `u64` values natural numbers below `2 ^ 64`. Every
checked operation of the Rust code becomes an explicit range test.
-/

@[expose] public section

namespace Spec

/-- `BOUND_SCALE` (src/lib.rs). -/
def BOUND_SCALE : Nat := 1000000000000

/-- `2 ^ 128`: a `u128` is a natural number strictly below it. -/
def U128 : Nat := 2 ^ 128

/-- The fields of `ResolvedPayoutLedgerV16`. -/
structure Ledger where
  snapshot_residual : Nat
  terminal_claim_exact_receipts_num : Nat
  terminal_claim_bound_unreceipted_num : Nat
  current_payout_rate_num : Nat
  current_payout_rate_den : Nat
  snapshot_slot : Nat
  payout_halted : Bool
  finalized : Bool
deriving DecidableEq, Repr

/-- The fields of `ResolvedPayoutReceiptV16`. -/
structure Receipt where
  present : Bool
  prior_bound_contribution_num : Nat
  live_released_face_at_receipt : Nat
  terminal_positive_claim_face : Nat
  paid_effective : Nat
  finalized : Bool
deriving DecidableEq, Repr

/-- `ResolvedPayoutReceiptV16::EMPTY`. -/
def Receipt.empty : Receipt :=
  ⟨false, 0, 0, 0, 0, false⟩

/-- The fields of `PoolV16`. -/
structure Pool where
  vault : Nat
  c_tot : Nat
  insurance : Nat
  ledger : Ledger
deriving DecidableEq, Repr

/-- The fields of `ClaimantV16`. -/
structure Claimant where
  capital : Nat
  pnl : Int
  receipt : Receipt
deriving DecidableEq, Repr

/-! ## Ranges of the Rust types -/

/-- Every numeric field of the ledger fits its Rust type. -/
def Ledger.Fits (l : Ledger) : Prop :=
  l.snapshot_residual < U128 ∧ l.terminal_claim_exact_receipts_num < U128 ∧
  l.terminal_claim_bound_unreceipted_num < U128 ∧ l.current_payout_rate_num < U128 ∧
  l.current_payout_rate_den < U128 ∧ l.snapshot_slot < 2 ^ 64

/-- Every numeric field of the receipt fits `u128`. -/
def Receipt.Fits (r : Receipt) : Prop :=
  r.prior_bound_contribution_num < U128 ∧ r.live_released_face_at_receipt < U128 ∧
  r.terminal_positive_claim_face < U128 ∧ r.paid_effective < U128

/-- Every numeric field of the pool fits its Rust type. -/
def Pool.Fits (p : Pool) : Prop :=
  p.vault < U128 ∧ p.c_tot < U128 ∧ p.insurance < U128 ∧ p.ledger.Fits

/-- Every numeric field of the account fits its Rust type (`pnl` is an `i128`). -/
def Claimant.Fits (a : Claimant) : Prop :=
  a.capital < U128 ∧ -2 ^ 127 ≤ a.pnl ∧ a.pnl < 2 ^ 127 ∧ a.receipt.Fits

/-! ## The Rust functions -/

/-- `V16Core::kernel_recompute_resolved_payout_rate`. -/
def recomputeRate (l : Ledger) : Option Ledger :=
  let total := l.terminal_claim_exact_receipts_num + l.terminal_claim_bound_unreceipted_num
  if U128 ≤ total then none
  else if total = 0 then
    some { l with current_payout_rate_num := 1, current_payout_rate_den := 1 }
  else if U128 ≤ l.snapshot_residual * BOUND_SCALE then none
  else
    some { l with
      current_payout_rate_num := min (l.snapshot_residual * BOUND_SCALE) total
      current_payout_rate_den := total }

/-- `validate_resolved_payout_receipt_value` returns `Ok`. -/
def receiptValid (r : Receipt) : Prop :=
  if r.present then
    r.terminal_positive_claim_face * BOUND_SCALE < U128 ∧
    r.terminal_positive_claim_face * BOUND_SCALE ≤ r.prior_bound_contribution_num ∧
    r.paid_effective ≤ r.terminal_positive_claim_face ∧
    (r.finalized = decide (r.paid_effective = r.terminal_positive_claim_face))
  else r = Receipt.empty

instance (r : Receipt) : Decidable (receiptValid r) := by
  unfold receiptValid; infer_instance

/-- `apply_resolved_payout_receipt_payment(receipt, actual_resolved_paid)`. -/
def applyPayment (r : Receipt) (x : Nat) : Option Receipt :=
  if ¬ receiptValid r then none
  else if x = 0 then some r
  else if ¬ r.present then none
  else if r.terminal_positive_claim_face < r.paid_effective then none
  else if r.terminal_positive_claim_face - r.paid_effective < x then none
  else if U128 ≤ r.paid_effective + x then none
  else
    let r' := { r with
      paid_effective := r.paid_effective + x
      finalized := decide (r.paid_effective + x = r.terminal_positive_claim_face) }
    if receiptValid r' then some r' else none

/-- `floor(face * rate_num / rate_den)`: the part of a receipt of face `face` owed at the
ledger's current rate (what `wide_mul_div_floor_u128` computes when `rate_den > 0`). -/
def owed (l : Ledger) (face : Nat) : Nat :=
  face * l.current_payout_rate_num / l.current_payout_rate_den

/-- `MarketGroupV16ViewMut::resolved_receipt_claimable_against_ledger(receipt, ledger)`. -/
def claimable (r : Receipt) (l : Ledger) : Option Nat :=
  if ¬ receiptValid r then none
  else if ¬ r.present then some 0
  else if l.payout_halted then none
  else
    let gross := owed l r.terminal_positive_claim_face
    if gross < r.paid_effective then none else some (gross - r.paid_effective)

/-- `create_resolved_payout_receipt_if_needed(pool, account)`. -/
def createReceipt (p : Pool) (a : Claimant) : Option (Pool × Claimant) :=
  if a.receipt.present then some (p, a)
  else
    let face := a.pnl.toNat
    let prior := face * BOUND_SCALE
    let l := p.ledger
    if U128 ≤ prior then none
    else if l.terminal_claim_bound_unreceipted_num < prior then none
    else if U128 ≤ l.terminal_claim_exact_receipts_num + prior then none
    else
      let l1 := { l with
        terminal_claim_bound_unreceipted_num := l.terminal_claim_bound_unreceipted_num - prior
        terminal_claim_exact_receipts_num := l.terminal_claim_exact_receipts_num + prior }
      let a1 := { a with receipt := ⟨true, prior, 0, face, 0, false⟩ }
      match recomputeRate l1 with
      | none => none
      | some l2 => some ({ p with ledger := l2 }, a1)

/-- `close_resolved_payout_tail(pool, account)`: on `Ok(payout)`, the new pool, the new
account and `payout`. -/
def closeTail (p : Pool) (a : Claimant) : Option (Pool × Claimant × Nat) :=
  if a.pnl < 0 then none
  else
    let stage : Option (Pool × Claimant × Nat × Bool) :=
      if 0 < a.pnl ∨ a.receipt.present = true then
        match createReceipt p a with
        | none => none
        | some (p1, a1) =>
          match claimable a1.receipt p1.ledger with
          | none => none
          | some c => some (p1, a1, c, true)
      else some (p, a, 0, false)
    match stage with
    | none => none
    | some (p1, a1, c, hasReceipt) =>
      let cap := a1.capital
      if U128 ≤ cap + c then none
      else
        let payout := min (cap + c) p1.vault
        let resolvedPaid := payout - min cap payout
        let a2 : Option Claimant :=
          if hasReceipt then
            match applyPayment a1.receipt resolvedPaid with
            | none => none
            | some r => some { a1 with receipt := r }
          else some a1
        match a2 with
        | none => none
        | some a2 =>
          if p1.vault < payout then none
          else
            some ({ p1 with vault := p1.vault - payout, c_tot := p1.c_tot - min cap p1.c_tot },
              { a2 with pnl := 0, capital := 0 }, payout)

/-- `claim_resolved_payout_topup_core(pool, account)`. -/
def topup (p : Pool) (a : Claimant) : Option (Pool × Claimant × Nat) :=
  match claimable a.receipt p.ledger with
  | none => none
  | some c =>
    if c = 0 then some (p, a, 0)
    else
      let payout := min c p.vault
      match applyPayment a.receipt payout with
      | none => none
      | some r =>
        if p.vault < payout then none
        else some ({ p with vault := p.vault - payout }, { a with receipt := r }, payout)

/-- The loop of `close_all` from the point where `total` has been accumulated so far. -/
def closeAllFrom (p : Pool) (accs : List Claimant) (total : Nat) :
    List Nat → Option (Pool × List Claimant × Nat)
  | [] => some (p, accs, total)
  | i :: rest =>
    if h : i < accs.length then
      match closeTail p accs[i] with
      | none => none
      | some (p', a', paid) =>
        if U128 ≤ total + paid then none
        else closeAllFrom p' (accs.set i a') (total + paid) rest
    else none

/-- `close_all(pool, accounts, order)`: on `Ok(total)`, the new pool, the new accounts and
`total`. -/
def closeAll (p : Pool) (accs : List Claimant) (order : List Nat) :
    Option (Pool × List Claimant × Nat) :=
  closeAllFrom p accs 0 order

/-! ## The invariant `Inv` and the sums `G`, `P`, `C` -/

/-- The accounts whose receipt is present. -/
def withReceipt (accs : List Claimant) : List Claimant :=
  accs.filter (fun a => a.receipt.present)

/-- `G`: the sum over present receipts of `floor(face * rate_num / rate_den)`. -/
def sumOwed (l : Ledger) (accs : List Claimant) : Nat :=
  ((withReceipt accs).map (fun a => owed l a.receipt.terminal_positive_claim_face)).sum

/-- `P`: the sum over present receipts of `paid_effective`. -/
def sumPaid (accs : List Claimant) : Nat :=
  ((withReceipt accs).map (fun a => a.receipt.paid_effective)).sum

/-- The sum over present receipts of `terminal_positive_claim_face * BOUND_SCALE`. -/
def sumFaceNum (accs : List Claimant) : Nat :=
  ((withReceipt accs).map (fun a => a.receipt.terminal_positive_claim_face * BOUND_SCALE)).sum

/-- `C`: the sum of `capital` over all accounts. -/
def sumCapital (accs : List Claimant) : Nat :=
  (accs.map (fun a => a.capital)).sum

/-- The invariant `Inv(pool, accounts)` of `aristotle/resolved_close_spec.md`. -/
structure Inv (p : Pool) (accs : List Claimant) : Prop where
  /-- `kernel_recompute_resolved_payout_rate(pool.ledger)` returns `Ok(pool.ledger)`. -/
  rate : recomputeRate p.ledger = some p.ledger
  /-- `pool.ledger.payout_halted == false`. -/
  live : p.ledger.payout_halted = false
  /-- Every account's receipt passes `validate_resolved_payout_receipt_value`. -/
  receipts : ∀ a ∈ accs, receiptValid a.receipt
  /-- The present receipts' faces, scaled, are covered by the exact-receipts bound. -/
  cover : sumFaceNum accs ≤ p.ledger.terminal_claim_exact_receipts_num
  /-- No present receipt has been paid beyond what it is owed at the current rate. -/
  paidLe : ∀ a ∈ accs, a.receipt.present = true →
    a.receipt.paid_effective ≤ owed p.ledger a.receipt.terminal_positive_claim_face
  /-- `c_tot` is the sum of capital. -/
  mirror : p.c_tot = sumCapital accs
  /-- Capital, insurance and the unpaid part of the snapshot are in the vault. -/
  backed : p.c_tot + p.insurance + (p.ledger.snapshot_residual - sumPaid accs) ≤ p.vault

end Spec
