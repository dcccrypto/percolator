"use client";

import { useState, useEffect, useCallback } from "react";

// ─── Style tokens ──────────────────────────────────────────────────────────────
const card = "rounded-none bg-[var(--panel-bg)] border border-[var(--border)]";
const labelStyle =
  "text-[10px] font-bold uppercase tracking-[0.15em] text-[var(--text-muted)]";

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-3 mb-3">
      <div className={labelStyle}>{children}</div>
      <div className="flex-1 h-px bg-[var(--border)]" />
    </div>
  );
}

interface StaleMarket {
  slab_address: string;
  symbol: string | null;
  oracle_authority: string | null;
  mark_price: number | null;
  total_accounts: number | null;
  open_interest_long: number | null;
  open_interest_short: number | null;
  last_updated_at: string | null;
}

function truncatePk(pk: string, chars = 6) {
  if (!pk || pk.length <= chars * 2) return pk;
  return `${pk.slice(0, chars)}…${pk.slice(-4)}`;
}

function OracleStaleRow({ market }: { market: StaleMarket }) {
  const totalOI =
    (market.open_interest_long ?? 0) + (market.open_interest_short ?? 0);
  const hasOI = totalOI > 0;
  const users = market.total_accounts ?? 0;

  return (
    <div className="grid grid-cols-[1fr_auto_auto_auto] gap-3 items-center px-4 py-3 border-b border-[var(--border)] last:border-0">
      {/* Market identity */}
      <div className="min-w-0">
        <div className="flex items-center gap-2 mb-0.5">
          <span
            className="inline-block w-[6px] h-[6px] rounded-full shrink-0"
            style={{ backgroundColor: "var(--short)" }}
          />
          <span className="text-[12px] font-mono text-white font-medium">
            {market.symbol && market.symbol.length <= 10
              ? `${market.symbol}`
              : truncatePk(market.slab_address)}
          </span>
          <span
            className="text-[9px] font-bold uppercase px-1.5 py-0.5 border"
            style={{ borderColor: "var(--short)", color: "var(--short)" }}
          >
            NO PRICE
          </span>
          {hasOI && (
            <span
              className="text-[9px] font-bold uppercase px-1.5 py-0.5 border"
              style={{ borderColor: "var(--warning)", color: "var(--warning)" }}
            >
              USERS STUCK
            </span>
          )}
        </div>
        <div className="text-[10px] text-[var(--text-dim)] font-mono">
          {truncatePk(market.slab_address, 8)}
        </div>
        {market.oracle_authority && (
          <div className="text-[10px] text-[var(--text-dim)] flex items-center gap-1">
            <span className="text-[var(--text-dim)]">authority:</span>
            <span className="font-mono text-[var(--cyan)]">
              {truncatePk(market.oracle_authority)}
            </span>
          </div>
        )}
      </div>

      {/* Users */}
      <div className="text-center">
        <div
          className="text-[16px] font-bold tabular-nums"
          style={{
            fontFamily: "var(--font-jetbrains-mono)",
            color: users > 0 ? "var(--warning)" : "var(--text-dim)",
          }}
        >
          {users}
        </div>
        <div className="text-[9px] uppercase tracking-wider text-[var(--text-dim)]">
          users
        </div>
      </div>

      {/* OI (raw) */}
      <div className="text-center">
        <div
          className="text-[12px] font-mono tabular-nums"
          style={{ color: hasOI ? "var(--text-secondary)" : "var(--text-dim)" }}
        >
          {hasOI
            ? (totalOI / 1e9).toLocaleString(undefined, {
                maximumFractionDigits: 0,
              })
            : "—"}
        </div>
        <div className="text-[9px] uppercase tracking-wider text-[var(--text-dim)]">
          OI (e9)
        </div>
      </div>

      {/* Copy slab */}
      <div>
        <button
          onClick={() =>
            navigator.clipboard.writeText(market.slab_address).catch(() => {})
          }
          className="text-[9px] uppercase tracking-wider px-2 py-1 border border-[var(--border)] text-[var(--text-dim)] hover:border-[var(--accent)] hover:text-[var(--accent)] transition-colors font-mono"
          title="Copy slab address"
        >
          copy
        </button>
      </div>
    </div>
  );
}

/**
 * OracleFreshnessSection
 *
 * Fetches all admin-oracle markets from the public /api/markets endpoint and
 * shows those with mark_price = null, sorted by user count desc.
 * This gives admins an at-a-glance view of which markets need an oracle price push.
 */
