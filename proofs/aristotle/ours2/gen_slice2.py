#!/usr/bin/env python3
"""gen_slice2.py [--check]

Build project/src/engine_slice.rs and project/src/wide_math.rs from the engine at the pinned commit
(read with `git show c38b6b1e:src/v16.rs`, pinned by git blob id). Every engine block is copied byte
for byte from the line range given in BLOCKS; each range is pinned by the text of its first and
last line. Text that is not the engine's is emitted between the marker lines
`// >>> NOT ENGINE` and `// <<< NOT ENGINE` (stubs, wrapper lines, `impl` lines, derive lines).
With --check, write nothing and exit 1 if the files in project/ differ from what would be generated.
"""
import hashlib, pathlib, subprocess, sys

HERE = pathlib.Path(__file__).resolve().parent
PIN = "c38b6b1e"
V16_BLOB = "7d3edfe86be1a80f49793cf551c39687a8766a4d"      # git rev-parse c38b6b1e:src/v16.rs
WIDE_BLOB = "97c6b16d4079aa5c83a9dd79c528e25db1027886"     # git rev-parse c38b6b1e:src/wide_math.rs

NE_OPEN = "// >>> NOT ENGINE"
NE_CLOSE = "// <<< NOT ENGINE"

# The engine derives bytemuck::Zeroable and bytemuck::Pod on its byte-wrapper structs. The slice has
# no dependencies, so the two attribute lines above each such struct are replaced by these two.
POD_ATTR = ["#[repr(C)]", "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]"]
POD_NOTE = ("engine has `#[repr(C)]` and `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, "
            "bytemuck::Zeroable, bytemuck::Pod)]` here; the bytemuck derives are dropped (no dependency)")

IMPL_CORE = "impl V16Core {"
IMPL_PV = "impl<'a> PortfolioV16View<'a> {"
IMPL_MG = "impl<'a> MarketGroupV16ViewMut<'a> {"
IMPL_TV = "impl TokenValueFlowProofV16 {"
MG_NOTE = "engine: `impl<'a, T> MarketGroupV16ViewMut<'a, T> {` (src/v16.rs:10377); the slice's view has no `markets`, so no `T`"

TAIL_OPEN = [
    "    /// WRAPPER, not engine text: these four lines and the closing brace. The body is the tail of",
    "    /// `close_resolved_account_not_atomic` (src/v16.rs:26580), from `let mut payout_receipt = None;`",
    "    /// to its last line. The engine's steps before it (src/v16.rs:26585-26642) are NOT here.",
    "    pub fn close_resolved_account_tail_not_atomic(",
    "        &mut self,",
    "        account: &mut PortfolioV16ViewMut<'_>,",
    "    ) -> V16Result<ResolvedCloseOutcomeV16> {",
]

# ("ne", [lines])                                            text that is not the engine's
# ("blk", first, last, head, tail, pre, post, note)          engine lines first..last; `head` must be
#     contained in line `first`, `tail` must equal line `last`; pre/post are non-engine lines
#     emitted before/after (wrapped in the NOT ENGINE markers together with the note)
def blk(first, last, head, tail, pre=None, post=None, note=None):
    return ("blk", first, last, head, tail, pre or [], post or [], note)

def in_impl(impl, note=None):
    return dict(pre=[impl], post=["}"], note=note)

