"use client";

import { useState, useEffect } from "react";

/**
 * Returns true when the viewport is >= 1024px (Tailwind `lg` breakpoint).
 * Defaults to `false` on SSR/initial render to match the mobile-first approach
 * (`hidden lg:grid`). This prevents the chart from being double-mounted during
 * server rendering, which caused the "two ChartEmptyState stacking" bug.
 */
export function useIsLargeScreen(): boolean {
  const [isLarge, setIsLarge] = useState(false);

  useEffect(() => {
    const mql = window.matchMedia("(min-width: 1024px)");
    setIsLarge(mql.matches);
    const handler = (e: MediaQueryListEvent) => setIsLarge(e.matches);
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }, []);

  return isLarge;
}
