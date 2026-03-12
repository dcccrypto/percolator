/**
 * GET /api/admin/bugs
 *
 * Returns all bug_reports rows with ALL columns (including PII like ip,
 * admin_notes, bounty_wallet) using the service-role client which bypasses
 * the column-level GRANT restrictions applied to the `authenticated` role by
 * migration 034.
 *
 * Auth: Supabase session cookie — user must be authenticated AND present in
 * the admin_users table. Returns 401 if unauthenticated, 403 if not an admin.
 *
 * Why this exists:
 *   Migration 034 (PERC-security/N7) revoked unrestricted SELECT on
 *   bug_reports from the `authenticated` role, restricting it to 7 safe
 *   columns. The admin dashboard needs all columns. Calling Supabase directly
 *   from the browser client silently returns partial rows. This server-side
 *   route uses getServiceClient() (service_role key) to bypass that restriction
 *   after verifying the caller is an admin.
 */

import { NextRequest, NextResponse } from "next/server";
import { createServerClient } from "@supabase/ssr";
import { getServiceClient } from "@/lib/supabase";
import { cookies } from "next/headers";

export const dynamic = "force-dynamic";

async function getSessionUser(req: NextRequest) {
  const cookieStore = await cookies();
  const supabase = createServerClient(
    process.env.NEXT_PUBLIC_SUPABASE_URL!,
    process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY!,
    {
      cookies: {
        getAll() {
          return cookieStore.getAll();
        },
        setAll(cookiesToSet) {
          // read-only in Route Handlers — no-op
        },
      },
    },
  );

  const {
    data: { user },
    error,
  } = await supabase.auth.getUser();
  if (error || !user) return null;
  return user;
}

export async function GET(req: NextRequest) {
  // 1. Verify session
  const user = await getSessionUser(req);
  if (!user) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  // 2. Verify admin
  if (!user.email) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }
  const sb = getServiceClient();
  const { data: adminRow } = await (sb as any)
    .from("admin_users")
    .select("id")
    .eq("email", user.email)
    .maybeSingle();

  if (!adminRow) {
    return NextResponse.json({ error: "Forbidden" }, { status: 403 });
  }

  // 3. Fetch all bug_reports columns using service role (bypasses column GRANT restrictions)
  const { data, error: fetchError } = await (sb as any)
    .from("bug_reports")
    .select("*")
    .order("created_at", { ascending: false })
    .limit(500);

  if (fetchError) {
    console.error("GET /api/admin/bugs error:", fetchError);
    return NextResponse.json({ error: "Failed to fetch bug reports" }, { status: 500 });
  }

  return NextResponse.json(data ?? []);
}

export async function PATCH(req: NextRequest) {
  // 1. Verify session
  const user = await getSessionUser(req);
  if (!user) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  // 2. Verify admin
  if (!user.email) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }
  const sb = getServiceClient();
  const { data: adminRow } = await (sb as any)
    .from("admin_users")
    .select("id")
    .eq("email", user.email)
    .maybeSingle();

  if (!adminRow) {
    return NextResponse.json({ error: "Forbidden" }, { status: 403 });
  }

  // 3. Parse body
  const body = await req.json().catch(() => null);
  if (!body || !body.id) {
    return NextResponse.json({ error: "Missing bug id" }, { status: 400 });
  }

  const { id } = body;

  // Whitelist allowed fields — prevent arbitrary column writes
  const ALLOWED_FIELDS = ["status", "admin_notes"] as const;
  type AllowedField = typeof ALLOWED_FIELDS[number];
  const updates: Partial<Record<AllowedField, string>> & { updated_at?: string } = {};
  for (const field of ALLOWED_FIELDS) {
    if (field in body && body[field] !== undefined) {
      updates[field] = body[field] as string;
    }
  }
  if (Object.keys(updates).length === 0) {
    return NextResponse.json({ error: "No updatable fields provided" }, { status: 400 });
  }
  // Ensure updated_at is refreshed
  updates.updated_at = new Date().toISOString();

  const { data, error: updateError } = await (sb as any)
    .from("bug_reports")
    .update(updates)
    .eq("id", id)
    .select()
    .maybeSingle();

  if (updateError) {
    console.error("PATCH /api/admin/bugs error:", updateError);
    return NextResponse.json({ error: "Failed to update bug report" }, { status: 500 });
  }

  return NextResponse.json(data);
}
