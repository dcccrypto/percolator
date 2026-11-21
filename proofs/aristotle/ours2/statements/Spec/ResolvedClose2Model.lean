/-!
# Resolved close on the engine text: a reference model of `project/src/engine_slice.rs`

Definitions only. Lean core, no Mathlib. Typechecked with the toolchain in `../lean-toolchain`.

This is a HAND-WRITTEN reference model, written to state precisely what RCs1..RC6s of
`project/aristotle/resolved_close_spec.md` mean. Aristotle does not take hand-written Lean as the
thing to verify (it compiles the Rust and writes its own Lean). That this model matches the Rust
is not proved anywhere.

Conventions. `Option` models `V16Result`: `none` is any `Err` and also any panic. `u128` values
are `Nat`; a checked operation whose result exceeds `U128_MAX` is `none`. Values are the decoded
ones: the byte wrappers and `decode_bool` are not modelled (clause `wf` of the spec's `Inv` says
every decode succeeds). Not modelled because they write nothing the properties mention:
`payout_snapshot`, `payout_snapshot_pnl_pos_tot`, `snapshot_slot`, the ledger's `finalized`.
`TokenValueFlowProofV16 ... .validate()` is not modelled: on this path its inputs satisfy
`capital_paid + resolved_paid = payout = vault_before - vault_after`, which is what it checks.
-/

namespace Spec.ResolvedClose2

/-- `BOUND_SCALE`, engine `src/lib.rs:25`. -/
def BOUND_SCALE : Nat := 1000000000000

def U128_MAX : Nat := 2 ^ 128 - 1

/-- A checked `u128` result. -/
def ck (n : Nat) : Option Nat := if n ≤ U128_MAX then some n else none

/-- `ResolvedPayoutLedgerV16` (engine `src/v16.rs:7094`). -/
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

/-- The fields of the slice's `PortfolioAccountV16Account`. -/
structure Acct where
  capital : Nat
  pnl : Int
  reservedPnl : Nat
  feeCredits : Int
  certValid : Nat       -- health_cert.valid
  receipt : Receipt
deriving Repr, DecidableEq

/-- The fields of the slice's `MarketGroupV16HeaderAccount` that the path reads or writes. -/
structure Market where
  vault : Nat
  insurance : Nat
  cTot : Nat
  pnlPosTot : Nat
  pnlPosBoundTotNum : Nat
  pnlPosBoundTot : Nat
  pnlMaturedPosTot : Nat
  earnings : Nat         -- backing_provider_earnings_total
  freshBackingNum : Nat  -- source_fresh_backing_total_num
  blockerCount : Nat     -- resolved_payout_blocker_count
  staleCertCount : Nat   -- stale_certificate_count
  bStaleCount : Nat      -- b_stale_account_count
  negPnlCount : Nat      -- negative_pnl_account_count
  captured : Bool        -- payout_snapshot_captured
  ledger : Ledger        -- resolved_payout_ledger
deriving Repr, DecidableEq

structure St where
  market : Market
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

/-- `floor(face * num / den)`: what `wide_mul_div_floor_u128` returns (property F3). -/
def grossOf (face : Nat) (l : Ledger) : Nat := face * l.rateNum / l.rateDen

/-- `resolved_receipt_claimable_against_ledger` (engine `src/v16.rs:25252-25271`).
`wide_mul_div_floor_u128` panics on a zero divisor or a result above `u128::MAX` (F3). -/
def claimableNow (r : Receipt) (l : Ledger) : Option Nat :=
  if !receiptOk r then none
  else if !r.present then some 0
  else if l.halted then none
  else if l.rateDen = 0 ∨ U128_MAX < grossOf r.face l then none
  else if r.paid ≤ grossOf r.face l then some (grossOf r.face l - r.paid) else none

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

/-- `residual()` (engine `src/v16.rs:10602-10611`); the additions saturate at `u128::MAX`. -/
def residualOf (m : Market) : Nat :=
  m.vault - min U128_MAX (m.cTot + m.insurance + m.earnings + m.freshBackingNum / BOUND_SCALE)

/-- `initialize_resolved_payout_ledger_if_needed` (engine `src/v16.rs:25177-25201`). -/
def initLedger (m : Market) : Option Market :=
  if m.captured then some m
  else
    match recompute ⟨residualOf m, 0, m.pnlPosBoundTotNum, 0, 0, false⟩ with
    | none => none
    | some l => some { m with captured := true, ledger := l }

