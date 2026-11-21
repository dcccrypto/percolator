/-
MirFlow.Core: an abstract semantics for rustc MIR control-flow graphs, a certificate checker
for "checked-exit" facts, and the proof that the checker is sound.

Nothing here mentions the engine. The world `W` is an opaque type (everything reachable from the
function's reference arguments) and the checks are opaque functions `chk k : W → Bool`
("runtime validator number k returns Ok on this world").

No axioms beyond Lean's core are used; see `#print axioms` at the end of the file.
-/

namespace MirFlow

abbrev Local := Nat
abbrev Block := Nat
abbrev Env := Local → Nat

/-- What the checker may know about a local. `const n`: its value (for an enum: its
discriminant) is `n`. `cond hs ps`: if it is `0`, then every check in `hs` holds of the current
world and every check in `ps` holds of the current world provided it held of the entry world. -/
inductive AVal where
  | const (n : Nat)
  | cond (hs ps : List Nat)
deriving DecidableEq, Repr

inductive Stmt where
  /-- An assignment to (part of) local `l` whose value is not modelled. Does not touch the world. -/
  | havoc (l : Local)
  /-- Something that may write the world; the checks listed in `same` keep their value. -/
  | write (same : List Nat)
  /-- `_l = <enum variant with discriminant d>` or an integer/bool constant `d`. -/
  | setD (l : Local) (d : Nat)
  /-- `_l = copy _m`, `_l = move _m`, `_l = discriminant(_m)`. -/
  | copy (l m : Local)
deriving DecidableEq, Repr

inductive Term where
  | goto (b : Block)
  | switch (l : Local) (cases : List (Nat × Block)) (other : Option Block)
  /-- A call. It may write the world, except that checks in `same` keep their value.
  If `uncond`, or if the result (in `dst`) has discriminant 0 (`Ok`): each check in `pres`
  that held before the call holds after it, and each check in `est` holds after it. -/
  | call (dst : Local) (b : Block) (same pres est : List Nat) (uncond : Bool)
  /-- `<Result<_, _> as Try>::branch(src)`: `Ok` (0) gives `Continue` (0), `Err` gives `Break` (1). -/
  | branch (dst src : Local) (b : Block)
  /-- `<Result<_, _> as FromResidual<Result<Infallible, _>>>::from_residual(_)`: always `Err` (1). -/
  | residual (dst : Local) (b : Block)
  | ret
  /-- `unreachable`, `resume`, a diverging call: no successor, no return. -/
  | stop
deriving DecidableEq, Repr

structure BlockD where
  stmts : List Stmt
  term : Term
deriving Repr

abbrev Cfg := List BlockD

/-! ## Concrete (still abstract-world) semantics -/

def upd (env : Env) (l : Local) (v : Nat) : Env := fun x => if x = l then v else env x

section Sem
variable {W : Type} (chk : Nat → W → Bool)

inductive SStep : Stmt → W → Env → W → Env → Prop where
  | havoc {l w env env'} : (∀ x, x ≠ l → env' x = env x) → SStep (.havoc l) w env w env'
  | write {same w w' env} : (∀ k, k ∈ same → chk k w' = chk k w) → SStep (.write same) w env w' env
  | setD {l d w env} : SStep (.setD l d) w env w (upd env l d)
  | copy {l m w env} : SStep (.copy l m) w env w (upd env l (env m))

inductive SSteps : List Stmt → W → Env → W → Env → Prop where
  | nil {w env} : SSteps [] w env w env
  | cons {s ss w env w1 env1 w2 env2} :
      SStep chk s w env w1 env1 → SSteps ss w1 env1 w2 env2 → SSteps (s :: ss) w env w2 env2

def lookupCase : List (Nat × Block) → Nat → Option Block
  | [], _ => none
  | (v, b) :: t, x => if v = x then some b else lookupCase t x

inductive TStep : Term → W → Env → Block → W → Env → Prop where
  | goto {b w env} : TStep (.goto b) w env b w env
  | switchCase {l cases other b w env} :
      lookupCase cases (env l) = some b → TStep (.switch l cases other) w env b w env
  | switchOther {l cases b w env} :
      lookupCase cases (env l) = none → TStep (.switch l cases (some b)) w env b w env
  | call {dst b same pres est uncond w env w' env'} :
      (∀ x, x ≠ dst → env' x = env x) →
      (∀ k, k ∈ same → chk k w' = chk k w) →
      (∀ k, k ∈ pres → (uncond = true ∨ env' dst = 0) → chk k w = true → chk k w' = true) →
      (∀ k, k ∈ est → (uncond = true ∨ env' dst = 0) → chk k w' = true) →
      TStep (.call dst b same pres est uncond) w env b w' env'
  | branch {dst src b w env} :
      TStep (.branch dst src b) w env b w (upd env dst (if env src = 0 then 0 else 1))
  | residual {dst b w env} : TStep (.residual dst b) w env b w (upd env dst 1)

/-- `Reach ex cfg w0 b w env`: starting in block 0 with world `w0` and any locals, execution can
be at the start of block `b` with world `w` and locals `env`, without having executed a block
listed in `ex`. With `ex = []` this is every execution. -/
inductive Reach (ex : List Block) (cfg : Cfg) (w0 : W) : Block → W → Env → Prop where
  | entry (env : Env) : Reach ex cfg w0 0 w0 env
  | step {b bd w env w1 env1 b' w2 env2} :
      Reach ex cfg w0 b w env → b ∉ ex → cfg[b]? = some bd →
      SSteps chk bd.stmts w env w1 env1 → TStep chk bd.term w1 env1 b' w2 env2 →
      Reach ex cfg w0 b' w2 env2

/-- The function returns from block `b` with final world `w1` and locals `env1`
(`env1 0` is the discriminant of the return value), without having executed a block in `ex`. -/
def Returns (ex : List Block) (cfg : Cfg) (w0 : W) (b : Block) (w1 : W) (env1 : Env) : Prop :=
  ∃ bd w env, Reach chk ex cfg w0 b w env ∧ b ∉ ex ∧ cfg[b]? = some bd ∧
    SSteps chk bd.stmts w env w1 env1 ∧ bd.term = .ret

end Sem

/-! ## Abstract states and transfer functions -/

structure AState where
  /-- checks known to hold of the current world -/
  H : List Nat
  /-- checks known to hold of the current world if they held of the entry world -/
  Pr : List Nat
  av : List (Local × AVal)
deriving DecidableEq, Repr

def getAv : List (Local × AVal) → Local → Option AVal
  | [], _ => none
  | (k, v) :: t, l => if k = l then some v else getAv t l

def kill (l : Local) (av : List (Local × AVal)) : List (Local × AVal) :=
  av.filter (fun p => decide (p.1 ≠ l))

def AVal.stable (same : List Nat) : AVal → Bool
  | .const _ => true
  | .cond hs ps => hs.all (fun k => decide (k ∈ same)) && ps.all (fun k => decide (k ∈ same))

def clobber (same : List Nat) (a : AState) : AState :=
  { H := a.H.filter (fun k => decide (k ∈ same)),
    Pr := a.Pr.filter (fun k => decide (k ∈ same)),
    av := a.av.filter (fun p => p.2.stable same) }

def tStmt : Stmt → AState → AState
  | .havoc l, a => { a with av := kill l a.av }
  | .write same, a => clobber same a
  | .setD l d, a => { a with av := (l, .const d) :: kill l a.av }
  | .copy l m, a =>
    match getAv a.av m with
    | some v => { a with av := (l, v) :: kill l a.av }
    | none => { a with av := kill l a.av }

def tStmts : List Stmt → AState → AState
  | [], a => a
  | s :: ss, a => tStmts ss (tStmt s a)

def optEdge (other : Option Block) (a : AState) : List (Block × AState) :=
  match other with
  | some b => [(b, a)]
  | none => []

def tTerm : Term → AState → List (Block × AState)
  | .goto b, a => [(b, a)]
  | .switch l cases other, a =>
    match getAv a.av l with
    | some (.const n) =>
      match lookupCase cases n with
      | some b => [(b, a)]
      | none => optEdge other a
    | some (.cond hs ps) =>
      cases.map (fun c => (c.2, if c.1 = 0 then { a with H := hs ++ a.H, Pr := ps ++ a.Pr } else a))
        ++ optEdge other a
    | none => cases.map (fun c => (c.2, a)) ++ optEdge other a
  | .call dst b same pres est uncond, a =>
    let a1 := clobber same a
    let hs := est ++ pres.filter (fun k => decide (k ∈ a.H))
    let ps := pres.filter (fun k => decide (k ∈ a.Pr))
    if uncond then
      [(b, { H := hs ++ a1.H, Pr := ps ++ a1.Pr, av := kill dst a1.av })]
    else
      [(b, { H := a1.H, Pr := a1.Pr, av := (dst, .cond hs ps) :: kill dst a1.av })]
  | .branch dst src b, a =>
    match getAv a.av src with
    | some (.const n) => [(b, { a with av := (dst, .const (if n = 0 then 0 else 1)) :: kill dst a.av })]
    | some (.cond hs ps) => [(b, { a with av := (dst, .cond hs ps) :: kill dst a.av })]
    | none => [(b, { a with av := kill dst a.av })]
  | .residual dst b, a => [(b, { a with av := (dst, .const 1) :: kill dst a.av })]
  | .ret, _ => []
  | .stop, _ => []

/-- `avLe a p`: the fact `p` about a local follows from the facts of `a`. Either it is one of
them, or it is a conditional fact that follows from what `a` knows about the same local: the
local is known to be nonzero (the condition is vacuous), or it is known to be zero and the
conclusions are facts of `a`, or `a` has a conditional fact that is at least as strong. -/
def avLe (a : AState) (p : Local × AVal) : Bool :=
  decide (p ∈ a.av) ||
  match p.2 with
  | .const _ => false
  | .cond hs ps =>
    match getAv a.av p.1 with
    | some (.const n) =>
      decide (n ≠ 0) ||
      (hs.all (fun k => decide (k ∈ a.H)) && ps.all (fun k => decide (k ∈ a.Pr) || decide (k ∈ a.H)))
    | some (.cond hs' ps') =>
      hs.all (fun k => decide (k ∈ hs') || decide (k ∈ a.H)) &&
      ps.all (fun k => decide (k ∈ ps') || decide (k ∈ hs') || decide (k ∈ a.Pr) || decide (k ∈ a.H))
    | none => false

/-- `le a b`: every fact of `b` follows from the facts of `a`. -/
def le (a b : AState) : Bool :=
  b.H.all (fun k => decide (k ∈ a.H)) &&
  b.Pr.all (fun k => decide (k ∈ a.Pr) || decide (k ∈ a.H)) &&
  b.av.all (avLe a)

/-- What is to be shown at every `return`. -/
structure Goal where
  /-- checks that must hold of the returned world -/
  req : List Nat
  /-- checks that must hold of the returned world if they held of the entry world -/
  reqP : List Nat
  /-- blocks excluded from the claim: it is about executions that do not pass through them -/
  exempt : List Block
  /-- the claim is only about returns whose value has discriminant 0 (`Ok`) -/
  okOnly : Bool
deriving Repr

def isErr (a : AState) : Bool :=
  match getAv a.av 0 with
  | some (.const n) => decide (n ≠ 0)
  | _ => false

/-- The return value is known to carry the goal: "if it is `Ok` then the checks hold". -/
def condOk (g : Goal) (a : AState) : Bool :=
  match getAv a.av 0 with
  | some (.cond hs ps) =>
    g.req.all (fun k => decide (k ∈ hs) || decide (k ∈ a.H)) &&
    g.reqP.all (fun k => decide (k ∈ ps) || decide (k ∈ hs) || decide (k ∈ a.Pr) || decide (k ∈ a.H))
  | _ => false

def retOk (g : Goal) (a : AState) : Bool :=
  (g.okOnly && isErr a) || (g.okOnly && condOk g a) ||
  (g.req.all (fun k => decide (k ∈ a.H)) &&
   g.reqP.all (fun k => decide (k ∈ a.Pr) || decide (k ∈ a.H)))

def edgeOk (inv : List (Option AState)) (e : Block × AState) : Bool :=
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
     | .ret => retOk g a1
     | _ => true)
  | _, _ => true

def initState (g : Goal) : AState := { H := [], Pr := g.reqP, av := [] }

def checkCert (g : Goal) (cfg : Cfg) (inv : List (Option AState)) : Bool :=
  edgeOk inv (0, initState g) && (List.range cfg.length).all (blockOk g cfg inv)

/-! ## Soundness -/

section Sound
variable {W : Type} (chk : Nat → W → Bool)

def AVal.sat (w0 w : W) (env : Env) (l : Local) : AVal → Prop
  | .const n => env l = n
  | .cond hs ps => env l = 0 →
      (∀ k, k ∈ hs → chk k w = true) ∧ (∀ k, k ∈ ps → chk k w0 = true → chk k w = true)

def Gamma (a : AState) (w0 w : W) (env : Env) : Prop :=
  (∀ k, k ∈ a.H → chk k w = true) ∧
  (∀ k, k ∈ a.Pr → chk k w0 = true → chk k w = true) ∧
  (∀ p, p ∈ a.av → p.2.sat chk w0 w env p.1)

theorem getAv_mem {av : List (Local × AVal)} {l : Local} {v : AVal} :
    getAv av l = some v → (l, v) ∈ av := by
  induction av with
  | nil => intro h; simp [getAv] at h
  | cons p t ih =>
    obtain ⟨k, v'⟩ := p
    intro h
    simp only [getAv] at h
    by_cases hk : k = l
    · simp [hk] at h; subst hk; subst h; simp
    · simp [hk] at h; exact List.mem_cons_of_mem _ (ih h)

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

theorem mem_kill {l : Local} {av : List (Local × AVal)} {p : Local × AVal} :
    p ∈ kill l av → p ∈ av ∧ p.1 ≠ l := by
  intro h
  simpa [kill] using h

/-- A fact about a local other than `l` survives a change of `l` alone. -/
theorem sat_of_agree {w0 w : W} {env env' : Env} {l : Local} {v : AVal}
    (h : env' l = env l) : v.sat chk w0 w env l → v.sat chk w0 w env' l := by
  cases v with
  | const n => simp [AVal.sat, h]
  | cond hs ps => simp [AVal.sat, h]

theorem sat_stable {w0 w w' : W} {env : Env} {l : Local} {v : AVal} {same : List Nat}
    (hs : ∀ k, k ∈ same → chk k w' = chk k w) (hst : v.stable same = true) :
    v.sat chk w0 w env l → v.sat chk w0 w' env l := by
  cases v with
  | const n => simp [AVal.sat]
  | cond hs' ps' =>
    simp only [AVal.stable, Bool.and_eq_true, List.all_eq_true, decide_eq_true_eq] at hst
    intro h h0
    obtain ⟨h1, h2⟩ := h h0
    refine ⟨fun k hk => ?_, fun k hk hw0 => ?_⟩
    · rw [hs k (hst.1 k hk)]; exact h1 k hk
    · rw [hs k (hst.2 k hk)]; exact h2 k hk hw0

theorem gamma_clobber {a : AState} {w0 w w' : W} {env : Env} {same : List Nat}
    (hs : ∀ k, k ∈ same → chk k w' = chk k w) (hg : Gamma chk a w0 w env) :
    Gamma chk (clobber same a) w0 w' env := by
  obtain ⟨hH, hP, hA⟩ := hg
  refine ⟨fun k hk => ?_, fun k hk hw0 => ?_, fun p hp => ?_⟩
  · simp only [clobber, List.mem_filter, decide_eq_true_eq] at hk
    rw [hs k hk.2]; exact hH k hk.1
  · simp only [clobber, List.mem_filter, decide_eq_true_eq] at hk
    rw [hs k hk.2]; exact hP k hk.1 hw0
  · simp only [clobber, List.mem_filter] at hp
    exact sat_stable chk hs hp.2 (hA p hp.1)

/-- Facts about locals other than `l` survive a change of `l` alone. -/
theorem gamma_kill {a : AState} {w0 w : W} {env env' : Env} {l : Local}
    (hag : ∀ x, x ≠ l → env' x = env x) (hg : Gamma chk a w0 w env) :
    Gamma chk { a with av := kill l a.av } w0 w env' := by
  obtain ⟨hH, hP, hA⟩ := hg
  refine ⟨hH, hP, fun p hp => ?_⟩
  obtain ⟨hp1, hp2⟩ := mem_kill hp
  exact sat_of_agree chk (hag p.1 hp2) (hA p hp1)

theorem gamma_set {a : AState} {w0 w : W} {env env' : Env} {l : Local} {v : AVal}
    (hag : ∀ x, x ≠ l → env' x = env x) (hv : v.sat chk w0 w env' l)
    (hg : Gamma chk a w0 w env) :
    Gamma chk { a with av := (l, v) :: kill l a.av } w0 w env' := by
  have hk := gamma_kill chk hag hg
  obtain ⟨hH, hP, hA⟩ := hk
  refine ⟨hH, hP, fun p hp => ?_⟩
  simp only [List.mem_cons] at hp
  rcases hp with hp | hp
  · subst hp; exact hv
  · exact hA p hp

theorem upd_other {env : Env} {l : Local} {v : Nat} : ∀ x, x ≠ l → upd env l v x = env x := by
  intro x hx; simp [upd, hx]

theorem upd_self {env : Env} {l : Local} {v : Nat} : upd env l v l = v := by
  simp [upd]

theorem tStmt_sound {s : Stmt} {a : AState} {w0 w w' : W} {env env' : Env}
    (hg : Gamma chk a w0 w env) (hs : SStep chk s w env w' env') :
    Gamma chk (tStmt s a) w0 w' env' := by
  cases hs with
  | havoc hag => exact gamma_kill chk hag hg
  | write hsame => exact gamma_clobber chk hsame hg
  | setD =>
    exact gamma_set chk upd_other (by simp [AVal.sat, upd]) hg
  | @copy l m _ _ =>
    simp only [tStmt]
    cases hm : getAv a.av m with
    | none => exact gamma_kill chk upd_other hg
    | some v =>
      have hsat := hg.2.2 _ (getAv_mem hm)
      refine gamma_set chk upd_other ?_ hg
      cases v with
      | const n => simpa [AVal.sat, upd] using hsat
      | cond hs ps => simpa [AVal.sat, upd] using hsat

theorem tStmts_sound {ss : List Stmt} {a : AState} {w0 w w' : W} {env env' : Env}
    (hg : Gamma chk a w0 w env) (hs : SSteps chk ss w env w' env') :
    Gamma chk (tStmts ss a) w0 w' env' := by
  induction hs generalizing a with
  | nil => exact hg
  | cons h1 _ ih => exact ih (tStmt_sound chk hg h1)

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
    simp only [tTerm]
    cases hav : getAv a.av l with
    | none =>
      refine ⟨a, ?_, hg⟩
      simp only [List.mem_append, List.mem_map]
      exact Or.inl ⟨_, hmem, rfl⟩
    | some v =>
      have hsat := hg.2.2 _ (getAv_mem hav)
      cases v with
      | const n =>
        simp only [AVal.sat] at hsat
        rw [hsat] at hl
        simp only [hl]
        exact ⟨a, by simp, hg⟩
      | cond hs ps =>
        simp only [AVal.sat] at hsat
        by_cases h0 : env l = 0
        · obtain ⟨h1, h2⟩ := hsat h0
          refine ⟨{ a with H := hs ++ a.H, Pr := ps ++ a.Pr }, ?_, ?_⟩
          · simp only [List.mem_append, List.mem_map]
            refine Or.inl ⟨_, hmem, ?_⟩
            simp [h0]
          · obtain ⟨hH, hP, hA⟩ := hg
            refine ⟨fun k hk => ?_, fun k hk hw0 => ?_, hA⟩
            · simp only [List.mem_append] at hk
              rcases hk with hk | hk
              · exact h1 k hk
              · exact hH k hk
            · simp only [List.mem_append] at hk
              rcases hk with hk | hk
              · exact h2 k hk hw0
              · exact hP k hk hw0
        · refine ⟨a, ?_, hg⟩
          simp only [List.mem_append, List.mem_map]
          refine Or.inl ⟨_, hmem, ?_⟩
          simp [h0]
  | @switchOther l cases _ _ _ hl =>
    simp only [tTerm]
    cases hav : getAv a.av l with
    | none =>
      refine ⟨a, ?_, hg⟩
      simp only [List.mem_append]
      exact Or.inr (mem_optEdge rfl)
    | some v =>
      have hsat := hg.2.2 _ (getAv_mem hav)
      cases v with
      | const n =>
        simp only [AVal.sat] at hsat
        rw [hsat] at hl
        simp only [hl]
        exact ⟨a, mem_optEdge rfl, hg⟩
      | cond hs ps =>
        refine ⟨a, ?_, hg⟩
        simp only [List.mem_append]
        exact Or.inr (mem_optEdge rfl)
  | @call dst b same pres est uncond _ _ _ _ hag hsame hpres hest =>
    have hc := gamma_clobber chk hsame hg
    have hk := gamma_kill chk hag hc
    obtain ⟨hH, hP, hA⟩ := hk
    have hHS : (uncond = true ∨ env' dst = 0) →
        ∀ k, k ∈ est ++ pres.filter (fun k => decide (k ∈ a.H)) → chk k w' = true := by
      intro hc0 k hk
      simp only [List.mem_append, List.mem_filter, decide_eq_true_eq] at hk
      rcases hk with hk | hk
      · exact hest k hk hc0
      · exact hpres k hk.1 hc0 (hg.1 k hk.2)
    have hPS : (uncond = true ∨ env' dst = 0) →
        ∀ k, k ∈ pres.filter (fun k => decide (k ∈ a.Pr)) → chk k w0 = true → chk k w' = true := by
      intro hc0 k hk hw0
      simp only [List.mem_filter, decide_eq_true_eq] at hk
      exact hpres k hk.1 hc0 (hg.2.1 k hk.2 hw0)
    cases uncond with
    | true =>
      refine ⟨_, List.mem_singleton.mpr rfl, ?_, ?_, hA⟩
      · intro k hk
        rcases List.mem_append.mp hk with hk | hk
        · exact hHS (Or.inl rfl) k hk
        · exact hH k hk
      · intro k hk hw0
        rcases List.mem_append.mp hk with hk | hk
        · exact hPS (Or.inl rfl) k hk hw0
        · exact hP k hk hw0
    | false =>
      refine ⟨_, List.mem_singleton.mpr rfl, hH, hP, ?_⟩
      intro p hp
      rcases List.mem_cons.mp hp with hp | hp
      · subst hp
        intro h0
        exact ⟨hHS (Or.inr h0), hPS (Or.inr h0)⟩
      · exact hA p hp
  | @branch dst src b _ _ =>
    simp only [tTerm]
    cases hav : getAv a.av src with
    | none => exact ⟨_, List.mem_singleton.mpr rfl, gamma_kill chk upd_other hg⟩
    | some v =>
      have hsat := hg.2.2 _ (getAv_mem hav)
      cases v with
      | const n =>
        refine ⟨_, List.mem_singleton.mpr rfl, gamma_set chk upd_other ?_ hg⟩
        simp only [AVal.sat] at hsat ⊢
        simp [upd, hsat]
      | cond hs ps =>
        refine ⟨_, List.mem_singleton.mpr rfl, gamma_set chk upd_other ?_ hg⟩
        simp only [AVal.sat] at hsat ⊢
        intro h0
        apply hsat
        simp only [upd_self] at h0
        by_cases hsrc : env src = 0
        · exact hsrc
        · simp [hsrc] at h0
  | residual =>
    simp only [tTerm]
    exact ⟨_, List.mem_singleton.mpr rfl,
      gamma_set chk upd_other (by simp [AVal.sat, upd]) hg⟩

theorem avLe_sound {a : AState} {p : Local × AVal} {w0 w : W} {env : Env}
    (hle : avLe a p = true) (hg : Gamma chk a w0 w env) : p.2.sat chk w0 w env p.1 := by
  obtain ⟨hH, hP, hA⟩ := hg
  obtain ⟨l, v⟩ := p
  simp only [avLe, Bool.or_eq_true, decide_eq_true_eq] at hle
  rcases hle with hmem | hle
  · exact hA _ hmem
  · cases v with
    | const n => simp at hle
    | cond hs ps =>
      simp only at hle
      cases hav : getAv a.av l with
      | none => simp [hav] at hle
      | some v' =>
        have hsat := hA _ (getAv_mem hav)
        cases v' with
        | const n =>
          simp only [hav, Bool.or_eq_true, Bool.and_eq_true, List.all_eq_true,
            decide_eq_true_eq] at hle
          simp only [AVal.sat] at hsat ⊢
          intro h0
          rcases hle with hn | ⟨h1, h2⟩
          · exact absurd (hsat ▸ h0) hn
          · refine ⟨fun k hk => hH k (h1 k hk), fun k hk hw0 => ?_⟩
            rcases h2 k hk with h | h
            · exact hP k h hw0
            · exact hH k h
        | cond hs' ps' =>
          simp only [hav, Bool.or_eq_true, Bool.and_eq_true, List.all_eq_true,
            decide_eq_true_eq] at hle
          simp only [AVal.sat] at hsat ⊢
          intro h0
          obtain ⟨c1, c2⟩ := hsat h0
          obtain ⟨h1, h2⟩ := hle
          refine ⟨fun k hk => ?_, fun k hk hw0 => ?_⟩
          · rcases h1 k hk with h | h
            · exact c1 k h
            · exact hH k h
          · rcases h2 k hk with ((h | h) | h) | h
            · exact c2 k h hw0
            · exact c1 k h
            · exact hP k h hw0
            · exact hH k h

theorem le_sound {a b : AState} {w0 w : W} {env : Env} (hle : le a b = true)
    (hg : Gamma chk a w0 w env) : Gamma chk b w0 w env := by
  simp only [le, Bool.and_eq_true, List.all_eq_true, decide_eq_true_eq, Bool.or_eq_true] at hle
  obtain ⟨⟨h1, h2⟩, h3⟩ := hle
  refine ⟨fun k hk => hg.1 k (h1 k hk), fun k hk hw0 => ?_, fun p hp => avLe_sound chk (h3 p hp) hg⟩
  rcases h2 k hk with h | h
  · exact hg.2.1 k h hw0
  · exact hg.1 k h

theorem blockOk_of_check {g : Goal} {cfg : Cfg} {inv : List (Option AState)}
    (h : checkCert g cfg inv = true) {b : Block} {bd : BlockD} (hb : cfg[b]? = some bd) :
    blockOk g cfg inv b = true := by
  simp only [checkCert, Bool.and_eq_true, List.all_eq_true, List.mem_range] at h
  have hlt : b < cfg.length := by
    rcases Nat.lt_or_ge b cfg.length with hlt | hge
    · exact hlt
    · rw [List.getElem?_eq_none hge] at hb; cases hb
  exact h.2 b hlt

/-- The certificate describes every reachable state. -/
theorem reach_sound {g : Goal} {cfg : Cfg} {inv : List (Option AState)}
    (h : checkCert g cfg inv = true) {w0 w : W} {b : Block} {env : Env}
    (hr : Reach chk g.exempt cfg w0 b w env) :
    ∃ a, inv[b]? = some (some a) ∧ Gamma chk a w0 w env := by
  induction hr with
  | entry env =>
    have h0 : edgeOk inv (0, initState g) = true := by
      simp only [checkCert, Bool.and_eq_true] at h; exact h.1
    simp only [edgeOk] at h0
    cases hi : inv[0]? with
    | none => simp [hi] at h0
    | some o =>
      cases o with
      | none => simp [hi] at h0
      | some c =>
        simp only [hi] at h0
        refine ⟨c, rfl, le_sound chk h0 ?_⟩
        refine ⟨?_, ?_, ?_⟩
        · intro k hk; simp [initState] at hk
        · intro k _ hw0; exact hw0
        · intro p hp; simp [initState] at hp
  | @step b bd w env w1 env1 b' w2 env2 _ hne hb hss hts ih =>
    obtain ⟨a, hia, hga⟩ := ih
    have hbo := blockOk_of_check h hb
    simp only [blockOk, hb, hia, Bool.or_eq_true, decide_eq_true_eq, hne, false_or,
      Bool.and_eq_true, List.all_eq_true] at hbo
    have hg1 := tStmts_sound chk hga hss
    obtain ⟨a', hmem, hg'⟩ := tTerm_sound chk hg1 hts
    have he := hbo.1 _ hmem
    simp only [edgeOk] at he
    cases hi : inv[b']? with
    | none => simp [hi] at he
    | some o =>
      cases o with
      | none => simp [hi] at he
      | some c =>
        simp only [hi] at he
        exact ⟨c, rfl, le_sound chk he hg'⟩

/-- **Soundness of the checker.** If the certificate checks, then every return of the abstract
program that did not pass through an exempt block, with an `Ok` value when the goal is about `Ok`
only, leaves a world in which every required check holds and every check required to be
preserved holds if it held on entry. -/
theorem checkCert_sound {g : Goal} {cfg : Cfg} {inv : List (Option AState)}
    (h : checkCert g cfg inv = true) {w0 w1 : W} {b : Block} {env1 : Env}
    (hret : Returns chk g.exempt cfg w0 b w1 env1) (hok : g.okOnly = true → env1 0 = 0) :
    (∀ k, k ∈ g.req → chk k w1 = true) ∧
    (∀ k, k ∈ g.reqP → chk k w0 = true → chk k w1 = true) := by
  obtain ⟨bd, w, env, hr, hne, hb, hss, hterm⟩ := hret
  obtain ⟨a, hia, hga⟩ := reach_sound chk h hr
  have hbo := blockOk_of_check h hb
  simp only [blockOk, hb, hia, hterm, Bool.or_eq_true, decide_eq_true_eq, hne, false_or,
    Bool.and_eq_true] at hbo
  have hg1 := tStmts_sound chk hga hss
  have hro := hbo.2
  simp only [retOk, Bool.or_eq_true, Bool.and_eq_true, List.all_eq_true, decide_eq_true_eq] at hro
  obtain ⟨hH, hP, hA⟩ := hg1
  rcases hro with (hro | hro) | hro
  · exfalso
    obtain ⟨hk, he⟩ := hro
    have h0 := hok hk
    simp only [isErr] at he
    cases hav : getAv (tStmts bd.stmts a).av 0 with
    | none => simp [hav] at he
    | some v =>
      cases v with
      | cond hs ps => simp [hav] at he
      | const n =>
        simp only [hav, decide_eq_true_eq] at he
        have hsat := hA _ (getAv_mem hav)
        simp only [AVal.sat] at hsat
        exact he (hsat ▸ h0)
  · obtain ⟨hk, hc⟩ := hro
    have h0 := hok hk
    simp only [condOk] at hc
    cases hav : getAv (tStmts bd.stmts a).av 0 with
    | none => simp [hav] at hc
    | some v =>
      cases v with
      | const n => simp [hav] at hc
      | cond hs ps =>
        simp only [hav, Bool.and_eq_true, List.all_eq_true, Bool.or_eq_true,
          decide_eq_true_eq] at hc
        have hsat := hA _ (getAv_mem hav)
        simp only [AVal.sat] at hsat
        obtain ⟨c1, c2⟩ := hsat h0
        refine ⟨fun k hk' => ?_, fun k hk' hw0 => ?_⟩
        · rcases hc.1 k hk' with h' | h'
          · exact c1 k h'
          · exact hH k h'
        · rcases hc.2 k hk' with ((h' | h') | h') | h'
          · exact c2 k h' hw0
          · exact c1 k h'
          · exact hP k h' hw0
          · exact hH k h'
  · refine ⟨fun k hk => hH k (hro.1 k hk), fun k hk hw0 => ?_⟩
    rcases hro.2 k hk with h' | h'
    · exact hP k h' hw0
    · exact hH k h'

/-- **Composition.** A certified callee justifies the `call` instruction that stands for it in a
caller. If the callee's certificate checks for the goal `⟨est, pres, [], okOnly⟩`, then any actual
return of the callee from world `w` to world `w'` with return discriminant `d` is a step of
`.call dst b same pres est (!okOnly)` in the caller, provided the frame part `same` holds for that
call (that part comes from the types of the arguments, not from the certificate).

This is why a summary derived by the tool for a callee may be used in a caller's graph: the
`call` instruction allows at least every behaviour the callee's own graph has. The check numbers
of caller and callee must denote the same predicates; the extractor only applies a summary when
the callee's market and account arguments are the caller's own. -/
theorem call_of_cert {est pres : List Nat} {okOnly : Bool} {cfgC : Cfg} {invC : List (Option AState)}
    (hc : checkCert ⟨est, pres, [], okOnly⟩ cfgC invC = true)
    {w w' : W} {bC : Block} {envC : Env}
    (hret : Returns chk [] cfgC w bC w' envC)
    {same : List Nat} (hsame : ∀ k, k ∈ same → chk k w' = chk k w)
    (env : Env) (dst : Local) (b : Block) :
    TStep chk (.call dst b same pres est (!okOnly)) w env b w' (upd env dst (envC 0)) := by
  refine TStep.call upd_other hsame ?_ ?_
  · intro k hk hcond hw
    have hok : okOnly = true → envC 0 = 0 := by
      intro ho
      rcases hcond with h | h
      · simp [ho] at h
      · simpa [upd] using h
    exact (checkCert_sound chk hc hret hok).2 k hk hw
  · intro k hk hcond
    have hok : okOnly = true → envC 0 = 0 := by
      intro ho
      rcases hcond with h | h
      · simp [ho] at h
      · simpa [upd] using h
    exact (checkCert_sound chk hc hret hok).1 k hk

end Sound

end MirFlow

#print axioms MirFlow.checkCert_sound
#print axioms MirFlow.call_of_cert
