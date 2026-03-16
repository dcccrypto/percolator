/**
 * Test Setup Utilities - Keypair Security Validation
 *
 * Provides secure keypair loading with automatic file permission validation.
 * Ensures test keypairs have restrictive permissions (600) to prevent exposure.
 *
 * Usage:
 *   import { loadTestKeypair } from "./setup";
 *   const deployer = loadTestKeypair("/tmp/deployer.json");
 */

import * as fs from "fs";
import { Keypair } from "@solana/web3.js";

/**
 * Logger utility for test setup messages
 */
const logger = {
  info: (msg: string) => console.log(`[TEST-SETUP] ✓ ${msg}`),
  warn: (msg: string) => console.warn(`[TEST-SETUP] ⚠️  ${msg}`),
  error: (msg: string) => console.error(`[TEST-SETUP] ✗ ${msg}`),
};

/**
 * Load a keypair from file with security validation
 *
 * Validates that keypair files have restrictive permissions (600).
 * On Unix/Linux systems, loose permissions allow group/others to read private keys.
 * Auto-fixes permissions if necessary.
 *
 * @param filePath - Path to keypair JSON file
 * @param options - Configuration options
 * @returns Keypair instance
 * @throws Error if file doesn't exist or contains invalid keypair data
 *
 * @example
 * ```typescript
 * const deployer = loadTestKeypair("/tmp/deployer.json");
 * const trader = loadTestKeypair(
 *   `${process.env.HOME}/.config/solana/id.json`,
 *   { skipAutoFix: true }
 * );
 * ```
 */
export function loadTestKeypair(
  filePath: string,
  options: {
    /** Skip auto-fixing permissions and just warn (default: false) */
    skipAutoFix?: boolean;
    /** Fail startup if permissions are incorrect (default: false) */
    strictMode?: boolean;
  } = {}
): Keypair {
  const { skipAutoFix = false, strictMode = false } = options;

  // Verify file exists
  if (!fs.existsSync(filePath)) {
    logger.error(`Keypair file not found: ${filePath}`);
    throw new Error(`Keypair file not found: ${filePath}`);
  }

  // Check file permissions
  try {
    const stats = fs.statSync(filePath);
    const mode = stats.mode & parseInt("777", 8);
    const expectedMode = parseInt("600", 8);

    // If permissions are not 600, warn and optionally fix
    if (mode !== expectedMode) {
      const currentPerms = mode.toString(8).padStart(3, "0");
      const currentPermsOctal = `0${currentPerms}`;

      logger.warn(
        `Keypair file has insecure permissions: ${currentPermsOctal} (expected 0600)`
      );
      logger.warn(`  File: ${filePath}`);

      if (strictMode) {
        logger.error(`STRICT MODE: Refusing to load keypair with insecure permissions`);
        logger.error(`  Fix with: chmod 600 "${filePath}"`);
        throw new Error(
          `Keypair file ${filePath} has insecure permissions (${currentPermsOctal}). ` +
            `Strict mode enabled. Fix with: chmod 600 "${filePath}"`
        );
      }

      if (skipAutoFix) {
        logger.warn(
          `  To fix: chmod 600 "${filePath}"`
        );
      } else {
        // Auto-fix permissions
        try {
          fs.chmodSync(filePath, 0o600);
          logger.info(
            `Auto-fixed keypair file permissions to 0600: ${filePath}`
          );
        } catch (err) {
          const errMsg = err instanceof Error ? err.message : String(err);
          logger.error(`Failed to auto-fix permissions: ${errMsg}`);
          if (strictMode) {
            throw err;
          }
          logger.warn(`  Proceeding with current permissions (not recommended)`);
        }
      }
    } else {
      logger.info(`Keypair file permissions verified (0600): ${filePath}`);
    }
  } catch (err) {
    // Only re-throw if it's a security-critical error we threw
    if (err instanceof Error && err.message.includes("insecure permissions")) {
      throw err;
    }
    // Log stat errors but continue (permission checking may not work on all systems)
    if (err instanceof Error) {
      logger.warn(`Could not verify file permissions: ${err.message}`);
    }
  }

  // Load keypair data
  let keypairData: number[];
  try {
    const fileContent = fs.readFileSync(filePath, "utf8");
    keypairData = JSON.parse(fileContent);
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err);
    logger.error(
      `Failed to parse keypair file ${filePath}: ${errMsg}`
    );
    throw new Error(
      `Invalid keypair JSON in ${filePath}. Ensure file contains valid Solana keypair array. Error: ${errMsg}`
    );
  }

  // Validate keypair data format
  if (
    !Array.isArray(keypairData) ||
    keypairData.length !== 64 ||
    !keypairData.every((x) => typeof x === "number" && x >= 0 && x <= 255)
  ) {
    logger.error(
      `Invalid keypair format in ${filePath}. Expected 64-byte array.`
    );
    throw new Error(
      `Invalid keypair format in ${filePath}. Expected array of 64 numbers (0-255).`
    );
  }

  // Create Keypair instance
  try {
    const keypair = Keypair.fromSecretKey(new Uint8Array(keypairData));
    logger.info(`Loaded keypair: ${keypair.publicKey.toBase58()}`);
    return keypair;
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err);
    logger.error(`Failed to create Keypair from data: ${errMsg}`);
    throw new Error(
      `Failed to create Keypair from ${filePath}: ${errMsg}`
    );
  }
}