STUB_TYPES = """\
// The receivers of the engine methods below. In the engine `MarketGroupV16ViewMut<'a, T>` has
// `header: &'a mut MarketGroupV16HeaderAccount` and `markets: &'a mut [Market<T>]`
// (src/v16.rs:6231-6234); here it has only `header`, and the header struct has only the fields the
// copied code reads or writes, each with the engine's field type and in the engine's field order
// (src/v16.rs:9447-9494). `PortfolioAccountV16Account` likewise (src/v16.rs:28182-28219).

/// Stands for the engine's `V16Core` (src/v16.rs:1164 `impl V16Core`).
pub struct V16Core;

/// STUB of src/v16.rs:6226-6229 (no `markets`, no `T`).
pub struct MarketGroupV16View<'a> {
    pub header: &'a MarketGroupV16HeaderAccount,
}

/// STUB of src/v16.rs:6231-6234 (no `markets`, no `T`).
pub struct MarketGroupV16ViewMut<'a> {
    pub header: &'a mut MarketGroupV16HeaderAccount,
}

/// STUB of src/v16.rs:9447-9494: 21 of the engine's 47 fields, same names, same types, same order.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarketGroupV16HeaderAccount {
    pub vault: V16PodU128,
    pub insurance: V16PodU128,
    pub c_tot: V16PodU128,
    pub pnl_pos_tot: V16PodU128,
    pub pnl_pos_bound_tot_num: V16PodU128,
    pub pnl_pos_bound_tot: V16PodU128,
    pub pnl_matured_pos_tot: V16PodU128,
    pub backing_provider_earnings_total: V16PodU128,
    pub source_fresh_backing_total_num: V16PodU128,
    pub resolved_payout_blocker_count: V16PodU64,
    pub stale_certificate_count: V16PodU64,
    pub b_stale_account_count: V16PodU64,
    pub negative_pnl_account_count: V16PodU64,
    pub risk_epoch: V16PodU64,
    pub current_slot: V16PodU64,
    pub mode: u8,
    pub resolved_slot: V16PodU64,
    pub payout_snapshot: V16PodU128,
    pub payout_snapshot_pnl_pos_tot: V16PodU128,
    pub payout_snapshot_captured: u8,
    pub resolved_payout_ledger: ResolvedPayoutLedgerV16Account,
}

/// STUB of src/v16.rs:27923-27935: only `valid`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HealthCertV16Account {
    pub valid: u8,
}

/// STUB of src/v16.rs:28182-28219: 6 of the engine's fields, same names, same types, same order.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortfolioAccountV16Account {
    pub capital: V16PodU128,
    pub pnl: V16PodI128,
    pub reserved_pnl: V16PodU128,
    pub fee_credits: V16PodI128,
    pub health_cert: HealthCertV16Account,
    pub resolved_payout_receipt: ResolvedPayoutReceiptV16Account,
}

/// STUB of src/v16.rs:28126-28145: only the field the copied code names. Only DEAD code uses it.
pub struct PortfolioSourceDomainV16Account {
    pub source_claim_bound_num: V16PodU128,
}

/// STUB of src/v16.rs:6020-6033: only the two fields the copied code names. Only DEAD code uses it.
#[derive(Clone, Copy)]
pub struct SourceCreditStateV16 {
    pub positive_claim_bound_num: u128,
    pub exact_positive_claim_num: u128,
}"""

STUB_FNS = """\
// ---------------------------------------------------------------------------------------------
// STUBS. Four kinds; aristotle/resolved_close_spec.md gives the reasons.
//
// STUB-OK: the engine function runs checks and writes nothing; here it returns `Ok(())`. A stub of
//   this kind can only turn an engine `Err` into `Ok`. So a statement about runs of the slice that
//   return `Ok` covers every engine run that returns `Ok`; the engine may additionally return `Err`
//   there.
// STUB-FRAME (`sync_unattributed_loss_lock`): the engine function writes two account bytes that are
//   NOT in this slice (`liquidation_lock`, `loss_provenance`) and may return `Err`; here it returns
//   `Ok(())` and writes nothing. Like STUB-OK it can only turn an engine `Err` into `Ok`.
// STUB-PATH (`burn_account_source_claim_bound_num`): the engine function returns `Ok(())` having
//   written nothing when `burn_num == 0` or when the account's source-claim bound sum is 0
//   (src/v16.rs:12894-12900); the stub is that path. On the engine's other path (an account that
//   reaches the tail with positive pnl AND a nonzero source-claim bound) the engine runs code that
//   is not in this slice and that can write fields of this slice. That path is NOT covered:
//   every statement about the slice transfers to the engine only for accounts whose source-claim
//   bound sum is 0 when `set_account_pnl` runs, or whose pnl is not positive.
// STUB-DEAD: called only inside `if let Some(domain) = increase_domain` of `set_account_pnl_inner`
//   (src/v16.rs:15382-15408). `set_account_pnl` passes `source_domain = None` (src/v16.rs:15223)
//   and `increase_domain` is `source_domain` or `None` (src/v16.rs:15358-15367), so the slice
//   never calls them. Their bodies panic, so a proof that a caller does not panic also shows this.
// ---------------------------------------------------------------------------------------------
impl<'a> MarketGroupV16ViewMut<'a> {
    /// src/v16.rs:6290-6295 without `markets`.
    pub fn as_view(&self) -> MarketGroupV16View<'_> {
        MarketGroupV16View {
            header: self.header,
        }
    }

    /// STUB-OK of src/v16.rs:10378 (`validate_shape`, a read-only audit of the whole market group).
    pub fn validate_shape(&self) -> V16Result<()> {
        Ok(())
    }

    /// STUB-PATH of src/v16.rs:12889-12928: the engine's path for `burn_num == 0` or no source claims.
    fn burn_account_source_claim_bound_num(
        &mut self,
        _account: &mut PortfolioV16ViewMut<'_>,
        _burn_num: u128,
    ) -> V16Result<()> {
        Ok(())
    }

    /// STUB-FRAME of src/v16.rs:15318-15339 (writes `account.header.liquidation_lock` and
    /// `account.header.loss_provenance`, neither of which is in the slice).
    fn sync_unattributed_loss_lock(
        _account: &mut PortfolioV16ViewMut<'_>,
        _new_pnl: i128,
        _loss_domain: Option<usize>,
    ) -> V16Result<()> {
        Ok(())
    }

    /// STUB-DEAD.
    fn ensure_account_source_claim_market_id(
        &self,
        _account: &mut PortfolioV16ViewMut<'_>,
        _domain: usize,
    ) -> V16Result<()> {
        panic!("STUB-DEAD")
    }

    /// STUB-DEAD.
    fn source_credit_for_domain(&self, _domain: usize) -> V16Result<SourceCreditStateV16> {
        panic!("STUB-DEAD")
    }

    /// STUB-DEAD.
    fn set_source_credit_for_domain(
        &mut self,
        _domain: usize,
        _source: SourceCreditStateV16,
    ) -> V16Result<()> {
        panic!("STUB-DEAD")
    }
}

impl<'a> PortfolioV16ViewMut<'a> {
    /// STUB-OK of src/v16.rs:6865 (`validate_with_market`, a read-only audit of the account).
    pub fn validate_with_market(&self, _market: &MarketGroupV16View<'_>) -> V16Result<()> {
        Ok(())
    }

    /// STUB-DEAD.
    fn source_domain_mut_or_insert(
        &mut self,
        _domain: usize,
    ) -> V16Result<&mut PortfolioSourceDomainV16Account> {
        panic!("STUB-DEAD")
    }
}

impl V16Core {
    /// STUB-DEAD.
    fn prepare_source_credit_domain_recompute_for_epoch(
        _source: SourceCreditStateV16,
        _risk_epoch: u64,
    ) -> V16Result<(SourceCreditStateV16, u64)> {
        panic!("STUB-DEAD")
    }
}"""

