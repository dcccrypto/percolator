import { describe, it, expect, vi } from "vitest";
import {
  logApiCall,
  logDbQuery,
  logOperation,
  logOperationComplete,
  logTransientError,
  logMarker,
} from "./logHelpers.js";
import type { Logger } from "./logger.js";

// Create a mock logger
const createMockLogger = () => ({
  debug: vi.fn(),
  info: vi.fn(),
  warn: vi.fn(),
  error: vi.fn(),
});

describe("logHelpers", () => {
  describe("logApiCall", () => {
    it("logs successful API calls with info level", () => {
      const logger = createMockLogger() as unknown as Logger;
      logApiCall(logger, "GET", "/markets", 200, 25);

      expect(logger.info).toHaveBeenCalledWith("API GET /markets", {
        method: "GET",
        path: "/markets",
        status: 200,
        durationMs: 25,
      });
    });

    it("logs client errors with warn level", () => {
      const logger = createMockLogger() as unknown as Logger;
      logApiCall(logger, "POST", "/orders", 400, 10);

      expect(logger.warn).toHaveBeenCalledWith("API POST /orders", {
        method: "POST",
        path: "/orders",
        status: 400,
        durationMs: 10,
      });
    });

    it("logs server errors with error level", () => {
      const logger = createMockLogger() as unknown as Logger;
      logApiCall(logger, "DELETE", "/wallet", 500, 50);

      expect(logger.error).toHaveBeenCalledWith("API DELETE /wallet", {
        method: "DELETE",
        path: "/wallet",
        status: 500,
        durationMs: 50,
      });
    });

    it("includes context in log output", () => {
      const logger = createMockLogger() as unknown as Logger;
      logApiCall(logger, "GET", "/market/ABC", 200, 30, { marketId: "ABC123" });

      expect(logger.info).toHaveBeenCalledWith("API GET /market/ABC", {
        method: "GET",
        path: "/market/ABC",
        status: 200,
        durationMs: 30,
        marketId: "ABC123",
      });
    });
  });

  describe("logDbQuery", () => {
    it("logs fast queries with info level", () => {
      const logger = createMockLogger() as unknown as Logger;
      logDbQuery(logger, "SELECT markets", 50);

      expect(logger.info).toHaveBeenCalledWith("DB: SELECT markets", {
        query: "SELECT markets",
        durationMs: 50,
      });
    });

    it("logs slow queries with warn level", () => {
      const logger = createMockLogger() as unknown as Logger;
      logDbQuery(logger, "SELECT positions", 1500);

      expect(logger.warn).toHaveBeenCalledWith("DB: SELECT positions", {
        query: "SELECT positions",
        durationMs: 1500,
        slow: true,
      });
    });

    it("includes context like rows affected", () => {
      const logger = createMockLogger() as unknown as Logger;
      logDbQuery(logger, "INSERT trades", 100, { rowsAffected: 5 });

      expect(logger.info).toHaveBeenCalledWith("DB: INSERT trades", {
        query: "INSERT trades",
        durationMs: 100,
        rowsAffected: 5,
      });
    });

    it("flags queries at 1000ms threshold as slow", () => {
      const logger = createMockLogger() as unknown as Logger;
      logDbQuery(logger, "SELECT large_table", 1000);

      expect(logger.warn).toHaveBeenCalledWith("DB: SELECT large_table", {
        query: "SELECT large_table",
        durationMs: 1000,
        slow: true,
      });
    });

    it("does not flag queries under 1000ms as slow", () => {
      const logger = createMockLogger() as unknown as Logger;
      logDbQuery(logger, "SELECT small_table", 999);

      expect(logger.info).toHaveBeenCalledWith("DB: SELECT small_table", {
        query: "SELECT small_table",
        durationMs: 999,
      });
    });
  });

  describe("logOperation", () => {
    it("logs operation start with info level", () => {
      const logger = createMockLogger() as unknown as Logger;
      logOperation(logger, "market-discovery", "Discovering markets on Mainnet");

      expect(logger.info).toHaveBeenCalledWith(
        "market-discovery: Discovering markets on Mainnet",
        {
          operation: "market-discovery",
        },
      );
    });

    it("includes context in operation log", () => {
      const logger = createMockLogger() as unknown as Logger;
      logOperation(logger, "liquidation-scan", "Scanning for liquidations", {
        programId: "PERC...",
        accountCount: 1000,
      });

      expect(logger.info).toHaveBeenCalledWith(
        "liquidation-scan: Scanning for liquidations",
        {
          operation: "liquidation-scan",
          programId: "PERC...",
          accountCount: 1000,
        },
      );
    });
  });

  describe("logOperationComplete", () => {
    it("logs operation completion with info level", () => {
      const logger = createMockLogger() as unknown as Logger;
      logOperationComplete(logger, "market-discovery", 5000);

      expect(logger.info).toHaveBeenCalledWith("market-discovery: Complete in 5000ms", {
        operation: "market-discovery",
        durationMs: 5000,
        completed: true,
      });
    });

    it("includes result summary in completion log", () => {
      const logger = createMockLogger() as unknown as Logger;
      logOperationComplete(logger, "market-discovery", 5000, {
        discovered: 42,
        updated: 8,
        failed: 2,
      });

      expect(logger.info).toHaveBeenCalledWith("market-discovery: Complete in 5000ms", {
        operation: "market-discovery",
        durationMs: 5000,
        completed: true,
        discovered: 42,
        updated: 8,
        failed: 2,
      });
    });
  });

  describe("logTransientError", () => {
    it("logs transient errors with warn level", () => {
      const logger = createMockLogger() as unknown as Logger;
      const error = new Error("429 Too many requests");
      logTransientError(logger, "fetchSlab", 1, 3, error, 500);

      expect(logger.warn).toHaveBeenCalledWith(
        "fetchSlab: Attempt 1/3 failed, retrying in 500ms",
        {
          operation: "fetchSlab",
          attempt: 1,
          maxAttempts: 3,
          error: "429 Too many requests",
          retryDelayMs: 500,
        },
      );
    });

    it("includes context when retry fails", () => {
      const logger = createMockLogger() as unknown as Logger;
      const error = new Error("Network timeout");
      logTransientError(logger, "rpc-call", 2, 5, error, 1000, {
        endpoint: "https://api.mainnet-beta.solana.com",
        method: "getAccountInfo",
      });

      expect(logger.warn).toHaveBeenCalledWith(
        "rpc-call: Attempt 2/5 failed, retrying in 1000ms",
        {
          operation: "rpc-call",
          attempt: 2,
          maxAttempts: 5,
          error: "Network timeout",
          retryDelayMs: 1000,
          endpoint: "https://api.mainnet-beta.solana.com",
          method: "getAccountInfo",
        },
      );
    });

    it("handles non-Error errors", () => {
      const logger = createMockLogger() as unknown as Logger;
      logTransientError(logger, "task", 1, 2, "Unknown error", 500);

      expect(logger.warn).toHaveBeenCalledWith("task: Attempt 1/2 failed, retrying in 500ms", {
        operation: "task",
        attempt: 1,
        maxAttempts: 2,
        error: "Unknown error",
        retryDelayMs: 500,
      });
    });
  });

  describe("logMarker", () => {
    it("logs debug markers with info level", () => {
      const logger = createMockLogger() as unknown as Logger;
      logMarker(logger, "entered-liquidation-loop");

      expect(logger.debug).toHaveBeenCalledWith("[entered-liquidation-loop]", {
        marker: "entered-liquidation-loop",
      });
    });

    it("includes context in marker log", () => {
      const logger = createMockLogger() as unknown as Logger;
      logMarker(logger, "cache-stale", { cacheAgeMs: 45000, maxAgeMs: 30000 });

      expect(logger.debug).toHaveBeenCalledWith("[cache-stale]", {
        marker: "cache-stale",
        cacheAgeMs: 45000,
        maxAgeMs: 30000,
      });
    });
  });
});
