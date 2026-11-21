module

public import Spec.Rust
public import SepLogic.Public
public import Spec.Model

/-!
# RC1: under `Inv`, the receipts are together owed at most `snapshot_residual`

A public statement about the generated functions
`V16Core::kernel_recompute_resolved_payout_rate` and
`MarketGroupV16ViewMut::resolved_receipt_claimable_against_ledger`, for an account slice of any
length, assuming the contract of `wide_mul_div_floor_u128`.
-/

@[expose] public section

namespace Spec
open Iris Iris.BI Rust2Lean.Public

/-- A `ResolvedPayoutLedgerV16` value, its fields in declaration order. -/
def ledgerV {PROP : Type} [BI PROP] (S : Vocab PROP) (l : Ledger) : S.Value :=
  S.tupleVal [S.intVal l.snapshot_residual, S.intVal l.terminal_claim_exact_receipts_num,
    S.intVal l.terminal_claim_bound_unreceipted_num, S.intVal l.current_payout_rate_num,
    S.intVal l.current_payout_rate_den, S.intVal l.snapshot_slot, S.boolVal l.payout_halted,
    S.boolVal l.finalized]

/-- A `ResolvedPayoutReceiptV16` value, its fields in declaration order. -/
def receiptV {PROP : Type} [BI PROP] (S : Vocab PROP) (r : Receipt) : S.Value :=
  S.tupleVal [S.boolVal r.present, S.intVal r.prior_bound_contribution_num,
    S.intVal r.live_released_face_at_receipt, S.intVal r.terminal_positive_claim_face,
    S.intVal r.paid_effective, S.boolVal r.finalized]

/-- `Ok(v)` of a `V16Result<T>` (`Ok` has discriminant 0). -/
def okResultV {PROP : Type} [BI PROP] (S : Vocab PROP) (v : S.Value) : S.Value :=
  S.variantVal 0 (S.tupleVal [v])

/-- What `resolved_receipt_claimable_against_ledger(receipt, ledger)` should return for an
account whose receipt passes validation, when the payout is not halted: `0` for an absent
receipt, else `floor(face * rate_num / rate_den) - paid_effective`. -/
def claimableAmount (l : Ledger) (a : Claimant) : Nat :=
  if a.receipt.present then
    owed l a.receipt.terminal_positive_claim_face - a.receipt.paid_effective
  else 0

/-- The contract of `wide_mul_div_floor_u128(a, b, d)` assumed here (proved elsewhere for the
upstream engine, not in this project): when `d > 0` and the quotient fits in `u128`, the call
does not panic and returns `floor(a * b / d)`. -/
def wideMulDivFloorContract {PROP : Type} [BI PROP] (S : Vocab PROP) : Prop :=
  -- For all u128 values a, b, d,
  ∀ a b d : Nat,
    -- PRECONDITIONS
    -- if they fit u128,
    a < U128 → b < U128 → d < U128 →
    -- d is positive
    0 < d →
    -- and floor(a * b / d) fits u128,
    a * b / d < U128 →
    -- then wide_mul_div_floor_u128(a, b, d) does not unwind and,
    -- on normal return,
    ⊢ S.call Mir.Fn.«percolator_resolved_close_slice::wide_math::wide_mul_div_floor_u128»
        [(S.intVal a, S.argTy Mir.Fn.«percolator_resolved_close_slice::wide_math::wide_mul_div_floor_u128» 0),
         (S.intVal b, S.argTy Mir.Fn.«percolator_resolved_close_slice::wide_math::wide_mul_div_floor_u128» 1),
         (S.intVal d, S.argTy Mir.Fn.«percolator_resolved_close_slice::wide_math::wide_mul_div_floor_u128» 2)]
        (S.returns fun v => iprop(
          -- POSTCONDITIONS
          -- returns floor(a * b / d).
          ⌊v = S.intVal ((a * b / d : Nat) : Int)⌋))

/--
RC1. Assuming the `wide_mul_div_floor_u128` contract: for every pool and every slice of
accounts (any length) satisfying `Inv`, the stored ledger is what
`kernel_recompute_resolved_payout_rate` returns for it; each account's receipt is owed exactly
`claimableAmount` by `resolved_receipt_claimable_against_ledger`; and what the receipts are
still owed plus what they have been paid is at most `snapshot_residual` (all sums are over the
slice, in mathematical integers). In particular `G <= snapshot_residual` and `P <= G`.
-/
def rc1_spec : Property := fun S =>
  -- ASSUMPTION: the contract of wide_mul_div_floor_u128.
  wideMulDivFloorContract S →
  -- For every pool and every slice of accounts,
  ∀ (pool : Pool) (accounts : List Claimant),
    -- PRECONDITIONS
    -- whose fields are values of their Rust types
    pool.Fits → (∀ a ∈ accounts, a.Fits) →
    -- and which satisfy Inv,
    Inv pool accounts →
    -- POSTCONDITIONS
    -- (1) kernel_recompute_resolved_payout_rate(pool.ledger) does not unwind
    -- and returns Ok(pool.ledger);
    (⊢ S.call
        Mir.Fn.«percolator_resolved_close_slice::resolved_payout::V16Core::kernel_recompute_resolved_payout_rate»
        [(ledgerV S pool.ledger,
          S.argTy Mir.Fn.«percolator_resolved_close_slice::resolved_payout::V16Core::kernel_recompute_resolved_payout_rate» 0)]
        (S.returns fun v => iprop(⌊v = okResultV S (ledgerV S pool.ledger)⌋))) ∧
    -- (2) for every account in the slice,
    -- resolved_receipt_claimable_against_ledger(receipt, pool.ledger)
    -- does not unwind and returns Ok(claimableAmount);
    (∀ a ∈ accounts, ⊢ S.call
        Mir.Fn.«percolator_resolved_close_slice::resolved_payout::MarketGroupV16ViewMut::resolved_receipt_claimable_against_ledger»
        [(receiptV S a.receipt,
          S.argTy Mir.Fn.«percolator_resolved_close_slice::resolved_payout::MarketGroupV16ViewMut::resolved_receipt_claimable_against_ledger» 0),
         (ledgerV S pool.ledger,
          S.argTy Mir.Fn.«percolator_resolved_close_slice::resolved_payout::MarketGroupV16ViewMut::resolved_receipt_claimable_against_ledger» 1)]
        (S.returns fun v => iprop(⌊v = okResultV S (S.intVal (claimableAmount pool.ledger a))⌋))) ∧
    -- (3) the amounts still claimable plus P are at most snapshot_residual;
    (accounts.map (claimableAmount pool.ledger)).sum + sumPaid accounts ≤
      pool.ledger.snapshot_residual ∧
    -- (4) G <= snapshot_residual
    sumOwed pool.ledger accounts ≤ pool.ledger.snapshot_residual ∧
    -- (5) and P <= G.
    sumPaid accounts ≤ sumOwed pool.ledger accounts

end Spec