MG = dict(pre=[IMPL_MG], post=["}"], note=MG_NOTE)
POD = dict(pre=POD_ATTR, note=POD_NOTE)

ITEMS = [
    ("ne", ["use crate::wide_math::wide_mul_div_floor_u128;", "use crate::BOUND_SCALE;"]),
    blk(286, 342, "#[derive(Clone, Copy, Debug, PartialEq, Eq)]", "pub type V16Result<T> = core::result::Result<T, V16Error>;"),
    blk(5051, 5056, "#[derive(Clone, Copy, Debug, PartialEq, Eq)]", "}"),
    blk(7092, 7159, "#[repr(C)]", "}"),
    blk(7161, 7179, "fn validate_resolved_payout_receipt_value(", "}"),
    blk(7181, 7206, "fn apply_resolved_payout_receipt_payment(", "}"),
    blk(8357, 8361, "#[derive(Clone, Copy, Debug, PartialEq, Eq)]", "}"),
    blk(8401, 8415, "pub struct V16PodU64 {", "}", **POD),
    blk(8419, 8433, "pub struct V16PodU128 {", "}", **POD),
    blk(8437, 8451, "pub struct V16PodI128 {", "}", **POD),
    blk(28046, 28087, "pub struct ResolvedPayoutLedgerV16Account {", "}", **POD),
    blk(28091, 28122, "pub struct ResolvedPayoutReceiptV16Account {", "}", **POD),
    blk(29226, 29240, "fn encode_bool(value: bool) -> u8 {", "}"),
    blk(29305, 29312, "fn decode_market_mode(value: u8) -> V16Result<MarketModeV16> {", "}"),
    blk(29413, 29418, "fn validate_non_min_i128(v: i128) -> V16Result<()> {", "}"),
    ("ne", STUB_TYPES.split("\n")),
    blk(6298, 6304, "pub struct PortfolioV16View<'a> {", "}"),
    # token value flow proof (the cfg(not(kani)) variants; the cfg(kani) ones differ only in `pub`)
    blk(7607, 7608, "#[cfg(not(kani))]", "const V16_TOKEN_VALUE_CLASS_COUNT: usize = 17;"),
    blk(7633, 7655, "#[repr(u8)]", "}"),
    blk(7669, 7679, "#[repr(C)]", "}"),
    blk(7682, 7691, "pub const fn empty(vault_before: u128, vault_after: u128) -> Self {", "    }", **in_impl(IMPL_TV)),
    blk(7860, 7930, "pub fn capital_and_resolved_payout_to_external_out(", "    }", **in_impl(IMPL_TV)),
    # V16Core kernels
    blk(2684, 2707, "/// Recompute the resolved payout rate", "    }", **in_impl(IMPL_CORE)),
    blk(2968, 2984, "#[inline]", "    }", **in_impl(IMPL_CORE)),
    blk(6725, 6727, "fn validate_resolved_payout_receipt_static(", "    }", **in_impl(IMPL_PV)),
    ("ne", STUB_FNS.split("\n")),
    # market-group methods
    blk(10584, 10586, "fn backing_provider_earnings_total(&self) -> u128 {", "    }", **MG),
    blk(10592, 10594, "fn source_fresh_backing_total_atoms(&self) -> u128 {", "    }", **MG),
    blk(10602, 10611, "fn residual(&self) -> u128 {", "    }", **MG),
    blk(10618, 10620, "fn junior_claim_bound(&self) -> u128 {", "    }", **MG),
    blk(15218, 15224, "fn set_account_pnl(", "    }", **MG),
    blk(15341, 15468, "fn set_account_pnl_inner(", "    }", **MG),
    blk(25158, 25167, "fn resolved_positive_payout_ready(&self) -> V16Result<bool> {", "    }", **MG),
    blk(25169, 25271, "fn recompute_resolved_payout_rate(&mut self) -> V16Result<()> {", "    }", **MG),
    blk(26129, 26134, "fn resolved_payout_rate_is_terminal(&self) -> V16Result<bool> {", "    }", **MG),
    blk(26140, 26160, "fn clear_fully_diluted_resolved_receipt_if_terminal(", "    }", **MG),
    blk(26492, 26525, "fn claim_resolved_payout_topup_core_not_atomic(", "    }", **MG),
    blk(26643, 26702, "let mut payout_receipt = None;", "        Ok(ResolvedCloseOutcomeV16::Closed { payout })",
        pre=[IMPL_MG] + TAIL_OPEN, post=["    }", "}"], note=MG_NOTE),
    ("ne", ["pub mod harness;"]),
]

