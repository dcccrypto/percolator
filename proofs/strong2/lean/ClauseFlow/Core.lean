/-
ClauseFlow.Core: an abstract semantics for rustc MIR control-flow graphs with a value domain of
guarded clauses, a certificate checker, and the proof that the checker is sound.

Relation to `proofs/novel/lean/MirFlow/Core.lean` (session e2e-lean-fable-1121): same world model
(an opaque world `W`, opaque checks `chk k : W → Bool`), same trust style (a `Bool` checker
evaluated by the kernel plus one soundness theorem). The abstract state is different: instead of
one fact per local it is a list of literals about locals and a list of clauses

    lit₁ ∧ … ∧ litₙ ⟹ ⊥        or        lit₁ ∧ … ∧ litₙ ⟹ (checks hs hold; checks ps are preserved)

Nothing here mentions the engine. No axioms beyond Lean's core; see the end of the file.
-/

namespace ClauseFlow

abbrev Local := Nat
abbrev Block := Nat
abbrev Env := Local → Nat

/-! ## Literals and clauses -/

inductive Test where
  | eq (n : Nat)
  | ne (n : Nat)
deriving DecidableEq, Repr

def Test.holds : Test → Nat → Prop
  | .eq n, v => v = n
  | .ne n, v => v ≠ n

def Test.neg : Test → Test
  | .eq n => .ne n
  | .ne n => .eq n

/-- Two tests on the same value that cannot both hold. -/
def Test.conflict : Test → Test → Bool
  | .eq a, .eq b => decide (a ≠ b)
  | .eq a, .ne b => decide (a = b)
  | .ne a, .eq b => decide (a = b)
  | .ne _, .ne _ => false

/-- `s.imp t`: every value that passes `s` passes `t`. -/
def Test.imp : Test → Test → Bool
  | .eq a, .eq b => decide (a = b)
  | .eq a, .ne b => decide (a ≠ b)
  | .ne a, .ne b => decide (a = b)
  | .ne _, .eq _ => false

instance (t : Test) (v : Nat) : Decidable (t.holds v) := by
  cases t <;> simp only [Test.holds] <;> infer_instance

abbrev Lit := Local × Test

def Lit.holds (env : Env) (p : Lit) : Prop := p.2.holds (env p.1)

instance (env : Env) (p : Lit) : Decidable (Lit.holds env p) := by
  simp only [Lit.holds]; infer_instance

inductive Concl where
  | bot
  /-- every check in `hs` holds of the current world; every check in `ps` holds of the current
  world if it held of the reference world (the entry world, or for a call the world before it) -/
  | chk (hs ps : List Nat)
deriving DecidableEq, Repr

structure Clause where
  gs : List Lit
  c : Concl
deriving DecidableEq, Repr

def Concl.isBot : Concl → Bool
  | .bot => true
  | .chk _ _ => false

def Concl.hs : Concl → List Nat
  | .bot => []
  | .chk hs _ => hs

def Concl.ps : Concl → List Nat
  | .bot => []
  | .chk _ ps => ps

/-! ## Programs -/

inductive Stmt where
  /-- An assignment to (part of) local `l` whose value is not modelled. Does not touch the world. -/
  | havoc (l : Local)
  /-- Something that may write the world; the checks listed in `same` keep their value. -/
  | write (same : List Nat)
  /-- `_l = <constant d>` or an enum variant with discriminant `d`. -/
  | setD (l : Local) (d : Nat)
  /-- `_l = copy _m` and the like. -/
  | copy (l m : Local)
  /-- `_l = Eq(_m, const n)`: 1 if equal, 0 if not. -/
  | cmpEq (l m : Local) (n : Nat)
  /-- `_l = Ne(_m, const n)`: 0 if equal, 1 if not. -/
  | cmpNe (l m : Local) (n : Nat)
deriving DecidableEq, Repr

inductive Term where
  | goto (b : Block)
  | switch (l : Local) (cases : List (Nat × Block)) (other : Option Block)
  /-- A call. Locals outside `dsts` are unchanged. It may write the world, except that checks in
  `same` keep their value. Each clause of `post` holds of the locals after the call, its
  conclusion read with the world before the call as reference world. -/
  | call (dsts : List Local) (b : Block) (same : List Nat) (post : List Clause)
  /-- `<Result<_, _> as Try>::branch(src)`: `Ok` (0) gives `Continue` (0) with the same payload,
  `Err` gives `Break` (1). `pd`, `ps` are the payload ghosts of `dst`, `src`. -/
  | branch (dst src pd ps : Local) (b : Block)
  | ret
  /-- `unreachable`, `resume`, a diverging call: no successor, no return. -/
  | stop
deriving Repr

structure BlockD where
  stmts : List Stmt
  term : Term
deriving Repr

abbrev Cfg := List BlockD

def upd (env : Env) (l : Local) (v : Nat) : Env := fun x => if x = l then v else env x

def lookupCase : List (Nat × Block) → Nat → Option Block
  | [], _ => none
  | (v, b) :: t, x => if v = x then some b else lookupCase t x

section Sem
variable {W : Type} (chk : Nat → W → Bool)

def Concl.sat (w0 w : W) : Concl → Prop
  | .bot => False
  | .chk hs ps => (∀ k, k ∈ hs → chk k w = true) ∧ (∀ k, k ∈ ps → chk k w0 = true → chk k w = true)

def Clause.sat (w0 w : W) (env : Env) (c : Clause) : Prop :=
  (∀ g, g ∈ c.gs → Lit.holds env g) → c.c.sat chk w0 w

inductive SStep : Stmt → W → Env → W → Env → Prop where
  | havoc {l w env env'} : (∀ x, x ≠ l → env' x = env x) → SStep (.havoc l) w env w env'
  | write {same w w' env} : (∀ k, k ∈ same → chk k w' = chk k w) → SStep (.write same) w env w' env
  | setD {l d w env} : SStep (.setD l d) w env w (upd env l d)
  | copy {l m w env} : SStep (.copy l m) w env w (upd env l (env m))
  | cmpEq {l m n w env} : SStep (.cmpEq l m n) w env w (upd env l (if env m = n then 1 else 0))
  | cmpNe {l m n w env} : SStep (.cmpNe l m n) w env w (upd env l (if env m = n then 0 else 1))

inductive SSteps : List Stmt → W → Env → W → Env → Prop where
  | nil {w env} : SSteps [] w env w env
  | cons {s ss w env w1 env1 w2 env2} :
      SStep chk s w env w1 env1 → SSteps ss w1 env1 w2 env2 → SSteps (s :: ss) w env w2 env2