/-- `create_resolved_payout_receipt_if_needed` (engine `src/v16.rs:25203-25245`). The engine's
`RecoveryRequired` exit (which also sets `payout_halted`) is `none` here. -/
def createReceipt (m : Market) (a : Acct) : Option (Market × Acct) :=
  if a.receipt.present then some (m, a)
  else
    match initLedger m with
    | none => none
    | some m0 =>
      let face := a.pnl.toNat
      match ck (face * BOUND_SCALE) with
      | none => none
      | some x =>
        if m0.ledger.unreceiptedNum < x then none
        else
          match ck (m0.ledger.exactNum + x) with
          | none => none
          | some e =>
            match recompute { m0.ledger with unreceiptedNum := m0.ledger.unreceiptedNum - x,
                                             exactNum := e } with
            | none => none
            | some l => some ({ m0 with ledger := l },
                              { a with receipt := ⟨true, x, 0, face, 0, false⟩ })

/-- `resolved_payout_rate_is_terminal` (engine `src/v16.rs:26129-26134`). -/
def rateTerminal (m : Market) : Bool :=
  decide (m.ledger.unreceiptedNum = 0) &&
    (decide (m.ledger.rateNum = m.ledger.rateDen) || decide (m.freshBackingNum = 0))

/-- `resolved_positive_payout_ready` (engine `src/v16.rs:25158-25167`). -/
def payoutReady (m : Market) : Bool :=
  decide (m.bStaleCount = 0) && decide (m.staleCertCount = 0) && decide (m.negPnlCount = 0)
    && decide (m.blockerCount = 0)

/-- `clear_fully_diluted_resolved_receipt_if_terminal` (engine `src/v16.rs:26140-26160`). -/
def clearIfTerminal (m : Market) (a : Acct) : Option Acct :=
  if !a.receipt.present || a.receipt.finalized then some a
  else if !rateTerminal m then some a
  else if !payoutReady m then some a
  else
    match claimableNow a.receipt m.ledger with
    | none => none
    | some c => if c ≠ 0 then some a else some { a with receipt := Receipt.empty }

/-- `set_account_pnl(account, 0)` (engine `src/v16.rs:15218-15224`, `15341-15468`) as far as it
writes the market header, with the two stubs of the slice (`burn_account_source_claim_bound_num`,
`sync_unattributed_loss_lock`) as no-ops. -/
def setPnlZero (m : Market) (pnl : Int) : Option Market :=
  let oldPos := pnl.toNat
  let m1 : Option Market :=
    if oldPos = 0 then
      match ck m.pnlPosTot, ck m.pnlPosBoundTotNum with
      | some _, some _ => some m
      | _, _ => none
    else
      match ck (oldPos * BOUND_SCALE) with
      | none => none
      | some decNum =>
        if m.pnlPosTot < oldPos then none
        else
          let tot := m.pnlPosTot - oldPos
          match ck (tot * BOUND_SCALE) with
          | none => none
          | some exactMin =>
            some { m with pnlPosTot := tot,
                          pnlPosBoundTotNum := max (m.pnlPosBoundTotNum - decNum) exactMin,
                          pnlMaturedPosTot := min m.pnlMaturedPosTot tot }
  match m1 with
  | none => none
  | some m1 =>
    let whole := m1.pnlPosBoundTotNum / BOUND_SCALE
    let bound := if m1.pnlPosBoundTotNum % BOUND_SCALE = 0 then whole else whole + 1
    let m2 := { m1 with pnlPosBoundTot := bound }
    if pnl < 0 then
      (if m2.negPnlCount = 0 then none else some { m2 with negPnlCount := m2.negPnlCount - 1 })
    else some m2

/-- The close tail, engine `src/v16.rs:26643-26702` (`close_resolved_account_tail_not_atomic`,
`close_one`). Returns the new market, the new account and the payout. -/
def closeTail (m : Market) (a : Acct) : Option (Market × Acct × Nat) :=
  let pre : Option (Market × Acct × Nat × Bool) :=
    if 0 < a.pnl ∨ a.receipt.present = true then
      match createReceipt m a with
      | none => none
      | some (m1, a1) =>
        match claimableNow a1.receipt m1.ledger with
        | none => none
        | some c => some (m1, a1, c, true)
    else some (m, a, 0, false)
  match pre with
  | none => none
  | some (m1, a1, pnlPayout, hasReceipt) =>
    match ck (a1.capital + pnlPayout) with
    | none => none
    | some sum =>
      let payout := min sum m1.vault
      let capitalPaid := min a1.capital payout
      let resolvedPaid := payout - capitalPaid
      let r : Option Receipt :=
        if hasReceipt then applyPayment a1.receipt resolvedPaid else some a1.receipt
      match r with
      | none => none
      | some r' =>
        match clearIfTerminal m1 { a1 with receipt := r' } with
        | none => none
        | some a2 =>
          match setPnlZero { m1 with vault := m1.vault - payout,
                                     cTot := m1.cTot - min a1.capital m1.cTot } a2.pnl with
          | none => none
          | some m2 =>
            some (m2, { capital := 0, pnl := 0, reservedPnl := 0, feeCredits := 0, certValid := 0,
                        receipt := a2.receipt }, payout)

/-- `claim_resolved_payout_topup_core_not_atomic` (engine `src/v16.rs:26492-26525`, `topup_one`). -/
def topupCore (m : Market) (a : Acct) : Option (Market × Acct × Nat) :=
  match claimableNow a.receipt m.ledger with
  | none => none
  | some c =>
    if c = 0 then
      match clearIfTerminal m a with
      | none => none
      | some a' => some (m, a', 0)
    else
      let payout := min c m.vault
      match applyPayment a.receipt payout with
      | none => none
      | some r' => some ({ m with vault := m.vault - payout }, { a with receipt := r' }, payout)

