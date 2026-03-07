import { Connection } from "@solana/web3.js";
import { getConfig, getWsEndpoint } from "./config";

/**
 * Sentinel WSS URL used when no Helius WS key is configured.
 * @solana/web3.js Connection auto-derives wss:// from the HTTP endpoint
 * using `wsEndpoint || makeWebsocketUrl(endpoint)` — any falsy value
 * (false, undefined, "") falls through. We must provide a truthy string.
 * This non-routable address fails the TCP handshake immediately without
 * triggering the aggressive reconnect loop that wss://percolatorlaunch.com
 * causes on Vercel (which can't upgrade WebSockets). (#869)
 */
const WS_DISABLED_SENTINEL = "wss://0.0.0.0";

let _connection: Connection | null = null;

export function getConnection(): Connection {
  if (!_connection) {
    _connection = new Connection(getConfig().rpcUrl, {
      commitment: "confirmed",
      wsEndpoint: getWsEndpoint() ?? WS_DISABLED_SENTINEL,
    });
  }
  return _connection;
}
