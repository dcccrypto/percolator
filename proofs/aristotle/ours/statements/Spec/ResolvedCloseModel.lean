/-!
# Resolved close, payout part: model of the Rust in `project/src/resolved_payout*.rs`

Definitions only. Lean core, no Mathlib. Typechecked with the toolchain in `../lean-toolchain`.

Each definition models one Rust function of the project slice (which is the engine's code, see
`project/src/resolved_payout.rs`, or a transcription of it, see
`project/src/resolved_payout/close_path.rs`). `Option` models `V16Result`: `none` is any `Err`
and also any panic (the engine's callers revert the transaction on either). `u128` values are
modelled as `Nat`; a checked operation whose result exceeds `U128_MAX` is `none`.

This is a hand-written reference model. Aristotle does not take hand-written Lean as the thing to
verify (it compiles the Rust and writes its own Lean); this file states what we mean, precisely.
-/

namespace Spec.ResolvedClose

/-- `BOUND_SCALE`, engine `src/lib.rs:25`. -/
def BOUND_SCALE : Nat := 1000000000000

def U128_MAX : Nat := 2 ^ 128 - 1

/-- A checked `u128` result. -/
def ck (n : Nat) : Option Nat := if n ≤ U128_MAX then some n else none

/-- `ResolvedPayoutLedgerV16` (engine `src/v16.rs:7094`), without `snapshot_slot` and
`finalized`, which the modelled functions neither read nor write. -/
structure Ledger where
  residual : Nat        -- snapshot_residual
  exactNum : Nat        -- terminal_claim_exact_receipts_num
  unreceiptedNum : Nat  -- terminal_claim_bound_unreceipted_num
  rateNum : Nat         -- current_payout_rate_num
  rateDen : Nat         -- current_payout_rate_den
  halted : Bool         -- payout_halted
deriving Repr, DecidableEq

/-- `ResolvedPayoutReceiptV16` (engine `src/v16.rs:7126`). -/
structure Receipt where
  present : Bool
  priorBoundNum : Nat   -- prior_bound_contribution_num
  liveReleased : Nat    -- live_released_face_at_receipt
  face : Nat            -- terminal_positive_claim_face
  paid : Nat            -- paid_effective
  finalized : Bool
deriving Repr, DecidableEq

def Receipt.empty : Receipt := ⟨false, 0, 0, 0, 0, false⟩

/-- `ClaimantV16`: the account fields the path touches. -/
structure Acct where
  capital : Nat
  pnl : Int
  receipt : Receipt
deriving Repr, DecidableEq

/-- `PoolV16`: the market-header fields the path touches. -/
structure Pool where
  vault : Nat
  cTot : Nat
  insurance : Nat
  ledger : Ledger
deriving Repr, DecidableEq

structure St where
  pool : Pool
  accts : List Acct
deriving Repr, DecidableEq

/-- `V16Core::kernel_recompute_resolved_payout_rate` (engine `src/v16.rs:2688-2707`). -/
def recompute (l : Ledger) : Option Ledger :=
  match ck (l.exactNum + l.unreceiptedNum) with
  | none => none
  | some tot =>
    if tot = 0 then some { l with rateNum := 1, rateDen := 1 }
    else
      match ck (l.residual * BOUND_SCALE) with
      | none => none
      | some x => some { l with rateNum := min x tot, rateDen := tot }

/-- `validate_resolved_payout_receipt_value` returns `Ok` (engine `src/v16.rs:7161-7179`). -/
def receiptOk (r : Receipt) : Bool :=
  if r.present then
    decide (r.face * BOUND_SCALE ≤ U128_MAX) && decide (r.face * BOUND_SCALE ≤ r.priorBoundNum)
      && decide (r.paid ≤ r.face) && (r.finalized == decide (r.paid = r.face))
  else
    decide (r = Receipt.empty)

/-- The gross amount a receipt is owed at the ledger's rate: `floor(face * num / den)`
(`wide_mul_div_floor_u128`, engine `src/wide_math.rs:1623`). -/
def gross (r : Receipt) (l : Ledger) : Nat := r.face * l.rateNum / l.rateDen

/-- `resolved_receipt_claimable_against_ledger` (engine `src/v16.rs:25252-25271`).
`wide_mul_div_floor_u128` panics on a zero divisor or a result above `u128::MAX`. -/
def claimableNow (r : Receipt) (l : Ledger) : Option Nat :=
  if !receiptOk r then none
  else if !r.present then some 0
  else if l.halted then none
  else if l.rateDen = 0 ∨ U128_MAX < gross r l then none
  else if r.paid ≤ gross r l then some (gross r l - r.paid) else none

/-- `apply_resolved_payout_receipt_payment` (engine `src/v16.rs:7181-7206`). -/
def applyPayment (r : Receipt) (x : Nat) : Option Receipt :=
  if !receiptOk r then none
  else if x = 0 then some r
  else if !r.present then none
  else if r.face - r.paid < x then none
  else
    match ck (r.paid + x) with
    | none => none
    | some p =>
      let r' : Receipt := { r with paid := p, finalized := decide (p = r.face) }
      if receiptOk r' then some r' else none

/-- `create_resolved_payout_receipt_if_needed` (slice `close_path.rs`; engine
`src/v16.rs:25203-25245` after the snapshot is captured). -/
def createReceipt (p : Pool) (a : Acct) : Option (Pool × Acct) :=
  if a.receipt.present then some (p, a)
  else
    let face := a.pnl.toNat
    match ck (face * BOUND_SCALE) with
    | none => none
    | some x =>
      if p.ledger.unreceiptedNum < x then none
      else
        match ck (p.ledger.exactNum + x) with
        | none => none
        | some e =>
          match recompute { p.ledger with unreceiptedNum := p.ledger.unreceiptedNum - x,
                                          exactNum := e } with
          | none => none
          | some l => some ({ p with ledger := l },
                            { a with receipt := ⟨true, x, 0, face, 0, false⟩ })

/-- `close_resolved_payout_tail` (slice `close_path.rs`; engine `src/v16.rs:26643-26691`).
Returns the new pool, the new account and the payout. -/
def closeTail (p : Pool) (a : Acct) : Option (Pool × Acct × Nat) :=
  if a.pnl < 0 then none
  else
    let pre : Option (Pool × Acct × Nat × Bool) :=
      if 0 < a.pnl ∨ a.receipt.present = true then
        match createReceipt p a with
        | none => none
        | some (p1, a1) =>
          match claimableNow a1.receipt p1.ledger with
          | none => none
          | some c => some (p1, a1, c, true)
      else some (p, a, 0, false)
    match pre with
    | none => none
    | some (p1, a1, pnlPayout, hasReceipt) =>
      match ck (a1.capital + pnlPayout) with
      | none => none
      | some sum =>
        let payout := min sum p1.vault
        let capitalPaid := min a1.capital payout
        let resolvedPaid := payout - capitalPaid
        let r : Option Receipt :=
          if hasReceipt then applyPayment a1.receipt resolvedPaid else some a1.receipt
        match r with
        | none => none
        | some r' =>
          some ({ p1 with vault := p1.vault - payout,
                          cTot := p1.cTot - min a1.capital p1.cTot },
                { capital := 0, pnl := 0, receipt := r' }, payout)

/-- `claim_resolved_payout_topup_core` (slice `close_path.rs`; engine `src/v16.rs:26492-26525`). -/
def topupCore (p : Pool) (a : Acct) : Option (Pool × Acct × Nat) :=
  match claimableNow a.receipt p.ledger with
  | none => none
  | some c =>
    if c = 0 then some (p, a, 0)
    else
      let payout := min c p.vault
      match applyPayment a.receipt payout with
      | none => none
      | some r' => some ({ p with vault := p.vault - payout }, { a with receipt := r' }, payout)

/-- One engine call on account `i`: close it. `none` if `i` is out of range. -/
def closeStep (s : St) (i : Nat) : Option (St × Nat) :=
  match s.accts[i]? with
  | none => none
  | some a =>
    match closeTail s.pool a with
    | none => none
    | some (p', a', payout) => some ({ pool := p', accts := s.accts.set i a' }, payout)

/-- One engine call on account `i`: claim a receipt top-up. -/
def topupStep (s : St) (i : Nat) : Option (St × Nat) :=
  match s.accts[i]? with
  | none => none
  | some a =>
    match topupCore s.pool a with
    | none => none
    | some (p', a', payout) => some ({ pool := p', accts := s.accts.set i a' }, payout)

/-- `close_all` (slice `close_path.rs`): close the accounts `order` in sequence; total paid.
(The Rust also fails if the total exceeds `u128::MAX`.) -/
def closeAll (s : St) : List Nat → Option (St × Nat)
  | [] => some (s, 0)
  | i :: is =>
    match closeStep s i with
    | none => none
    | some (s1, p) =>
      match closeAll s1 is with
      | none => none
      | some (s2, q) =>
        match ck (p + q) with
        | none => none
        | some t => some (s2, t)

/-! ## Sums over the account list -/

def sumCap : List Acct → Nat
  | [] => 0
  | a :: as => a.capital + sumCap as

/-- Total already paid against receipts. -/
def sumPaid : List Acct → Nat
  | [] => 0
  | a :: as => (if a.receipt.present then a.receipt.paid else 0) + sumPaid as

/-- Total scaled face of the receipts. -/
def sumFaceNum : List Acct → Nat
  | [] => 0
  | a :: as => (if a.receipt.present then a.receipt.face * BOUND_SCALE else 0) + sumFaceNum as

/-- Total gross amount owed to the receipts at the ledger's rate. -/
def sumGross (l : Ledger) : List Acct → Nat
  | [] => 0
  | a :: as => (if a.receipt.present then gross a.receipt l else 0) + sumGross l as

/-! ## The invariant -/

/-- The state after the payout snapshot, as far as the payout path depends on it.

* `rate`: the stored rate is the one `kernel_recompute_resolved_payout_rate` computes
  (in particular `exactNum + unreceiptedNum` and `residual * BOUND_SCALE` fit in `u128`);
* `live`: payouts are not halted;
* `receipts`: every account's receipt passes the engine's receipt validation;
* `cover`: the scaled faces of the receipts are counted in the ledger's exact-receipt total;
* `paidLe`: no receipt has been paid more than its gross amount at the rate;
* `mirror`: `c_tot` is the sum of account capital;
* `backed`: the vault holds the capital, the insurance and the part of the snapshot residual
  that has not been paid out yet. -/
structure Inv (s : St) : Prop where
  rate : recompute s.pool.ledger = some s.pool.ledger
  live : s.pool.ledger.halted = false
  receipts : ∀ a ∈ s.accts, receiptOk a.receipt = true
  cover : sumFaceNum s.accts ≤ s.pool.ledger.exactNum
  paidLe : ∀ a ∈ s.accts, a.receipt.present = true → a.receipt.paid ≤ gross a.receipt s.pool.ledger
  mirror : s.pool.cTot = sumCap s.accts
  backed : s.pool.cTot + s.pool.insurance + (s.pool.ledger.residual - sumPaid s.accts)
             ≤ s.pool.vault

end Spec.ResolvedClose
