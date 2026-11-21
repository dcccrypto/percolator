module

public import Iris.BI.BIBase

@[expose] public section

/-!
# Affine pure propositions

The pinned Iris revision provides the affine modality but predates the notation
`⌊φ⌋`. This notation means `emp ∧ pure φ` and does not require an affinity
assumption. The meaning of `emp` is supplied by the chosen BI. The concrete
`IProp` of this development uses affine `UPred`, where this conjunction does
not establish that the machine has no live allocations.
-/

syntax "⌊" term "⌋" : term

macro_rules
  | `(iprop(⌊$φ⌋)) => ``(Iris.BI.BIBase.affinely (Iris.BI.BIBase.pure $φ))

-- A term expansion also supports clients with another notation for floor.
macro_rules
  | `(⌊$φ⌋) => ``(iprop(⌊$φ⌋))
