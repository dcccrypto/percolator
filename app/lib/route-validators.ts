import { PublicKey } from "@solana/web3.js";
import { NextResponse } from "next/server";

/**
 * Validates and sanitizes route parameters to prevent injection attacks
 * and ensure type safety for common API parameter types.
 */

/** Validates a Solana public key address format */
export function validatePublicKeyParam(value: string | null | undefined): { valid: false; response: NextResponse } | { valid: true; publicKey: PublicKey } {
  if (!value) {
    return {
      valid: false,
      response: NextResponse.json({ error: "Invalid address" }, { status: 400 }),
    };
  }

  try {
    const publicKey = new PublicKey(value);
    return { valid: true, publicKey };
  } catch {
    return {
      valid: false,
      response: NextResponse.json({ error: "Invalid address format" }, { status: 400 }),
    };
  }
}

/** Validates a slab (market) address parameter */
export function validateSlabParam(value: string | null | undefined): { valid: false; response: NextResponse } | { valid: true; slab: string } {
  if (!value || typeof value !== "string") {
    return {
      valid: false,
      response: NextResponse.json({ error: "Invalid slab address" }, { status: 400 }),
    };
  }

  // Attempt to validate as PublicKey to ensure correct format
  try {
    new PublicKey(value);
    return { valid: true, slab: value };
  } catch {
    return {
      valid: false,
      response: NextResponse.json({ error: "Invalid slab address format" }, { status: 400 }),
    };
  }
}

/** Validates and parses a numeric parameter (e.g., account index, page number) */
export function validateNumericParam(
  value: string | null | undefined,
  options?: { min?: number; max?: number }
): { valid: false; response: NextResponse } | { valid: true; value: number } {
  if (!value || typeof value !== "string") {
    return {
      valid: false,
      response: NextResponse.json({ error: "Invalid numeric parameter" }, { status: 400 }),
    };
  }

  const num = parseInt(value, 10);
  if (isNaN(num)) {
    return {
      valid: false,
      response: NextResponse.json({ error: "Invalid numeric parameter" }, { status: 400 }),
    };
  }

  if (options?.min !== undefined && num < options.min) {
    return {
      valid: false,
      response: NextResponse.json(
        { error: `Value must be >= ${options.min}` },
        { status: 400 }
      ),
    };
  }

  if (options?.max !== undefined && num > options.max) {
    return {
      valid: false,
      response: NextResponse.json(
        { error: `Value must be <= ${options.max}` },
        { status: 400 }
      ),
    };
  }

  return { valid: true, value: num };
}

/** Validates that a wallet address is a valid Solana public key */
export function validateWalletParam(value: string | null | undefined): { valid: false; response: NextResponse } | { valid: true; wallet: string } {
  if (!value || typeof value !== "string") {
    return {
      valid: false,
      response: NextResponse.json({ error: "Invalid wallet address" }, { status: 400 }),
    };
  }

  try {
    const wallet = new PublicKey(value).toBase58();
    return { valid: true, wallet };
  } catch {
    return {
      valid: false,
      response: NextResponse.json({ error: "Invalid wallet address format" }, { status: 400 }),
    };
  }
}
