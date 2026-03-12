import { type NextRequest } from "next/server";
import { proxyToApi } from "@/lib/api-proxy";

export const dynamic = "force-dynamic";

/**
 * GET /api/funding/[slab]
 *
 * Proxies to percolator-api GET /funding/:slab
 * Removed standalone Supabase impl (GH#1066 — arch cleanup).
 */
export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ slab: string }> }
) {
  const { slab } = await params;
  return proxyToApi(req, `/funding/${slab}`);
}
