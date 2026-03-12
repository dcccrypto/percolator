/**
 * api-proxy.ts
 *
 * Shared utility for proxying Next.js API routes to percolator-api (Railway).
 * Eliminates duplicate Supabase query logic in percolator-launch.
 *
 * Usage:
 *   return proxyToApi(req, "/markets");
 *   return proxyToApi(req, `/markets/${slab}/trades`);
 */

import { type NextRequest, NextResponse } from "next/server";
import { getBackendUrl } from "@/lib/config";

const PROXY_TIMEOUT_MS = 8_000;

/**
 * Proxy a Next.js route handler request to the percolator-api backend.
 *
 * @param req          The incoming NextRequest (query params are forwarded automatically).
 * @param apiPath      The backend path to call (e.g. "/markets", "/funding/global").
 * @param extraHeaders Optional headers to forward to the backend.
 */
export async function proxyToApi(
  req: NextRequest,
  apiPath: string,
  extraHeaders?: Record<string, string>
): Promise<NextResponse> {
  let backendUrl: string;
  try {
    backendUrl = getBackendUrl();
  } catch {
    return NextResponse.json(
      { error: "Backend URL not configured" },
      { status: 502 }
    );
  }

  // Forward query string from original request
  const searchParams = req.nextUrl.searchParams.toString();
  const targetUrl = searchParams
    ? `${backendUrl}${apiPath}?${searchParams}`
    : `${backendUrl}${apiPath}`;

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), PROXY_TIMEOUT_MS);

  try {
    const upstream = await fetch(targetUrl, {
      method: req.method,
      headers: {
        "Content-Type": "application/json",
        ...extraHeaders,
      },
      signal: controller.signal,
    });

    const body = await upstream.text();

    return new NextResponse(body, {
      status: upstream.status,
      headers: {
        "Content-Type": upstream.headers.get("Content-Type") ?? "application/json",
        "Cache-Control": "no-store, max-age=0",
      },
    });
  } catch (err) {
    if ((err as Error).name === "AbortError") {
      return NextResponse.json({ error: "Backend timeout" }, { status: 504 });
    }
    return NextResponse.json({ error: "Backend unavailable" }, { status: 502 });
  } finally {
    clearTimeout(timer);
  }
}
