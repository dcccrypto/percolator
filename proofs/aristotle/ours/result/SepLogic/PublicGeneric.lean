module

public import SepLogic.Public

@[expose] public section

/-!
# Public vocabulary for generic library contracts

`GVocab` extends `Vocab` with what a contract of a generic library function needs to say about
its const and type arguments and the calls a client resolves, without the representation of Rust
types, the client's tables or the bodies. `SepLogic.PublicGenericBridge` interprets it in each
client program of a library and defines `LibraryRef.Satisfies`.

- `RTy` is a carrier of Rust type identities. It is nominal: two types with the same layout can
  select different trait implementations, so a type is never identified by its layout.
- `RConst` is a carrier of const arguments, such as the `N` of `[T; N]`.
- `represents T τ`: the client represents `T` as the layout type `τ`. A relation, so that a
  missing representation is never mistaken for a zero-sized one.
- `intConst v signed bytes N`: `N` is the integer `v` of the integer type with this signedness
  (`true` for signed) and width in bytes. A `usize` argument is `intConst v false 8`.
- `isArray E N T`: `T` is the array type `[E; N]`. `array` retains its inherited meaning of
  ownership of consecutive cells.
- `named n cs ts T`: `T` is the named type `n` (a struct, enum or union) at the const arguments
  `cs` and the type arguments `ts`.
- `instOf b cs ts c`: `c` is the client's callable instance of the generic body `b` at `cs` and
  `ts`.
- `resolvesTo i cs ts c`: the client resolves a call of the source item `i` at `cs` and `ts` to
  `c`.
- `fnConvention c .Rust` (inherited from `Vocab`): the resolved callable `c` exists and uses the
  Rust calling convention. State the convention needed at a call site alongside its behavioural
  contract; resolution and `wp_call` alone do not determine it.
- `assocTy i cs ts T`: the associated type `i` at `cs` and `ts` is `T` (`<I as Iterator>::Item`).
- `thin T`: pointers to `T` carry no metadata.
- `noRetag v τ`: a retag of `v` at `τ` leaves it, and the machine, as they are. A retag changes
  every shared reference, and every mutable reference or `Box` to an `Unpin` pointee; it leaves a
  mutable reference or `Box` to a `!Unpin` pointee alone. So a contract that asks `noRetag` of a
  type parameter's values excludes every type argument whose values hold a reference or `Box` that
  a retag changes, such as `&str`. A contract generic in a type takes their retags as a premise
  instead, `retaggable` (from `Vocab`), at an ownership it quantifies over, which a client
  discharges at its instance (`SepLogic.Retaggable`).

`instOf` and `resolvesTo` are partial: a client without an instance has none, and no handle is
made up for it. The handle-typed shorthands keep body, item and type handles apart.
-/

namespace Rust2Lean.Public
open Iris Iris.BI

/-- Operations available to a public generic specification, without their interpretation. -/
structure GVocab (PROP : Type) [BI PROP] extends Vocab PROP where
  RTy : Type
  RConst : Type
  represents : RTy → Ty → Prop
  intConst : Int → Bool → Nat → RConst → Prop
  isArray : RTy → RConst → RTy → Prop
  named : Int → List RConst → List RTy → RTy → Prop
  instOf : Int → List RConst → List RTy → Int → Prop
  resolvesTo : Int → List RConst → List RTy → Int → Prop
  assocTy : Int → List RConst → List RTy → RTy → Prop
  thin : RTy → Prop
  noRetag : Value → Ty → Prop

/-- `c` is the client's instance of the generic body `b` at the const arguments `cs` and the type
arguments `ts`. -/
abbrev GVocab.inst {PROP : Type} [BI PROP] (S : GVocab PROP) (b : RustBody) (cs : List S.RConst)
    (ts : List S.RTy) (c : Int) : Prop :=
  S.instOf b.id cs ts c

/-- The client resolves a call of the source item `i` at `cs` and `ts` to the callable `c`. -/
abbrev GVocab.resolves {PROP : Type} [BI PROP] (S : GVocab PROP) (i : RustItem)
    (cs : List S.RConst) (ts : List S.RTy) (c : Int) : Prop :=
  S.resolvesTo i.id cs ts c

/-- `T` is the named type `n` at the arguments `cs` and `ts`. -/
abbrev GVocab.isNamed {PROP : Type} [BI PROP] (S : GVocab PROP) (n : RustTypeName)
    (cs : List S.RConst) (ts : List S.RTy) (T : S.RTy) : Prop :=
  S.named n.id cs ts T

/-- The associated type `i` at `cs` and `ts` is `T`. -/
abbrev GVocab.assoc {PROP : Type} [BI PROP] (S : GVocab PROP) (i : RustItem)
    (cs : List S.RConst) (ts : List S.RTy) (T : S.RTy) : Prop :=
  S.assocTy i.id cs ts T

/-- A statement that can be interpreted in any public generic vocabulary. -/
abbrev GProperty := ∀ {PROP : Type} [BI PROP], GVocab PROP → Prop

end Rust2Lean.Public
