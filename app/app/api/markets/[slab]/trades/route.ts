import { type NextRequest } from "next/server";
import { proxyToApi } from "@/lib/api-proxy";
import { validateSlabParam } from "@/lib/route-validators";

export const dynamic = "force-dynamic";

/**
 * GET /api/markets/[slab]/trades
 *
 * Proxies to percolator-api GET /markets/:slab/trades
 * Removed standalone Supabase impl (GH#1066 — arch cleanup).
 * 
 * MEDIUM-003: Added slab parameter validation.
 */
export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ slab: string }> }
) {
  const { slab } = await params;

  // Validate slab parameter format
  const validation = validateSlabParam(slab);
  if (!validation.valid) {
    return validation.response;
  }
  const validSlab = validation.slab;

  return proxyToApi(req, `/markets/${validSlab}/trades`);
}