/**
 * Validate all test keypair files in a directory
 *
 * Scans directory for .json files and validates them as keypairs.
 * Returns validation results without throwing errors.
 *
 * @param dirPath - Directory to scan
 * @returns Array of validation results
 *
 * @example
 * ```typescript
 * const results = validateTestKeypairDirectory("/tmp");
 * results.forEach(r => {
 *   if (r.success) {
 *     console.log(`✓ ${r.file}: ${r.publicKey}`);
 *   } else {
 *     console.error(`✗ ${r.file}: ${r.error}`);
 *   }
 * });
 * ```
 */
export function validateTestKeypairDirectory(dirPath: string): Array<{
  file: string;
  success: boolean;
  publicKey?: string;
  permissions?: string;
  error?: string;
}> {
  const results: Array<{
    file: string;
    success: boolean;
    publicKey?: string;
    permissions?: string;
    error?: string;
  }> = [];

  try {
    const files = fs.readdirSync(dirPath);
    const jsonFiles = files.filter((f) => f.endsWith(".json"));

    for (const file of jsonFiles) {
      const filePath = `${dirPath}/${file}`;
      const result: {
        file: string;
        success: boolean;
        publicKey?: string;
        permissions?: string;
        error?: string;
      } = {
        file,
        success: false,
      };

      try {
        // Check permissions
        const stats = fs.statSync(filePath);
        const mode = stats.mode & parseInt("777", 8);
        result.permissions = `0${mode.toString(8).padStart(3, "0")}`;

        // Load and validate keypair
        const kp = loadTestKeypair(filePath, { skipAutoFix: true });
        result.publicKey = kp.publicKey.toBase58();
        result.success = true;
      } catch (err) {
        result.error = err instanceof Error ? err.message : String(err);
      }

      results.push(result);
    }
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err);
    logger.error(`Failed to scan directory ${dirPath}: ${errMsg}`);
  }

  return results;
}

/**
 * Test helper: Initialize keypairs needed for integration tests
 *
 * Loads default test keypair locations and returns them.
 * Useful for test suite initialization.
 *
 * @example
 * ```typescript
 * const { deployer, crank, oracle } = initTestKeypairs();
 * ```
 */
export function initTestKeypairs(): {
  deployer: Keypair;
  [key: string]: Keypair;
} {
  const keypairs: { deployer: Keypair; [key: string]: Keypair } = {
    deployer: loadTestKeypair("/tmp/deployer.json"),
  };

  // Optionally load other standard test keypairs
  const otherKeys = ["crank", "oracle", "trader", "liquidator"];
  for (const key of otherKeys) {
    const filePath = `/tmp/${key}.json`;
    if (fs.existsSync(filePath)) {
      try {
        keypairs[key] = loadTestKeypair(filePath);
      } catch (err) {
        logger.warn(`Could not load ${key} keypair from ${filePath}`);
      }
    }
  }

  return keypairs;
}
