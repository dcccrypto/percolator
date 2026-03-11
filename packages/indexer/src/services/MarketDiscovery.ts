import { PublicKey } from "@solana/web3.js";
import { discoverMarkets, type DiscoveredMarket } from "@percolator/sdk";
import { config, getConnection, getFallbackConnection, createLogger, captureException } from "@percolator/shared";

const logger = createLogger("indexer:market-discovery");

const INITIAL_RETRY_DELAYS = [5_000, 15_000, 30_000, 60_000]; // escalating backoff

export class MarketDiscovery {
  private markets = new Map<string, { market: DiscoveredMarket }>();
  private timer: ReturnType<typeof setInterval> | null = null;
  private consecutiveFailures = 0;
  
  async discover(): Promise<DiscoveredMarket[]> {
    const programIds = config.allProgramIds;
    const conn = getFallbackConnection();
    const all: DiscoveredMarket[] = [];
    let failedPrograms = 0;
    
    for (const id of programIds) {
      try {
        const found = await discoverMarkets(conn, new PublicKey(id));
        all.push(...found);
      } catch (e) {
        failedPrograms++;
        logger.warn("Failed to discover on program", { programId: id, error: e });
      }
      await new Promise(r => setTimeout(r, 2000));
    }
    
    // All programs failed — RPC is likely down
    if (failedPrograms === programIds.length && programIds.length > 0) {
      this.consecutiveFailures++;
      const err = new Error(`Market discovery failed for all ${programIds.length} programs (consecutive: ${this.consecutiveFailures})`);
      logger.error("All program discoveries failed — RPC may be down", {
        consecutiveFailures: this.consecutiveFailures,
        staleMarkets: this.markets.size,
      });
      captureException(err, { tags: { context: "market-discovery-total-failure" } });
      // Preserve stale markets — do NOT clear the map
      return [];
    }
    
    // Discovery returned 0 markets despite some programs succeeding
    if (all.length === 0) {
      logger.warn("Discovery succeeded but found 0 markets", {
        programCount: programIds.length,
        failedPrograms,
      });
    }
    
    // Only update the map when we actually found markets
    if (all.length > 0) {
      // Rebuild map to drop stale entries
      this.markets.clear();
      for (const market of all) {
        this.markets.set(market.slabAddress.toBase58(), { market });
      }
      this.consecutiveFailures = 0;
    }
    
    logger.info("Market discovery complete", {
      totalMarkets: all.length,
      failedPrograms,
      consecutiveFailures: this.consecutiveFailures,
    });
    return all;
  }
  
  getMarkets() {
    return this.markets;
  }
  
  async start(intervalMs = 300_000) {
    // Initial discovery with retry + backoff
    let initialSuccess = false;
    for (let attempt = 0; attempt <= INITIAL_RETRY_DELAYS.length; attempt++) {
      try {
        const markets = await this.discover();
        if (markets.length > 0) {
          initialSuccess = true;
          break;
        }
        // Got 0 markets — worth retrying
        if (attempt < INITIAL_RETRY_DELAYS.length) {
          const delay = INITIAL_RETRY_DELAYS[attempt];
          logger.warn(`Initial discovery returned 0 markets, retrying in ${delay / 1000}s`, { attempt: attempt + 1 });
          await new Promise(r => setTimeout(r, delay));
        }
      } catch (err) {
        logger.error("Initial discovery failed", { error: err, attempt: attempt + 1 });
        captureException(err, { tags: { context: "market-discovery-initial", attempt: String(attempt + 1) } });
        if (attempt < INITIAL_RETRY_DELAYS.length) {
          const delay = INITIAL_RETRY_DELAYS[attempt];
          logger.warn(`Retrying initial discovery in ${delay / 1000}s`);
          await new Promise(r => setTimeout(r, delay));
        }
      }
    }
    
    if (!initialSuccess) {
      logger.error("Initial market discovery exhausted all retries — will continue with periodic polling");
    }
    
    this.timer = setInterval(() => this.discover().catch((err) => {
      logger.error("Discovery failed", { error: err });
      captureException(err, { tags: { context: "market-discovery-periodic" } });
    }), intervalMs);
  }
  
  stop() {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }
}
