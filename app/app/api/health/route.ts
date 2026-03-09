import { NextResponse } from "next/server";
import { getBackendUrl, getRpcEndpoint } from "@/lib/config";
export const dynamic = "force-dynamic";

async function checkWithTimeout(
  url: string,
  init: RequestInit,
  timeoutMs = 3000
): Promise<boolean> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetch(url, { ...init, signal: controller.signal });
    return res.ok;
  } catch {
    return false;
  } finally {
    clearTimeout(timer);
  }
}

export async function GET() {
  let API_URL: string;
  try {
    API_URL = getBackendUrl();
  } catch {
    // getBackendUrl() throws in non-production when env vars are missing.
    // Return a degraded response instead of crashing the route at import time.
    return NextResponse.json(
      { status: "offline", api: false, rpc: false, ts: Date.now(), error: "Backend URL not configured" },
      { headers: { "Cache-Control": "no-store, max-age=0" } }
    );
  }
  const RPC_URL = getRpcEndpoint();

  const [apiOk, rpcOk] = await Promise.all([
    checkWithTimeout(`${API_URL}/health`, {}, 3000),
    checkWithTimeout(
      RPC_URL,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "getHealth" }),
      },
      3000
    ),
  ]);

  const status = apiOk && rpcOk ? "online" : apiOk ? "degraded" : "offline";

  return NextResponse.json(
    { status, api: apiOk, rpc: rpcOk, ts: Date.now() },
    {
      headers: {
        "Cache-Control": "no-store, max-age=0",
      },
    }
  );
}
