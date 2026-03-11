/**
 * Optional structured logging helper functions for Percolator services.
 *
 * These utilities provide a consistent logging pattern for common scenarios across services.
 * They are completely optional - existing logger.info/warn/error calls continue to work unchanged.
 *
 * Usage:
 * ```typescript
 * import { logApiCall, logDbQuery, logOperation } from "@percolator/shared";
 * 
 * // Log API calls
 * logApiCall(logger, "GET", "/markets", 200, 45);
 *
 * // Log database queries
 * logDbQuery(logger, "SELECT * FROM markets", 23);
 *
 * // Log long-running operations
 * logOperation(logger, "market-discovery", "Discovering markets from X to Y", { totalMarkets: 42 });
 * ```
 */

import type { Logger } from "./logger.js";

/**
 * Log an API request/response with consistent structure.
 *
 * Useful for REST API routes and external service calls.
 *
 * @param logger - Logger instance
 * @param method - HTTP method (GET, POST, PUT, DELETE, etc.)
 * @param path - Request path or endpoint
 * @param statusCode - HTTP status code (200, 404, 500, etc.)
 * @param durationMs - Time taken in milliseconds
 * @param context - Additional context to include in log
 *
 * @example
 * ```typescript
 * logApiCall(logger, "POST", "/api/markets", 201, 50, { marketId: "ABC123" });
 * // Output: {"method":"POST","path":"/api/markets","status":201,"durationMs":50,"marketId":"ABC123"}
 * ```
 */
export function logApiCall(
  logger: Logger,
  method: string,
  path: string,
  statusCode: number,
  durationMs: number,
  context?: Record<string, unknown>,
): void {
  const isError = statusCode >= 400;
  const level: "info" | "warn" | "error" = statusCode >= 500 ? "error" : isError ? "warn" : "info";

  logger[level](`API ${method} ${path}`, {
    method,
    path,
    status: statusCode,
    durationMs,
    ...context,
  });
}

/**
 * Log a database query execution with timing.
 *
 * Useful for tracking query performance and identifying slow queries.
 *
 * @param logger - Logger instance
 * @param queryType - Type of query (SELECT, INSERT, UPDATE, DELETE) or descriptive name
 * @param durationMs - Query execution time in milliseconds
 * @param context - Additional context (rows affected, etc.)
 *
 * @example
 * ```typescript
 * logDbQuery(logger, "SELECT markets", 45, { rows: 23 });
 * // Output: {"queryType":"SELECT markets","durationMs":45,"rows":23}
 * ```
 */
export function logDbQuery(
  logger: Logger,
  queryType: string,
  durationMs: number,
  context?: Record<string, unknown>,
): void {
  const isLong = durationMs >= 1000; // Flag queries 1 second or longer as warning
  const level: "info" | "warn" = isLong ? "warn" : "info";

  logger[level](`DB: ${queryType}`, {
    query: queryType,
    durationMs,
    ...(isLong && { slow: true }),
    ...context,
  });
}

/**
 * Log a long-running operation with start message and context.
 *
 * Useful for background jobs, cranks, liquidation scans, market discovery, etc.
 *
 * @param logger - Logger instance
 * @param operationName - Name of the operation (e.g., "liquidation-scan", "market-discovery")
 * @param description - Human-readable description of what's happening
 * @param context - Additional context (counts, addresses, etc.)
 *
 * @example
 * ```typescript
 * logOperation(logger, "market-discovery", "Discovering markets on Mainnet", {
 *   programIds: ["PERC..."],
 *   scannedAccounts: 1000,
 * });
 * // Output: {"operation":"market-discovery","description":"...","programIds":[...],"scannedAccounts":1000}
 * ```
 */
export function logOperation(
  logger: Logger,
  operationName: string,
  description: string,
  context?: Record<string, unknown>,
): void {
  logger.info(`${operationName}: ${description}`, {
    operation: operationName,
    ...context,
  });
}

/**
 * Log operation completion with result summary.
 *
 * Call after a long-running operation completes successfully.
 *
 * @param logger - Logger instance
 * @param operationName - Name of the operation (should match logOperation)
 * @param durationMs - Total duration in milliseconds
 * @param result - Summary of operation result (counts, statistics, etc.)
 *
 * @example
 * ```typescript
 * const startTime = Date.now();
 * // ... do work ...
 * logOperationComplete(logger, "market-discovery", Date.now() - startTime, {
 *   discovered: 42,
 *   updated: 8,
 *   failed: 2,
 * });
 * ```
 */
export function logOperationComplete(
  logger: Logger,
  operationName: string,
  durationMs: number,
  result?: Record<string, unknown>,
): void {
  logger.info(`${operationName}: Complete in ${durationMs}ms`, {
    operation: operationName,
    durationMs,
    completed: true,
    ...result,
  });
}

/**
 * Log a transient error with retry information.
 *
 * Use when an operation will be retried (e.g., rate-limited RPC call, network timeout).
 *
 * @param logger - Logger instance
 * @param operationName - Name of operation being retried
 * @param attempt - Current attempt number (1-indexed)
 * @param maxAttempts - Maximum attempts allowed
 * @param error - Error that occurred
 * @param nextRetryDelayMs - Milliseconds to wait before retry
 * @param context - Additional context
 *
 * @example
 * ```typescript
 * logTransientError(logger, "fetchSlab", 1, 3, new Error("429 Too many requests"), 500, {
 *   slabAddress: "...",
 * });
 * ```
 */
export function logTransientError(
  logger: Logger,
  operationName: string,
  attempt: number,
  maxAttempts: number,
  error: unknown,
  nextRetryDelayMs: number,
  context?: Record<string, unknown>,
): void {
  const message = error instanceof Error ? error.message : String(error);

  logger.warn(`${operationName}: Attempt ${attempt}/${maxAttempts} failed, retrying in ${nextRetryDelayMs}ms`, {
    operation: operationName,
    attempt,
    maxAttempts,
    error: message,
    retryDelayMs: nextRetryDelayMs,
    ...context,
  });
}

/**
 * Log a marker for debugging or tracking specific code paths.
 *
 * Useful for tracing execution flow during investigation.
 *
 * @param logger - Logger instance
 * @param marker - Name of the marker (e.g., "entered-liquidation-loop", "cache-hit")
 * @param context - Optional context to include
 *
 * @example
 * ```typescript
 * logMarker(logger, "cache-stale", { cacheAgeMs: 45000, maxAgeMsMs: 30000 });
 * // Output: {"marker":"cache-stale","cacheAgeMs":45000,"maxAgeMs":30000}
 * ```
 */
export function logMarker(
  logger: Logger,
  marker: string,
  context?: Record<string, unknown>,
): void {
  logger.debug(`[${marker}]`, { marker, ...context });
}
