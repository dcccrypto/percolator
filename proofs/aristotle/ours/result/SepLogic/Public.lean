module

public import Iris.BI
public import Iris.Algebra.Frac
public import SepLogic.Pure

@[expose] public section

/-!
# Public specification vocabulary

Import this module and a generated `Public.lean` to state reusable properties
without importing program bodies, the interpreter, or its separation logic.
Named handles record Rust descriptions and stable numeric identities without
exposing bodies, signatures or representations. `Vocab` supplies operations and
carrier types, with no semantic laws.
This file and `SepLogic.Pure` can be distributed with public specifications;
their only external dependency is the public upstream Iris library.

Users can define their own predicates using Iris connectives and pure Lean
mathematics. `PublicBridge` interprets every operation using the existing
implementation and logic, and defines `Program.Satisfies`.

The carrier types are parameters so specifications are independent of their
concrete representations. In particular, a public pointer does not expose how
provenance or borrowing information is represented.

The constructors cover integers, booleans, thin raw pointers, unit and array
types, slice pointers, structs, tuples, enums and union values. Other types are
named through function signatures (`fnArgTy`, `fnRetTy`) or sized source fields
(`field`). A missing field has no type or offset; `field` returns `none`.
A borrow's give-back is a wand ending in `upd`, the update the logic's loans end
in.

Owned buffers keep their allocation right (`globalAlloc`) separate from their
contents. `pointsToAny` describes spare capacity without asserting that its
bytes are initialised. A contract returning an owned buffer retains both its
allocation right and contents; byte ownership alone does not supply a right to
free it.

Conventions for reading and writing values:

- `tupleVal` lists a struct's or tuple's fields in declaration order, every
  field included. Each field's value has its own type's shape: a fieldless
  unit-like field such as `()` or `PhantomData` is `unitVal`, but a zero-sized
  field with structure keeps it. Array values are `tupleVal`s of their
  elements, so `[(); 2]` is `tupleVal [unitVal, unitVal]`.
- `variantVal` takes the variant's Rust discriminant, not its index
  (`Ordering::Less` is `variantVal (-1) unitVal`), and the variant's fields as
  one value: a `tupleVal`, or `unitVal` for a variant without fields.
- Byte contents are initialised bytes without provenance (`List UInt8`). A run
  of `n` bytes is one cell of type `arrayTy (intTy false 1) n` holding the
  `tupleVal` of the byte values. The bridge reads it as the logic's bytes
  aligned to 1, so an empty run still says its pointer is valid.
- `Byte` also admits uninitialised bytes and bytes carrying pointer provenance.
  `encodesRaw` and `decodesRaw` retain those bytes; `encodes` remains the simpler
  relation for fully initialised, provenance-free bytes. A `unionVal` lists the
  byte contents of the union's data chunks, not its source fields.
-/

namespace Rust2Lean.Public
open Iris Iris.BI

/-- A named function handle. The stable numeric identity is resolved against the program;
`name` records the Rust description for readers. Neither field contains a body or signature. -/
structure RustFn where
  name : String
  id : Int
  deriving DecidableEq, Repr

/-- A named global handle, like `RustFn`: the global's stable numeric identity and, for readers,
its description. An anonymous constant's description is its contents. -/
structure RustGlobal where
  name : String
  id : Int
  deriving DecidableEq, Repr

/-- A generic body family's handle: a generic function body of a library, at no particular const
or type arguments. Its instances are callables of a client program (`GVocab.instOf`). -/
structure RustBody where
  name : String
  id : Int
  deriving DecidableEq, Repr

/-- A source item's handle: a function, trait method or associated type as source code names it,
which a client resolves at const and type arguments (`GVocab.resolvesTo`, `GVocab.assocTy`). -/
structure RustItem where
  name : String
  id : Int
  deriving DecidableEq, Repr

/-- A named Rust type's handle: a struct, enum or union definition, such as `Scalar`. -/
structure RustTypeName where
  name : String
  id : Int
  deriving DecidableEq, Repr

/-- Calling conventions available to public contracts. `C` and `SysV64` remain distinct even
on targets where they pass arguments alike. -/
inductive CallingConvention where
  | Rust
  | C
  | SysV64
  | Win64
  deriving DecidableEq, Repr

/-- The observable result of a call. Divergence has no result. -/
inductive CallOutcome (Value : Type) where
  | returned (value : Value)
  | unwound