inductive TStep : Term → W → Env → Block → W → Env → Prop where
  | goto {b w env} : TStep (.goto b) w env b w env
  | switchCase {l cases other b w env} :
      lookupCase cases (env l) = some b → TStep (.switch l cases other) w env b w env
  | switchOther {l cases b w env} :
      lookupCase cases (env l) = none → TStep (.switch l cases (some b)) w env b w env
  | call {dsts b same post w env w' env'} :
      (∀ x, x ∉ dsts → env' x = env x) →
      (∀ k, k ∈ same → chk k w' = chk k w) →
      (∀ c, c ∈ post → Clause.sat chk w w' env' c) →
      TStep (.call dsts b same post) w env b w' env'
  | branch {dst src pd ps b w env env'} :
      (∀ x, x ≠ dst → x ≠ pd → env' x = env x) →
      (env' dst = if env src = 0 then 0 else 1) →
      (env src = 0 → env' pd = env ps) →
      TStep (.branch dst src pd ps b) w env b w env'

/-- `Reach ex cfg w0 b w env`: starting in block 0 with world `w0` and any locals, execution can
be at the start of block `b` with world `w` and locals `env`, without having executed a block
listed in `ex`. With `ex = []` this is every execution. -/
inductive Reach (ex : List Block) (cfg : Cfg) (w0 : W) : Block → W → Env → Prop where
  | entry (env : Env) : Reach ex cfg w0 0 w0 env
  | step {b bd w env w1 env1 b' w2 env2} :
      Reach ex cfg w0 b w env → b ∉ ex → cfg[b]? = some bd →
      SSteps chk bd.stmts w env w1 env1 → TStep chk bd.term w1 env1 b' w2 env2 →
      Reach ex cfg w0 b' w2 env2

/-- The function returns from block `b` with final world `w1` and locals `env1`, without having
executed a block in `ex`. -/
def Returns (ex : List Block) (cfg : Cfg) (w0 : W) (b : Block) (w1 : W) (env1 : Env) : Prop :=
  ∃ bd w env, Reach chk ex cfg w0 b w env ∧ b ∉ ex ∧ cfg[b]? = some bd ∧
    SSteps chk bd.stmts w env w1 env1 ∧ bd.term = .ret

end Sem

/-! ## Abstract states, entailment, transfer functions -/

structure AState where
  facts : List Lit
  cls : List Clause
deriving Repr

def inconsistent (L : List Lit) : Bool :=
  L.any (fun p => L.any (fun q => decide (p.1 = q.1) && p.2.conflict q.2))

def implied (L : List Lit) (g : Lit) : Bool :=
  L.any (fun q => decide (q.1 = g.1) && q.2.imp g.2)

/-- The clauses of `a` all of whose guards follow from the literals `L`. -/
def fired (a : AState) (L : List Lit) : List Clause :=
  a.cls.filter (fun d => d.gs.all (implied L))

/-- Literals that follow from `L` by one unit step: a ⊥-clause of `a` all of whose guards but
one are known gives the negation of the remaining guard. -/
def unitLits (a : AState) (L : List Lit) : List Lit :=
  a.cls.flatMap (fun d =>
    if d.c.isBot then
      match d.gs.filter (fun g => !implied L g) with
      | [g] => [(g.1, g.2.neg)]
      | _ => []
    else [])

/-- Two rounds of unit propagation. -/
def closure (a : AState) (L : List Lit) : List Lit :=
  unitLits a (unitLits a L ++ L) ++ (unitLits a L ++ L)

/-- `entailsCl a c`: clause `c` follows from the facts and clauses of `a`, by assuming the guards
of `c`, propagating units twice, and firing once every clause of `a` whose guards are then known. -/
def entailsCl (a : AState) (c : Clause) : Bool :=
  let L := closure a (c.gs ++ a.facts)
  inconsistent L ||
  (fired a L).any (fun d => d.c.isBot) ||
  (match c.c with
   | .bot => false
   | .chk hs ps =>
     hs.all (fun k => (fired a L).any (fun d => decide (k ∈ d.c.hs))) &&
     ps.all (fun k => (fired a L).any (fun d => decide (k ∈ d.c.ps) || decide (k ∈ d.c.hs))))

/-- A fact is the clause "its negation gives ⊥". -/
def factCl (f : Lit) : Clause := ⟨[(f.1, f.2.neg)], .bot⟩

/-- `le a b`: every fact and clause of `b` follows from `a`. -/
def le (a b : AState) : Bool :=
  b.facts.all (fun f => entailsCl a (factCl f)) && b.cls.all (entailsCl a)

def mentions (l : Local) (c : Clause) : Bool := c.gs.any (fun g => decide (g.1 = l))

def kill (l : Local) (a : AState) : AState :=
  { facts := a.facts.filter (fun f => decide (f.1 ≠ l)),
    cls := a.cls.filter (fun c => !mentions l c) }

def Concl.restrict (same : List Nat) : Concl → Concl
  | .bot => .bot
  | .chk hs ps => .chk (hs.filter (fun k => decide (k ∈ same))) (ps.filter (fun k => decide (k ∈ same)))

def clobber (same : List Nat) (a : AState) : AState :=
  { facts := a.facts, cls := a.cls.map (fun c => ⟨c.gs, c.c.restrict same⟩) }

def renameLit (m l : Local) (g : Lit) : Lit := if g.1 = m then (l, g.2) else g

def renameCl (m l : Local) (c : Clause) : Clause := ⟨c.gs.map (renameLit m l), c.c⟩

/-- For `l := (m == n)` or `l := (m != n)`: a guard on `m` against `n` restated as a guard on `l`. -/
def swapLit (m : Local) (n : Nat) (l : Local) (tEq tNe : Test) (g : Lit) : Lit :=
  if g = (m, Test.eq n) then (l, tEq) else if g = (m, Test.ne n) then (l, tNe) else g

def cmpCls (a : AState) (m : Local) (n : Nat) (l : Local) (tEq tNe : Test) : List Clause :=
  (a.cls.filter (mentions m)).map (fun c => ⟨c.gs.map (swapLit m n l tEq tNe), c.c⟩) ++ a.cls

def tStmt : Stmt → AState → AState
  | .havoc l, a => kill l a
  | .write same, a => clobber same a
  | .setD l d, a => let a1 := kill l a; { a1 with facts := (l, .eq d) :: a1.facts }
  | .copy l m, a =>
    if l = m then a else
    let a1 := kill l a
    { facts := ((a1.facts.filter (fun f => decide (f.1 = m))).map (fun f => (l, f.2))) ++ a1.facts,
      cls := ((a1.cls.filter (mentions m)).map (renameCl m l)) ++ a1.cls }
  | .cmpEq l m n, a =>
    if l = m then kill l a else
    let a1 := kill l a
    { a1 with cls := ⟨[(l, .eq 0), (m, .eq n)], .bot⟩ :: ⟨[(l, .ne 0), (m, .ne n)], .bot⟩ ::
        cmpCls a1 m n l (.ne 0) (.eq 0) }
  | .cmpNe l m n, a =>
    if l = m then kill l a else
    let a1 := kill l a
    { a1 with cls := ⟨[(l, .ne 0), (m, .eq n)], .bot⟩ :: ⟨[(l, .eq 0), (m, .ne n)], .bot⟩ ::
        cmpCls a1 m n l (.eq 0) (.ne 0) }

def tStmts : List Stmt → AState → AState
  | [], a => a
  | s :: ss, a => tStmts ss (tStmt s a)

def optEdge (other : Option Block) (a : AState) : List (Block × AState) :=
  match other with
  | some b => [(b, a)]
  | none => []

/-- Checks known to hold now, with no condition. -/
def knownH (a : AState) : List Nat :=
  (a.cls.filter (fun c => c.gs.isEmpty)).flatMap (fun c => c.c.hs)

/-- Checks known to be preserved from the entry world, with no condition. -/
def knownP (a : AState) : List Nat :=
  (a.cls.filter (fun c => c.gs.isEmpty)).flatMap (fun c => c.c.ps)

/-- A post-clause of a call, re-read with the entry world as reference: a check the call
preserves holds after it if it held before, and is preserved from entry if it was before. -/
def rebase (H P : List Nat) : Concl → Concl
  | .bot => .bot
  | .chk hs ps => .chk (hs ++ ps.filter (fun k => decide (k ∈ H))) (ps.filter (fun k => decide (k ∈ P)))

def killAll : List Local → AState → AState
  | [], a => a
  | l :: t, a => killAll t (kill l a)

def tTerm : Term → AState → List (Block × AState)
  | .goto b, a => [(b, a)]
  | .switch l cases other, a =>
    cases.map (fun c => (c.2, { a with facts := (l, .eq c.1) :: a.facts }))
      ++ optEdge other { a with facts := cases.map (fun c => (l, .ne c.1)) ++ a.facts }
  | .call dsts b same post, a =>
    let a1 := killAll dsts (clobber same a)
    [(b, { a1 with cls := post.map (fun c => ⟨c.gs, rebase (knownH a) (knownP a) c.c⟩) ++ a1.cls })]
  | .branch dst src pd ps b, a =>
    let a1 := kill dst (kill pd a)
    if dst = src || dst = ps || pd = src || pd = ps || dst = pd then [(b, a1)] else
    let moved := ((a1.cls ++ a1.facts.map factCl).filter (fun c => mentions src c || mentions ps c)).map
      (fun c => ⟨(dst, .eq 0) :: (c.gs.filter (fun g => decide (g ≠ (src, Test.eq 0)))).map (renameLit ps pd), c.c⟩)
    [(b, { a1 with cls :=
      ⟨[(src, .eq 0), (dst, .ne 0)], .bot⟩ :: ⟨[(src, .ne 0), (dst, .ne 1)], .bot⟩ ::
      ⟨[(dst, .eq 0), (src, .ne 0)], .bot⟩ :: (moved ++ a1.cls) })]
  | .ret, _ => []
  | .stop, _ => []

/-- What is to be shown at every `return`. -/
structure Goal where
  /-- clauses over the locals at the return (the return value is local 0) -/
  cls : List Clause
  /-- blocks excluded from the claim: it is about executions that do not pass through them -/
  exempt : List Block
  /-- the checks the entry state is allowed to call "preserved so far" -/
  init : List Nat
deriving Repr

def edgeOk (inv : List (Option AState)) (e : Block × AState) : Bool :=
  inconsistent (closure e.2 e.2.facts) ||
  match inv[e.1]? with
  | some (some c) => le e.2 c
  | _ => false

def blockOk (g : Goal) (cfg : Cfg) (inv : List (Option AState)) (b : Block) : Bool :=
  decide (b ∈ g.exempt) ||
  match cfg[b]?, inv[b]? with
  | some bd, some (some a) =>
    let a1 := tStmts bd.stmts a
    (tTerm bd.term a1).all (edgeOk inv) &&
    (match bd.term with
     | .ret => g.cls.all (entailsCl a1)
     | _ => true)
  | _, _ => true

def initState (g : Goal) : AState := { facts := [], cls := [⟨[], .chk [] g.init⟩] }

def checkCert (g : Goal) (cfg : Cfg) (inv : List (Option AState)) : Bool :=
  (match inv[0]? with
   | some (some c) => le (initState g) c
   | _ => false) &&
  (List.range cfg.length).all (blockOk g cfg inv)

/-! ## Soundness -/

theorem Test.neg_holds {t : Test} {v : Nat} : t.neg.holds v ↔ ¬ t.holds v := by
  cases t <;> simp [Test.neg, Test.holds]

theorem Test.conflict_sound {s t : Test} {v : Nat} (h : s.conflict t = true)
    (hs : s.holds v) (ht : t.holds v) : False := by
  cases s <;> cases t <;> simp [Test.conflict, Test.holds] at h hs ht <;> omega

theorem Test.imp_sound {s t : Test} {v : Nat} (h : s.imp t = true) (hs : s.holds v) : t.holds v := by
  cases s <;> cases t <;> simp [Test.imp, Test.holds] at h hs ⊢ <;> omega

theorem inconsistent_sound {L : List Lit} {env : Env} (hL : ∀ p, p ∈ L → Lit.holds env p) :
    inconsistent L = false := by
  cases h : inconsistent L with
  | false => rfl
  | true =>
    exfalso
    simp only [inconsistent, List.any_eq_true, Bool.and_eq_true, decide_eq_true_eq] at h
    obtain ⟨p, hp, q, hq, hpq, hc⟩ := h
    have h1 := hL p hp
    have h2 := hL q hq
    simp only [Lit.holds] at h1 h2
    rw [hpq] at h1
    exact Test.conflict_sound hc h1 h2

theorem implied_sound {L : List Lit} {env : Env} {g : Lit} (hL : ∀ p, p ∈ L → Lit.holds env p)
    (h : implied L g = true) : Lit.holds env g := by
  simp only [implied, List.any_eq_true, Bool.and_eq_true, decide_eq_true_eq] at h
  obtain ⟨q, hq, hqg, hi⟩ := h
  have h1 := hL q hq
  simp only [Lit.holds] at h1 ⊢
  rw [hqg] at h1
  exact Test.imp_sound hi h1

theorem lookupCase_mem {cases : List (Nat × Block)} {x : Nat} {b : Block} :
    lookupCase cases x = some b → (x, b) ∈ cases := by
  induction cases with
  | nil => intro h; simp [lookupCase] at h
  | cons p t ih =>
    obtain ⟨v, b'⟩ := p
    intro h
    simp only [lookupCase] at h
    by_cases hv : v = x
    · simp [hv] at h; subst hv; subst h; simp
    · simp [hv] at h; exact List.mem_cons_of_mem _ (ih h)

theorem lookupCase_none {cases : List (Nat × Block)} {x : Nat} :
    lookupCase cases x = none → ∀ c, c ∈ cases → c.1 ≠ x := by
  induction cases with
  | nil => intro _ c hc; simp at hc
  | cons p t ih =>
    obtain ⟨v, b'⟩ := p
    intro h c hc
    simp only [lookupCase] at h
    by_cases hv : v = x
    · simp [hv] at h
    · simp [hv] at h
      rcases List.mem_cons.mp hc with hc | hc
      · subst hc; exact hv
      · exact ih h c hc

section Sound
variable {W : Type} (chk : Nat → W → Bool)

def Gamma (a : AState) (w0 w : W) (env : Env) : Prop :=
  (∀ f, f ∈ a.facts → Lit.holds env f) ∧ (∀ c, c ∈ a.cls → Clause.sat chk w0 w env c)

theorem unitLits_sound {a : AState} {L : List Lit} {w0 w : W} {env : Env}
    (hg : Gamma chk a w0 w env) (hL : ∀ p, p ∈ L → Lit.holds env p) :
    ∀ p, p ∈ unitLits a L → Lit.holds env p := by
  intro p hp
  simp only [unitLits, List.mem_flatMap] at hp
  obtain ⟨d, hd, hpd⟩ := hp
  cases hb : d.c.isBot with
  | false => simp [hb] at hpd
  | true =>
    simp only [hb, ite_true] at hpd
    generalize hf : d.gs.filter (fun g => !implied L g) = fl at hpd
    cases fl with
    | nil => simp at hpd
    | cons g t =>
      cases t with
      | cons _ _ => simp at hpd
      | nil =>
        simp only [List.mem_singleton] at hpd
        subst hpd
        simp only [Lit.holds]
        apply Test.neg_holds.mpr
        intro hgh
        have hall : ∀ g', g' ∈ d.gs → Lit.holds env g' := by
          intro g' hg'
          by_cases hi : implied L g' = true
          · exact implied_sound hL hi
          · have hmem : g' ∈ d.gs.filter (fun g => !implied L g) := by
              simp only [List.mem_filter, Bool.not_eq_true'] 
              exact ⟨hg', by simpa using hi⟩
            rw [hf] at hmem
            simp only [List.mem_singleton] at hmem
            subst hmem; exact hgh
        have hs := hg.2 d hd hall
        cases hdc : d.c with
        | bot => rw [hdc] at hs; exact hs
        | chk _ _ => rw [hdc] at hb; simp [Concl.isBot] at hb

theorem closure_sound {a : AState} {L : List Lit} {w0 w : W} {env : Env}
    (hg : Gamma chk a w0 w env) (hL : ∀ p, p ∈ L → Lit.holds env p) :
    ∀ p, p ∈ closure a L → Lit.holds env p := by
  have h1 : ∀ p, p ∈ unitLits a L ++ L → Lit.holds env p := by
    intro p hp
    rcases List.mem_append.mp hp with hp | hp
    · exact unitLits_sound chk hg hL p hp
    · exact hL p hp
  intro p hp
  rcases List.mem_append.mp hp with hp | hp
  · exact unitLits_sound chk hg h1 p hp
  · exact h1 p hp

theorem fired_sat {a : AState} {L : List Lit} {w0 w : W} {env : Env}
    (hg : Gamma chk a w0 w env) (hL : ∀ p, p ∈ L → Lit.holds env p) :
    ∀ d, d ∈ fired a L → d.c.sat chk w0 w := by
  intro d hd
  simp only [fired, List.mem_filter, List.all_eq_true] at hd
  exact hg.2 d hd.1 (fun g hgm => implied_sound hL (hd.2 g hgm))

theorem entailsCl_sound {a : AState} {c : Clause} {w0 w : W} {env : Env}
    (hg : Gamma chk a w0 w env) (h : entailsCl a c = true) : Clause.sat chk w0 w env c := by
  intro hgs
  have hL0 : ∀ p, p ∈ c.gs ++ a.facts → Lit.holds env p := by
    intro p hp
    rcases List.mem_append.mp hp with hp | hp
    · exact hgs p hp
    · exact hg.1 p hp
  have hL := closure_sound chk hg hL0
  have hF := fired_sat chk hg hL
  simp only [entailsCl, Bool.or_eq_true] at h
  rcases h with (h | h) | h
  · rw [inconsistent_sound hL] at h; cases h
  · simp only [List.any_eq_true] at h
    obtain ⟨d, hd, hb⟩ := h
    have hs := hF d hd
    cases hdc : d.c with
    | bot => rw [hdc] at hs; exact hs.elim
    | chk hs' ps' => rw [hdc] at hb; simp [Concl.isBot] at hb
  · cases hcc : c.c with
    | bot => rw [hcc] at h; simp at h
    | chk hs ps =>
      rw [hcc] at h
      simp only [Bool.and_eq_true, List.all_eq_true, List.any_eq_true, Bool.or_eq_true,
        decide_eq_true_eq] at h
      refine ⟨fun k hk => ?_, fun k hk hw0 => ?_⟩
      · obtain ⟨d, hd, hkd⟩ := h.1 k hk
        have hs := hF d hd
        cases hdc : d.c with
        | bot => rw [hdc] at hs; exact hs.elim
        | chk hs' ps' =>
          rw [hdc] at hs hkd
          exact hs.1 k hkd
      · obtain ⟨d, hd, hkd⟩ := h.2 k hk
        have hs := hF d hd
        cases hdc : d.c with
        | bot => rw [hdc] at hs; exact hs.elim
        | chk hs' ps' =>
          rw [hdc] at hs hkd
          rcases hkd with hkd | hkd
          · exact hs.2 k hkd hw0
          · exact hs.1 k hkd

theorem fact_of_factCl {f : Lit} {w0 w : W} {env : Env}
    (h : Clause.sat chk w0 w env (factCl f)) : Lit.holds env f := by
  by_cases hf : Lit.holds env f
  · exact hf
  · exfalso
    apply h
    intro g hg
    simp only [factCl, List.mem_singleton] at hg
    subst hg
    simp only [Lit.holds] at hf ⊢
    exact Test.neg_holds.mpr hf

theorem factCl_sat {f : Lit} {w0 w : W} {env : Env} (h : Lit.holds env f) :
    Clause.sat chk w0 w env (factCl f) := by
  intro hg
  have := hg (f.1, f.2.neg) (by simp [factCl])
  simp only [Lit.holds] at this h
  exact Test.neg_holds.mp this h

theorem le_sound {a b : AState} {w0 w : W} {env : Env} (hle : le a b = true)
    (hg : Gamma chk a w0 w env) : Gamma chk b w0 w env := by
  simp only [le, Bool.and_eq_true, List.all_eq_true] at hle
  exact ⟨fun f hf => fact_of_factCl chk (entailsCl_sound chk hg (hle.1 f hf)),
         fun c hc => entailsCl_sound chk hg (hle.2 c hc)⟩

theorem not_mentions {l : Local} {c : Clause} (h : (!mentions l c) = true) :
    ∀ g, g ∈ c.gs → g.1 ≠ l := by
  intro g hg hgl
  simp only [mentions, Bool.not_eq_true', List.any_eq_false, decide_eq_true_eq] at h
  exact h g hg hgl

/-- Facts and clauses that do not mention `l` survive a change of `l` alone. -/
theorem gamma_kill {a : AState} {w0 w : W} {env env' : Env} {l : Local}
    (hag : ∀ x, x ≠ l → env' x = env x) (hg : Gamma chk a w0 w env) :
    Gamma chk (kill l a) w0 w env' := by
  refine ⟨fun f hf => ?_, fun c hc => ?_⟩
  · simp only [kill, List.mem_filter, decide_eq_true_eq] at hf
    have := hg.1 f hf.1
    simp only [Lit.holds] at this ⊢
    rw [hag f.1 hf.2]; exact this
  · simp only [kill, List.mem_filter] at hc
    intro hgs
    apply hg.2 c hc.1
    intro g hgm
    have := hgs g hgm
    simp only [Lit.holds] at this ⊢
    rw [← hag g.1 (not_mentions hc.2 g hgm)]; exact this

theorem restrict_sat {same : List Nat} {c : Concl} {w0 w w' : W}
    (hs : ∀ k, k ∈ same → chk k w' = chk k w) (h : c.sat chk w0 w) :
    (c.restrict same).sat chk w0 w' := by
  cases c with
  | bot => exact h
  | chk hs' ps' =>
    simp only [Concl.restrict, Concl.sat, List.mem_filter, decide_eq_true_eq] at h ⊢
    refine ⟨fun k hk => ?_, fun k hk hw0 => ?_⟩
    · rw [hs k hk.2]; exact h.1 k hk.1
    · rw [hs k hk.2]; exact h.2 k hk.1 hw0

theorem gamma_clobber {a : AState} {w0 w w' : W} {env : Env} {same : List Nat}
    (hs : ∀ k, k ∈ same → chk k w' = chk k w) (hg : Gamma chk a w0 w env) :
    Gamma chk (clobber same a) w0 w' env := by
  refine ⟨hg.1, fun c hc => ?_⟩
  simp only [clobber, List.mem_map] at hc
  obtain ⟨d, hd, rfl⟩ := hc
  intro hgs
  exact restrict_sat chk hs (hg.2 d hd hgs)

theorem upd_other {env : Env} {l : Local} {v : Nat} : ∀ x, x ≠ l → upd env l v x = env x := by
  intro x hx; simp [upd, hx]

theorem upd_self {env : Env} {l : Local} {v : Nat} : upd env l v l = v := by
  simp [upd]

theorem gamma_addFact {a : AState} {w0 w : W} {env : Env} {f : Lit}
    (hf : Lit.holds env f) (hg : Gamma chk a w0 w env) :
    Gamma chk { a with facts := f :: a.facts } w0 w env := by
  refine ⟨fun p hp => ?_, hg.2⟩
  rcases List.mem_cons.mp hp with hp | hp
  · subst hp; exact hf
  · exact hg.1 p hp

/-- The two clauses that define a comparison result `l` (value `v`) of `m` against `n`. -/
theorem gamma_cmp {a : AState} {w0 w : W} {env : Env} {l m : Local} {t1 t2 t3 t4 : Test}
    (h1 : t1.holds (env l) → t2.holds (env m) → False)
    (h2 : t3.holds (env l) → t4.holds (env m) → False)
    (hg : Gamma chk a w0 w env) :
    Gamma chk { a with cls := ⟨[(l, t1), (m, t2)], .bot⟩ :: ⟨[(l, t3), (m, t4)], .bot⟩ :: a.cls } w0 w env := by
  refine ⟨hg.1, fun c hc => ?_⟩
  rcases List.mem_cons.mp hc with hc | hc
  · subst hc
    intro hgs
    exact h1 (hgs (l, t1) (by simp)) (hgs (m, t2) (by simp))
  · rcases List.mem_cons.mp hc with hc | hc
    · subst hc
      intro hgs
      exact h2 (hgs (l, t3) (by simp)) (hgs (m, t4) (by simp))
    · exact hg.2 c hc

theorem swapLit_sound {env : Env} {m l : Local} {n : Nat} {tEq tNe : Test}
    (h1 : tEq.holds (env l) → env m = n) (h2 : tNe.holds (env l) → env m ≠ n) (g : Lit) :
    Lit.holds env (swapLit m n l tEq tNe g) → Lit.holds env g := by
  intro h
  by_cases e1 : g = (m, Test.eq n)
  · subst e1
    simp only [swapLit, ite_true] at h
    exact h1 h
  · by_cases e2 : g = (m, Test.ne n)
    · subst e2
      have hne : ((m, Test.ne n) : Lit) ≠ (m, Test.eq n) := by simp
      simp only [swapLit, hne, ite_false, ite_true] at h
      exact h2 h
    · simp only [swapLit, e1, e2, ite_false] at h; exact h

theorem gamma_cmpCls {a : AState} {w0 w : W} {env : Env} {m l : Local} {n : Nat} {tEq tNe : Test}
    (h1 : tEq.holds (env l) → env m = n) (h2 : tNe.holds (env l) → env m ≠ n)
    (hg : Gamma chk a w0 w env) :
    Gamma chk { a with cls := cmpCls a m n l tEq tNe } w0 w env := by
  refine ⟨hg.1, fun c hc => ?_⟩
  rcases List.mem_append.mp hc with hc | hc
  · simp only [List.mem_map, List.mem_filter] at hc
    obtain ⟨c0, ⟨hc0, _⟩, rfl⟩ := hc
    intro hgs
    apply hg.2 c0 hc0
    intro g hgm
    exact swapLit_sound h1 h2 g (hgs _ (List.mem_map_of_mem hgm))
  · exact hg.2 c hc

theorem tStmt_sound {s : Stmt} {a : AState} {w0 w w' : W} {env env' : Env}
    (hg : Gamma chk a w0 w env) (hs : SStep chk s w env w' env') :
    Gamma chk (tStmt s a) w0 w' env' := by
  cases hs with
  | havoc hag => exact gamma_kill chk hag hg
  | write hsame => exact gamma_clobber chk hsame hg
  | @setD l d =>
    exact gamma_addFact chk (by simp [Lit.holds, Test.holds, upd]) (gamma_kill chk upd_other hg)
  | @copy l m =>
    simp only [tStmt]
    by_cases hlm : l = m
    · simp only [hlm, ite_true]
      have : upd env m (env m) = env := by
        funext x; simp only [upd]; split
        · next h => rw [h]
        · rfl
      rw [this]; exact hg
    · simp only [hlm, ite_false]
      have hk := gamma_kill chk (l := l) (env' := upd env l (env m)) upd_other hg
      have hm : upd env l (env m) m = env m := upd_other m (fun h => hlm h.symm)
      refine ⟨fun f hf => ?_, fun c hc => ?_⟩
      · rcases List.mem_append.mp hf with hf | hf
        · simp only [List.mem_map, List.mem_filter, decide_eq_true_eq] at hf
          obtain ⟨f0, ⟨hf0, hf0m⟩, rfl⟩ := hf
          have := hk.1 f0 hf0
          simp only [Lit.holds] at this ⊢
          rw [hf0m, hm] at this
          rw [upd_self]; exact this
        · exact hk.1 f hf
      · rcases List.mem_append.mp hc with hc | hc
        · simp only [List.mem_map, List.mem_filter] at hc
          obtain ⟨c0, ⟨hc0, _⟩, rfl⟩ := hc
          intro hgs
          apply hk.2 c0 hc0
          intro g hgm
          have := hgs (renameLit m l g) (by simp only [renameCl]; exact List.mem_map_of_mem hgm)
          simp only [renameLit] at this
          by_cases hgm' : g.1 = m
          · simp only [hgm', ite_true, Lit.holds, upd_self] at this
            simp only [Lit.holds, hgm', hm]; exact this
          · simp only [hgm', ite_false] at this; exact this
        · exact hk.2 c hc
  | @cmpEq l m n =>
    simp only [tStmt]
    by_cases hlm : l = m
    · simp only [hlm, ite_true]; exact gamma_kill chk upd_other hg
    · simp only [hlm, ite_false]
      have hm : upd env l (if env m = n then 1 else 0) m = env m := upd_other m (fun h => hlm h.symm)
      refine gamma_cmp chk ?_ ?_ (gamma_cmpCls chk ?_ ?_ (gamma_kill chk upd_other hg))
      · simp only [Test.holds, upd_self, hm]
        intro h1 h2; simp [h2] at h1
      · simp only [Test.holds, upd_self, hm]
        intro h1 h2; simp [h2] at h1
      · simp only [Test.holds, upd_self, hm]
        intro h1
        by_cases hh : env m = n
        · exact hh
        · simp [hh] at h1
      · simp only [Test.holds, upd_self, hm]
        intro h1 hh; simp [hh] at h1
  | @cmpNe l m n =>
    simp only [tStmt]
    by_cases hlm : l = m
    · simp only [hlm, ite_true]; exact gamma_kill chk upd_other hg
    · simp only [hlm, ite_false]
      have hm : upd env l (if env m = n then 0 else 1) m = env m := upd_other m (fun h => hlm h.symm)
      refine gamma_cmp chk ?_ ?_ (gamma_cmpCls chk ?_ ?_ (gamma_kill chk upd_other hg))
      · simp only [Test.holds, upd_self, hm]
        intro h1 h2; simp [h2] at h1
      · simp only [Test.holds, upd_self, hm]
        intro h1 h2; simp [h2] at h1
      · simp only [Test.holds, upd_self, hm]
        intro h1
        by_cases hh : env m = n
        · exact hh
        · simp [hh] at h1
      · simp only [Test.holds, upd_self, hm]
        intro h1 hh; simp [hh] at h1

theorem tStmts_sound {ss : List Stmt} {a : AState} {w0 w w' : W} {env env' : Env}
    (hg : Gamma chk a w0 w env) (hs : SSteps chk ss w env w' env') :
    Gamma chk (tStmts ss a) w0 w' env' := by
  induction hs generalizing a with
  | nil => exact hg
  | cons h1 _ ih => exact ih (tStmt_sound chk hg h1)

theorem knownH_sound {a : AState} {w0 w : W} {env : Env} (hg : Gamma chk a w0 w env) :
    ∀ k, k ∈ knownH a → chk k w = true := by
  intro k hk
  simp only [knownH, List.mem_flatMap, List.mem_filter, List.isEmpty_iff] at hk
  obtain ⟨c, ⟨hc, hce⟩, hkc⟩ := hk
  have hs := hg.2 c hc (by intro g hgm; rw [hce] at hgm; cases hgm)
  cases hcc : c.c with
  | bot => rw [hcc] at hs; exact hs.elim
  | chk hs' ps' => rw [hcc] at hs hkc; exact hs.1 k hkc

theorem knownP_sound {a : AState} {w0 w : W} {env : Env} (hg : Gamma chk a w0 w env) :
    ∀ k, k ∈ knownP a → chk k w0 = true → chk k w = true := by
  intro k hk
  simp only [knownP, List.mem_flatMap, List.mem_filter, List.isEmpty_iff] at hk
  obtain ⟨c, ⟨hc, hce⟩, hkc⟩ := hk
  have hs := hg.2 c hc (by intro g hgm; rw [hce] at hgm; cases hgm)
  cases hcc : c.c with
  | bot => rw [hcc] at hs; exact hs.elim
  | chk hs' ps' => rw [hcc] at hs hkc; exact hs.2 k hkc

theorem gamma_killAll {dsts : List Local} {a : AState} {w0 w : W} {env env' : Env}
    (hag : ∀ x, x ∉ dsts → env' x = env x) (hg : Gamma chk a w0 w env) :
    Gamma chk (killAll dsts a) w0 w env' := by
  induction dsts generalizing a env with
  | nil =>
    have : env' = env := funext (fun x => hag x (by simp))
    rw [this]; exact hg
  | cons l t ih =>
    simp only [killAll]
    -- change `l` first (to its final value), then the rest
    have h1 : Gamma chk (kill l a) w0 w (upd env l (env' l)) := gamma_kill chk upd_other hg
    apply ih _ h1
    intro x hx
    by_cases hxl : x = l
    · subst hxl; simp [upd]
    · rw [upd_other x hxl]
      exact hag x (by simp [hxl, hx])

theorem mem_optEdge {other : Option Block} {a : AState} {b : Block} (h : other = some b) :
    (b, a) ∈ optEdge other a := by
  subst h; simp [optEdge]

/-- Every concrete step of a terminator is covered by an abstract edge. -/
theorem tTerm_sound {t : Term} {a : AState} {w0 w w' : W} {env env' : Env} {b' : Block}
    (hg : Gamma chk a w0 w env) (ht : TStep chk t w env b' w' env') :
    ∃ a', (b', a') ∈ tTerm t a ∧ Gamma chk a' w0 w' env' := by
  cases ht with
  | goto => exact ⟨a, by simp [tTerm], hg⟩
  | @switchCase l cases other _ _ _ hl =>
    have hmem := lookupCase_mem hl
    refine ⟨{ a with facts := (l, .eq (env l)) :: a.facts }, ?_, ?_⟩
    · simp only [tTerm, List.mem_append, List.mem_map]
      exact Or.inl ⟨_, hmem, rfl⟩
    · exact gamma_addFact chk (by simp [Lit.holds, Test.holds]) hg
  | @switchOther l cases _ _ _ hl =>
    have hnone := lookupCase_none hl
    refine ⟨{ a with facts := cases.map (fun c => (l, .ne c.1)) ++ a.facts }, ?_, ?_⟩
    · simp only [tTerm, List.mem_append]
      exact Or.inr (mem_optEdge rfl)
    · refine ⟨fun f hf => ?_, hg.2⟩
      rcases List.mem_append.mp hf with hf | hf
      · simp only [List.mem_map] at hf
        obtain ⟨c, hc, rfl⟩ := hf
        simp only [Lit.holds, Test.holds]
        exact fun h => hnone c hc h.symm
      · exact hg.1 f hf
  | @call dsts b same post _ _ _ _ hag hsame hpost =>
    have hc := gamma_clobber chk hsame hg
    have hk := gamma_killAll chk hag hc
    refine ⟨_, List.mem_singleton.mpr rfl, hk.1, fun c hcm => ?_⟩
    rcases List.mem_append.mp hcm with hcm | hcm
    · simp only [List.mem_map] at hcm
      obtain ⟨d, hd, rfl⟩ := hcm
      intro hgs
      have hs := hpost d hd hgs
      cases hdc : d.c with
      | bot => rw [hdc] at hs; exact hs.elim
      | chk hs' ps' =>
        rw [hdc] at hs
        simp only [rebase, Concl.sat, List.mem_append, List.mem_filter, decide_eq_true_eq] at hs ⊢
        refine ⟨fun k hk' => ?_, fun k hk' hw0 => ?_⟩
        · rcases hk' with hk' | hk'
          · exact hs.1 k hk'
          · exact hs.2 k hk'.1 (knownH_sound chk hg k hk'.2)
        · exact hs.2 k hk'.1 (knownP_sound chk hg k hk'.2 hw0)
    · exact hk.2 c hcm
  | @branch dst src pd ps b _ _ _ hag hdst hpd =>
    have hk : Gamma chk (kill dst (kill pd a)) w0 w env' := by
      have h1 : Gamma chk (kill pd a) w0 w (upd env pd (env' pd)) := gamma_kill chk upd_other hg
      apply gamma_kill chk _ h1
      intro x hx
      by_cases hxp : x = pd
      · subst hxp; simp [upd]
      · rw [upd_other x hxp]; exact hag x hx hxp
    simp only [tTerm]
    by_cases hal : (dst = src || dst = ps || pd = src || pd = ps || dst = pd) = true
    · simp only [hal, ite_true]
      exact ⟨_, List.mem_singleton.mpr rfl, hk⟩
    · simp only [hal]
      simp only [Bool.or_eq_true, decide_eq_true_eq, not_or] at hal
      obtain ⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩ := hal
      have hsrc : env' src = env src := hag src (fun h => h1 h.symm) (fun h => h3 h.symm)
      have hps : env' ps = env ps := hag ps (fun h => h2 h.symm) (fun h => h4 h.symm)
      refine ⟨_, List.mem_singleton.mpr rfl, hk.1, fun c hcm => ?_⟩
      rcases List.mem_cons.mp hcm with hcm | hcm
      · subst hcm
        intro hgs
        have a1 := hgs (src, .eq 0) (by simp)
        have a2 := hgs (dst, .ne 0) (by simp)
        simp only [Lit.holds, Test.holds] at a1 a2
        rw [hsrc] at a1
        rw [hdst, a1] at a2
        exact a2 rfl
      · rcases List.mem_cons.mp hcm with hcm | hcm
        · subst hcm
          intro hgs
          have a1 := hgs (src, .ne 0) (by simp)
          have a2 := hgs (dst, .ne 1) (by simp)
          simp only [Lit.holds, Test.holds] at a1 a2
          rw [hsrc] at a1
          rw [hdst] at a2
          simp [a1] at a2
        · rcases List.mem_cons.mp hcm with hcm | hcm
          · subst hcm
            intro hgs
            have a1 := hgs (dst, .eq 0) (by simp)
            have a2 := hgs (src, .ne 0) (by simp)
            simp only [Lit.holds, Test.holds] at a1 a2
            rw [hsrc] at a2
            rw [hdst] at a1
            simp [a2] at a1
          · rcases List.mem_append.mp hcm with hcm | hcm
            · simp only [List.mem_map, List.mem_filter] at hcm
              obtain ⟨c0, ⟨hc0, _⟩, rfl⟩ := hcm
              intro hgs
              have hd0 : env' dst = 0 := hgs (dst, .eq 0) (by simp)
              have hs0 : env src = 0 := by
                rw [hdst] at hd0
                by_cases hh : env src = 0
                · exact hh
                · simp [hh] at hd0
              have hpe : env' pd = env' ps := by rw [hps]; exact hpd hs0
              have hc0sat : Clause.sat chk w0 w env' c0 := by
                rcases List.mem_append.mp hc0 with hc0 | hc0
                · exact hk.2 c0 hc0
                · simp only [List.mem_map] at hc0
                  obtain ⟨f, hf, rfl⟩ := hc0
                  exact factCl_sat chk (hk.1 f hf)
              apply hc0sat
              intro g hgm
              by_cases hge : g = (src, Test.eq 0)
              · subst hge
                simp only [Lit.holds, Test.holds]
                rw [hsrc]; exact hs0
              · have := hgs (renameLit ps pd g) (by
                  apply List.mem_cons_of_mem
                  apply List.mem_map_of_mem
                  simp only [List.mem_filter, decide_eq_true_eq]
                  exact ⟨hgm, hge⟩)
                simp only [renameLit] at this
                by_cases hgp : g.1 = ps
                · simp only [hgp, ite_true, Lit.holds] at this
                  simp only [Lit.holds, hgp]
                  rw [← hpe]; exact this
                · simp only [hgp, ite_false] at this; exact this
            · exact hk.2 c hcm

theorem blockOk_of_check {g : Goal} {cfg : Cfg} {inv : List (Option AState)}
    (h : checkCert g cfg inv = true) {b : Block} {bd : BlockD} (hb : cfg[b]? = some bd) :
    blockOk g cfg inv b = true := by
  simp only [checkCert, Bool.and_eq_true, List.all_eq_true, List.mem_range] at h
  have hlt : b < cfg.length := by
    rcases Nat.lt_or_ge b cfg.length with hlt | hge
    · exact hlt
    · rw [List.getElem?_eq_none hge] at hb; cases hb
  exact h.2 b hlt

theorem edgeOk_sound {inv : List (Option AState)} {b : Block} {a : AState} {w0 w : W} {env : Env}
    (he : edgeOk inv (b, a) = true) (hg : Gamma chk a w0 w env) :
    ∃ c, inv[b]? = some (some c) ∧ Gamma chk c w0 w env := by
  simp only [edgeOk, Bool.or_eq_true] at he
  rcases he with he | he
  · rw [inconsistent_sound (closure_sound chk hg hg.1)] at he; cases he
  · cases hi : inv[b]? with
    | none => simp [hi] at he
    | some o =>
      cases o with
      | none => simp [hi] at he
      | some c =>
        simp only [hi] at he
        exact ⟨c, rfl, le_sound chk he hg⟩

/-- The certificate describes every reachable state. -/
theorem reach_sound {g : Goal} {cfg : Cfg} {inv : List (Option AState)}
    (h : checkCert g cfg inv = true) {w0 w : W} {b : Block} {env : Env}
    (hr : Reach chk g.exempt cfg w0 b w env) :
    ∃ a, inv[b]? = some (some a) ∧ Gamma chk a w0 w env := by
  induction hr with
  | entry env =>
    have h0 := h
    simp only [checkCert, Bool.and_eq_true] at h0
    have h1 := h0.1
    cases hi : inv[0]? with
    | none => simp [hi] at h1
    | some o =>
      cases o with
      | none => simp [hi] at h1
      | some c =>
        simp only [hi] at h1
        refine ⟨c, rfl, le_sound chk h1 ?_⟩
        refine ⟨?_, ?_⟩
        · intro f hf; simp [initState] at hf
        · intro c hc
          simp only [initState, List.mem_singleton] at hc
          subst hc
          intro _
          exact ⟨fun k hk => (by cases hk), fun k _ hw0 => hw0⟩
  | @step b bd w env w1 env1 b' w2 env2 _ hne hb hss hts ih =>
    obtain ⟨a, hia, hga⟩ := ih
    have hbo := blockOk_of_check h hb
    simp only [blockOk, hb, hia, Bool.or_eq_true, decide_eq_true_eq, hne, false_or,
      Bool.and_eq_true, List.all_eq_true] at hbo
    have hg1 := tStmts_sound chk hga hss
    obtain ⟨a', hmem, hg'⟩ := tTerm_sound chk hg1 hts
    exact edgeOk_sound chk (hbo.1 _ hmem) hg'

/-- **Soundness of the checker.** If the certificate checks, then at every return of the abstract
program that did not pass through an exempt block, every clause of the goal holds of the final
locals and world, with the entry world as reference world. -/
theorem checkCert_sound {g : Goal} {cfg : Cfg} {inv : List (Option AState)}
    (h : checkCert g cfg inv = true) {w0 w1 : W} {b : Block} {env1 : Env}
    (hret : Returns chk g.exempt cfg w0 b w1 env1) :
    ∀ c, c ∈ g.cls → Clause.sat chk w0 w1 env1 c := by
  obtain ⟨bd, w, env, hr, hne, hb, hss, hterm⟩ := hret
  obtain ⟨a, hia, hga⟩ := reach_sound chk h hr
  have hbo := blockOk_of_check h hb
  simp only [blockOk, hb, hia, hterm, Bool.or_eq_true, decide_eq_true_eq, hne, false_or,
    Bool.and_eq_true, List.all_eq_true] at hbo
  have hg1 := tStmts_sound chk hga hss
  intro c hc
  exact entailsCl_sound chk hg1 (hbo.2 c hc)

/-- **Composition.** A certified callee justifies the `call` instruction that stands for it in a
caller. If the callee's certificate checks for a goal whose clauses mention only the return
value (local 0) and its payload ghost (local `po`), then any actual return of the callee from
world `w` to world `w'` is a step of the caller's `call` whose post-clauses are the goal's
clauses with 0 renamed to `dst` and `po` to `pd`, provided the frame part `same` holds for that
call (that part comes from the types of the arguments, not from the certificate). -/
theorem call_of_cert {g : Goal} {cfgC : Cfg} {invC : List (Option AState)}
    (hc : checkCert g cfgC invC = true) (hex : g.exempt = [])
    {po : Local} (hpo : po ≠ 0)
    (hvoc : ∀ c, c ∈ g.cls → ∀ q, q ∈ c.gs → q.1 = 0 ∨ q.1 = po)
    {w w' : W} {bC : Block} {envC : Env}
    (hret : Returns chk [] cfgC w bC w' envC)
    {same : List Nat} (hsame : ∀ k, k ∈ same → chk k w' = chk k w)
    (env : Env) (dst pd : Local) (hdp : dst ≠ pd) (b : Block) :
    TStep chk (.call [dst, pd] b same
        (g.cls.map (fun c => ⟨c.gs.map (fun q => if q.1 = 0 then (dst, q.2) else (pd, q.2)), c.c⟩)))
      w env b w' (upd (upd env dst (envC 0)) pd (envC po)) := by
  refine TStep.call ?_ hsame ?_
  · intro x hx
    simp only [List.mem_cons, List.not_mem_nil, or_false, not_or] at hx
    rw [upd_other x hx.2, upd_other x hx.1]
  · intro c hcm
    simp only [List.mem_map] at hcm
    obtain ⟨c0, hc0, rfl⟩ := hcm
    intro hgs
    have hret' : Returns chk g.exempt cfgC w bC w' envC := by rw [hex]; exact hret
    apply checkCert_sound chk hc hret' c0 hc0
    intro q hq
    have := hgs _ (List.mem_map_of_mem (f := fun q : Lit => if q.1 = 0 then (dst, q.2) else (pd, q.2)) hq)
    rcases hvoc c0 hc0 q hq with h0 | h0
    · simp only [h0, ite_true, Lit.holds] at this
      rw [upd_other dst hdp, upd_self] at this
      simp only [Lit.holds, h0]; exact this
    · have hq0 : q.1 ≠ 0 := by rw [h0]; exact hpo
      simp only [hq0, ite_false, Lit.holds, upd_self] at this
      simp only [Lit.holds, h0]; exact this

end Sound

end ClauseFlow

#print axioms ClauseFlow.checkCert_sound
#print axioms ClauseFlow.call_of_cert