export function OracleFreshnessSection() {
  const [staleMarkets, setStaleMarkets] = useState<StaleMarket[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastFetched, setLastFetched] = useState<Date | null>(null);

  const fetchStaleMarkets = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/markets");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: unknown = await res.json();
      const raw: unknown[] = Array.isArray(data)
        ? data
        : Array.isArray((data as { markets?: unknown[] }).markets)
        ? (data as { markets: unknown[] }).markets
        : [];

      // Filter: has oracle_authority (admin-mode) AND no mark_price
      const stale = (raw as StaleMarket[])
        .filter(
          (m) =>
            m.oracle_authority &&
            m.oracle_authority !== "" &&
            (m.mark_price === null || m.mark_price === undefined || m.mark_price === 0)
        )
        .sort(
          (a, b) => (b.total_accounts ?? 0) - (a.total_accounts ?? 0)
        );

      setStaleMarkets(stale);
      setLastFetched(new Date());
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchStaleMarkets();
  }, [fetchStaleMarkets]);

  const withUsers = staleMarkets.filter((m) => (m.total_accounts ?? 0) > 0);
  const withoutUsers = staleMarkets.filter((m) => (m.total_accounts ?? 0) === 0);

  return (
    <div className="mb-8">
      <SectionHeader>Oracle Freshness Check</SectionHeader>

      {/* Summary banner */}
      <div className={`${card} p-4 mb-4`}>
        <div className="flex flex-wrap gap-6 items-center justify-between">
          <div className="flex gap-6">
            <div>
              <div className="text-[24px] font-bold tabular-nums" style={{ fontFamily: "var(--font-jetbrains-mono)", color: staleMarkets.length > 0 ? "var(--short)" : "var(--long)" }}>
                {loading ? "…" : staleMarkets.length}
              </div>
              <div className="text-[10px] uppercase tracking-[0.15em] text-[var(--text-muted)]">
                admin markets w/o price
              </div>
            </div>
            <div>
              <div className="text-[24px] font-bold tabular-nums" style={{ fontFamily: "var(--font-jetbrains-mono)", color: withUsers.length > 0 ? "var(--warning)" : "var(--long)" }}>
                {loading ? "…" : withUsers.length}
              </div>
              <div className="text-[10px] uppercase tracking-[0.15em] text-[var(--text-muted)]">
                with trapped users
              </div>
            </div>
          </div>

          <div className="flex items-center gap-3">
            {lastFetched && (
              <span className="text-[10px] text-[var(--text-dim)] font-mono">
                fetched {lastFetched.toLocaleTimeString()}
              </span>
            )}
            <button
              onClick={fetchStaleMarkets}
              disabled={loading}
              className="text-[10px] uppercase tracking-[0.15em] px-3 py-1.5 border border-[var(--border)] text-[var(--text-dim)] hover:border-[var(--accent)] hover:text-[var(--accent)] transition-colors disabled:opacity-40"
            >
              {loading ? "refreshing…" : "refresh"}
            </button>
          </div>
        </div>

        {error && (
          <div className="mt-3 text-[11px] text-[var(--short)]">
            ✗ Failed to load: {error}
          </div>
        )}

        {!loading && !error && staleMarkets.length === 0 && (
          <div className="mt-3 text-[11px] text-[var(--long)]">
            ✓ All admin-oracle markets have a price pushed. No action needed.
          </div>
        )}
      </div>

      {/* Markets with trapped users — highest priority */}
      {withUsers.length > 0 && (
        <div className={`${card} overflow-hidden mb-4`}>
          <div className="border-b border-[var(--border)] px-4 py-3 flex items-center gap-2">
            <span
              className="inline-block w-[6px] h-[6px] rounded-full"
              style={{ backgroundColor: "var(--warning)" }}
            />
            <span className={labelStyle}>
              {withUsers.length} market{withUsers.length !== 1 ? "s" : ""} with users — action required
            </span>
          </div>
          <div>
            {withUsers.map((m) => (
              <OracleStaleRow key={m.slab_address} market={m} />
            ))}
          </div>
          <div className="border-t border-[var(--border)] px-4 py-3 bg-[rgba(255,183,0,0.03)]">
            <p className="text-[10px] text-[var(--text-dim)]">
              These markets have depositors with open interest but no oracle price. New position
              opens are blocked. The oracle authority for each market must call{" "}
              <span className="font-mono text-[var(--cyan)]">PushOraclePrice</span> on-chain.
              Use the Oracle Authority section above to delegate push rights to the keeper wallet.
            </p>
          </div>
        </div>
      )}

      {/* Markets without users — lower priority */}
      {withoutUsers.length > 0 && (
        <div className={`${card} overflow-hidden`}>
          <div className="border-b border-[var(--border)] px-4 py-3">
            <span className={labelStyle}>
              {withoutUsers.length} uninitialised market{withoutUsers.length !== 1 ? "s" : ""} (no users)
            </span>
          </div>
          <div className="max-h-[300px] overflow-y-auto">
            {withoutUsers.map((m) => (
              <OracleStaleRow key={m.slab_address} market={m} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