/-- One call on account `i`: close it. `none` if `i` is out of range. -/
def closeStep (s : St) (i : Nat) : Option (St × Nat) :=
  match s.accts[i]? with
  | none => none
  | some a =>
    match closeTail s.market a with
    | none => none
    | some (m', a', payout) => some ({ market := m', accts := s.accts.set i a' }, payout)

/-- One call on account `i`: claim a receipt top-up. -/
def topupStep (s : St) (i : Nat) : Option (St × Nat) :=
  match s.accts[i]? with
  | none => none
  | some a =>
    match topupCore s.market a with
    | none => none
    | some (m', a', payout) => some ({ market := m', accts := s.accts.set i a' }, payout)

/-- `close_all` (`harness.rs`, not engine): close the accounts `order` in sequence; total paid.
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

/-! ## The receipt list `R`: present receipts of the accounts, then the ghost list -/

/-- `(face, paid)` of the present receipts. -/
def presentReceipts : List Acct → List (Nat × Nat)
  | [] => []
  | a :: as =>
    if a.receipt.present then (a.receipt.face, a.receipt.paid) :: presentReceipts as
    else presentReceipts as

/-- `R` of the spec. `cl` is the ghost list of cleared receipts: a history variable. -/
def R (s : St) (cl : List (Nat × Nat)) : List (Nat × Nat) := presentReceipts s.accts ++ cl

def sumCap : List Acct → Nat
  | [] => 0
  | a :: as => a.capital + sumCap as

def sumPaid : List (Nat × Nat) → Nat
  | [] => 0
  | r :: rs => r.2 + sumPaid rs

def sumFaceNum : List (Nat × Nat) → Nat
  | [] => 0
  | r :: rs => r.1 * BOUND_SCALE + sumFaceNum rs

def sumGross (l : Ledger) : List (Nat × Nat) → Nat
  | [] => 0
  | r :: rs => grossOf r.1 l + sumGross l rs

/-! ## The invariant -/

/-- `Inv(M, A, CL)` of the spec, on decoded values (clause `wf` is the decoding itself). -/
structure Inv (s : St) (cl : List (Nat × Nat)) : Prop where
  receipts : ∀ a ∈ s.accts, receiptOk a.receipt = true
  mirror : s.market.cTot = sumCap s.accts
  fresh : s.market.captured = false →
    R s cl = [] ∧ s.market.cTot + s.market.insurance ≤ s.market.vault
  rate : s.market.captured = true → recompute s.market.ledger = some s.market.ledger
  live : s.market.captured = true → s.market.ledger.halted = false
  cover : s.market.captured = true → sumFaceNum (R s cl) ≤ s.market.ledger.exactNum
  paidLe : s.market.captured = true → ∀ r ∈ R s cl, r.2 ≤ grossOf r.1 s.market.ledger
  backed : s.market.captured = true →
    s.market.cTot + s.market.insurance + (s.market.ledger.residual - sumPaid (R s cl))
      ≤ s.market.vault

/-- Every `u128` field of the state fits in `u128` (true of any Rust value). -/
def Fits (s : St) : Prop :=
  s.market.vault ≤ U128_MAX ∧ s.market.pnlPosTot ≤ U128_MAX ∧
  s.market.pnlPosBoundTotNum ≤ U128_MAX ∧ s.market.pnlMaturedPosTot ≤ U128_MAX ∧
  s.market.earnings ≤ U128_MAX ∧ s.market.freshBackingNum ≤ U128_MAX ∧
  s.market.insurance ≤ U128_MAX ∧ ∀ a ∈ s.accts, a.capital ≤ U128_MAX

end Spec.ResolvedClose2