/-- Operations available to a public specification, without their interpretation. -/
structure Vocab (PROP : Type) [BI PROP] where
  Ptr : Type
  Value : Type
  Ty : Type
  /-- A byte of storage, possibly uninitialised or carrying pointer provenance. -/
  Byte : Type
  uninitByte : Byte
  /-- An initialised byte without pointer provenance. -/
  initByte : UInt8 → Byte
  intVal : Int → Value
  boolVal : Bool → Value
  ptrVal : Ptr → Value
  unitVal : Value
  /-- Integer types, with signedness (`true` for signed) and width in bytes. -/
  intTy : Bool → Nat → Ty
  boolTy : Ty
  rawPtrTy : Ty
  unitTy : Ty
  arrayTy : Ty → Nat → Ty
  nullPtr : Ptr
  address : Ptr → Int
  /-- Add a byte offset, preserving provenance. This does not assert dereferenceability. -/
  offset : Ptr → Int → Ptr
  /-- Size and alignment in bytes, when the type has a local layout. -/
  layout : Ty → Option (Nat × Nat)
  /-- A sized field's byte offset and type, in declaration order, for a struct,
  tuple or union. Absent fields and other shapes return `none`. Looking through
  a validity restriction does not discharge that restriction. -/
  field : Ty → Nat → Option (Int × Ty)
  loadable : Value → Ty → Prop
  pointsTo : Ptr → Ty → Value → PROP
  pointsToF : Ptr → Qp → Ty → Value → PROP
  /-- Full ownership of a cell of this type's size and alignment, with arbitrary contents,
  including uninitialised bytes. This does not assert that the bytes encode a value. -/
  pointsToAny : Ptr → Ty → PROP
  /-- The right to deallocate a global-allocator block with this size and alignment in bytes.
  Its contents are owned separately. Size zero is `emp`: it supplies no allocation right and
  imposes no pointer-validity or alignment condition. -/
  globalAlloc : Ptr → Nat → Nat → PROP
  /-- A named function contract, including valid arguments and a buildable callee frame.
  Both outcomes preserve the incoming panic payload stack. Use `wp_call_unwind` when a panic
  changes that stack. Neither contract demands termination. -/
  wp_call : Int → List (Value × Ty) → (CallOutcome Value → PROP) → PROP
  /-- A slice pointer or reference (`&[T]`, `&mut [T]`): the data pointer and the element count. -/
  slicePtrVal : Ptr → Nat → Value
  /-- A struct or tuple value, with its fields in declaration order. -/
  tupleVal : List Value → Value
  /-- An enum value: the variant's Rust discriminant and its fields as one value. -/
  variantVal : Int → Value → Value
  /-- A union value's data chunks, including uninitialised and provenance-bearing
  bytes. Chunk order and lengths come from the union type; `loadable` checks them. -/
  unionVal : List (List Byte) → Value
  /-- The type of argument `i` (from 0) of the function with this identity. -/
  fnArgTy : Int → Nat → Ty
  /-- The return type of the function with this identity. -/
  fnRetTy : Int → Ty
  /-- The callable with this identity exists and uses this calling convention. A call-site
  contract states this separately from `wp_call` or `wp_call_unwind`; neither fixes an ABI. -/
  fnConvention : Int → CallingConvention → Prop
  /-- The bytes a value of this type is stored as, all initialised and without provenance. -/
  encodes : Ty → Value → List UInt8 → Prop
  /-- Exact stored bytes, including padding, uninitialised bytes and provenance. -/
  encodesRaw : Ty → Value → List Byte → Prop
  /-- These bytes decode successfully to this value. This does not assert its
  runtime validity; use `loadable` separately when required. -/
  decodesRaw : Ty → List Byte → Value → Prop
  /-- The global with this identity holds these bytes at this alignment, and always will:
  a persistent, read-only fact. -/
  globalBytes : Int → Nat → List UInt8 → PROP
  /-- The update that ends a loan: a borrow's give-back is a wand ending in it. -/
  upd : PROP → PROP
  /-- Both unprotected and function-entry retags of a loadable value preserve its ownership
  predicate and the additional resource. Their result may have different pointer provenance.
  A generic container tracks logical values `x` using predicates `Own x` and requires this
  contract for each `x`; this does not assert equality of the concrete representations. -/
  retaggable : Ty → (Value → PROP) → PROP → Prop
  /-- A named call with its initial panic payload stack, oldest payload first. The
  postcondition receives the actual final stack on return or unwind, including newly raised
  or caught panics. Payload pointers carry no ownership by themselves. -/
  wp_call_unwind : Int → List (Value × Ty) → List Ptr →
    (CallOutcome Value → List Ptr → PROP) → PROP

abbrev Vocab.call {PROP : Type} [BI PROP] (S : Vocab PROP) (f : RustFn)
    (args : List (S.Value × S.Ty)) (Q : CallOutcome S.Value → PROP) : PROP :=
  S.wp_call f.id args Q

/-- A named call that observes the panic payload stack on either outcome. -/
abbrev Vocab.callUnwind {PROP : Type} [BI PROP] (S : Vocab PROP) (f : RustFn)
    (args : List (S.Value × S.Ty)) (before : List S.Ptr)
    (Q : CallOutcome S.Value → List S.Ptr → PROP) : PROP :=
  S.wp_call_unwind f.id args before Q

/-- The type of argument `i` (from 0) of a named function. -/
abbrev Vocab.argTy {PROP : Type} [BI PROP] (S : Vocab PROP) (f : RustFn) (i : Nat) : S.Ty :=
  S.fnArgTy f.id i

/-- The return type of a named function. -/
abbrev Vocab.retTy {PROP : Type} [BI PROP] (S : Vocab PROP) (f : RustFn) : S.Ty :=
  S.fnRetTy f.id

/-- A named global holds these bytes at this alignment, and always will. -/
abbrev Vocab.global {PROP : Type} [BI PROP] (S : Vocab PROP) (g : RustGlobal) (align : Nat)
    (bytes : List UInt8) : PROP :=
  S.globalBytes g.id align bytes

/-- A postcondition allowing only normal return. -/
def Vocab.returns {PROP : Type} [BI PROP] (S : Vocab PROP) (Q : S.Value → PROP) :
    CallOutcome S.Value → PROP
  | .returned v => Q v
  | .unwound => iprop(False)

/-- Consecutive cells with an explicit byte stride. A specification can relate
`stride` to `S.layout ty`; the predicate itself makes no layout assumptions. -/
def Vocab.array {PROP : Type} [BI PROP] (S : Vocab PROP) (p : S.Ptr)
    (ty : S.Ty) (stride : Nat) (q : Qp) : List S.Value → PROP
  | [] => iprop(emp)
  | v :: vs => iprop(S.pointsToF p q ty v ∗ S.array (S.offset p stride) ty stride q vs)

/-- A statement that can be interpreted in any public vocabulary. -/
abbrev Property := ∀ {PROP : Type} [BI PROP], Vocab PROP → Prop

end Rust2Lean.Public