HEADER = """// GENERATED by proofs/aristotle/ours2/gen_slice2.py. Do not edit: run the script.
// Engine: dcccrypto/percolator src/v16.rs at revision c38b6b1e (git blob {blob}).
// Every block between a line `// ---- src/v16.rs:A-B ----` and the next `// ---- end ----` is lines
// A..B of that file, byte for byte. Everything between `// >>> NOT ENGINE` and `// <<< NOT ENGINE`
// is not engine text. Nothing else is in this file except this header and blank lines."""

def git_blob(data: bytes) -> str:
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()

def git_show(path: str) -> bytes:
    return subprocess.run(["git", "show", "%s:%s" % (PIN, path)], cwd=HERE, check=True,
                          capture_output=True).stdout

def ne(lines, note=None):
    out = [NE_OPEN + (": " + note if note else "")]
    out.extend(lines)
    out.append(NE_CLOSE)
    return out

def build():
    v16 = git_show("src/v16.rs")
    wide = git_show("src/wide_math.rs")
    if git_blob(v16) != V16_BLOB:
        sys.exit("src/v16.rs at %s is not the pinned blob %s (got %s)" % (PIN, V16_BLOB, git_blob(v16)))
    if git_blob(wide) != WIDE_BLOB:
        sys.exit("src/wide_math.rs at %s is not the pinned blob %s" % (PIN, WIDE_BLOB))
    lines = v16.decode().split("\n")
    out = HEADER.format(blob=V16_BLOB).split("\n") + [""]
    for item in ITEMS:
        if item[0] == "ne":
            out.extend(ne(item[1])); out.append(""); continue
        _, first, last, head, tail, pre, post, note = item
        if head not in lines[first - 1]:
            sys.exit("line %d does not contain %r: %r" % (first, head, lines[first - 1]))
        if lines[last - 1] != tail:
            sys.exit("line %d is %r, expected %r" % (last, lines[last - 1], tail))
        if pre:
            out.extend(ne(pre, note))
        out.append("// ---- src/v16.rs:%d-%d ----" % (first, last))
        out.extend(lines[first - 1:last])
        out.append("// ---- end ----")
        if post:
            out.extend(ne(post))
        out.append("")
    return {"project/src/engine_slice.rs": "\n".join(out).encode(), "project/src/wide_math.rs": wide}

def main():
    files = build()
    check = "--check" in sys.argv
    bad = 0
    for rel, data in files.items():
        p = HERE / rel
        if check:
            if not p.exists() or p.read_bytes() != data:
                print("DIFFERS: %s" % rel); bad = 1
            else:
                print("identical: %s" % rel)
        else:
            p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(data)
            print("wrote %s (%d bytes)" % (rel, len(data)))
    sys.exit(bad)

main()
