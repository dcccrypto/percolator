/**
 * StepReview component tests
 *
 * Primary coverage: GH#1117 — airdrop promise must not appear for custom (non-mirror) tokens.
 * Custom tokens have the user wallet as mint authority; only Percolator mirror tokens
 * can be auto-airdropped after market creation.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { StepReview } from "@/components/create/StepReview";

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock("@/lib/config", () => ({
  getNetwork: vi.fn(() => "devnet"),
}));

vi.mock("@/components/create/CostEstimate", () => ({
  CostEstimate: () => <div data-testid="cost-estimate" />,
}));

vi.mock("next/link", () => ({
  default: ({ children, href, ...props }: { children: React.ReactNode; href: string }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

// ── Shared props ──────────────────────────────────────────────────────────────

const baseProps = {
  tokenSymbol: "BTC",
  tokenName: "Bitcoin",
  mintAddress: "So11111111111111111111111111111111111111112",
  tokenDecimals: 6,
  priceUsd: 60000,
  mintValid: true,
  mintExistsOnNetwork: true,
  oracleType: "admin" as const,
  oracleLabel: "Admin Oracle",
  slabTier: "small" as const,
  tradingFeeBps: 30,
  initialMarginBps: 1000,
  lpCollateral: "1000",
  insuranceAmount: "100",
  walletConnected: true,
  walletBalanceSol: 1.5,
  hasSufficientBalance: true,
  requiredSol: 0.46,
  hasTokens: true,
  hasSufficientTokensForSeed: true,
  feeConflict: false,
  onBack: vi.fn(),
  onLaunch: vi.fn(),
  canLaunch: true,
};

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("StepReview — GH#1117: airdrop promise visibility by mint authority", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows airdrop notice when isPercolatorMirror=true (Percolator mirror token)", () => {
    render(<StepReview {...baseProps} isPercolatorMirror={true} />);

    // Auto-airdrop promise should be visible
    expect(
      screen.getByText(/tokens automatically after the market is created/i)
    ).toBeDefined();
    expect(screen.getByText(/No tokens needed upfront/i)).toBeDefined();

    // Custom-token deposit notice should NOT appear
    expect(screen.queryByText(/mint tokens from your wallet/i)).toBeNull();
  });

  it("hides airdrop notice when isPercolatorMirror=false (custom token, user = mint authority)", () => {
    render(<StepReview {...baseProps} isPercolatorMirror={false} />);

    // Airdrop promise must not show for custom tokens
    expect(
      screen.queryByText(/tokens automatically after the market is created/i)
    ).toBeNull();
    expect(screen.queryByText(/No tokens needed upfront/i)).toBeNull();

    // Should instead show the custom-token deposit guidance
    expect(
      screen.getByText(/mint tokens from your wallet/i)
    ).toBeDefined();
    expect(screen.getByText(/Custom token/i)).toBeDefined();
  });

  it("hides airdrop notice when isPercolatorMirror is omitted (defaults to false)", () => {
    // isPercolatorMirror default = false — safe default for unknown/custom tokens
    render(<StepReview {...baseProps} />);

    expect(
      screen.queryByText(/tokens automatically after the market is created/i)
    ).toBeNull();
    expect(screen.getByText(/Custom token/i)).toBeDefined();
  });

  it("shows correct launch button label for mirror token on devnet", () => {
    render(<StepReview {...baseProps} isPercolatorMirror={true} />);
    expect(screen.getByRole("button", { name: /LAUNCH & MINT TOKENS/i })).toBeDefined();
  });

  it("shows generic launch button label for custom token on devnet", () => {
    render(<StepReview {...baseProps} isPercolatorMirror={false} />);
    expect(screen.getByRole("button", { name: /LAUNCH MARKET/i })).toBeDefined();
    // Must NOT show the mint-tokens label for custom tokens
    expect(screen.queryByRole("button", { name: /LAUNCH & MINT TOKENS/i })).toBeNull();
  });
});

describe("StepReview — mint validation banners", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows invalid mint banner when mintValid=false", () => {
    render(
      <StepReview
        {...baseProps}
        mintValid={false}
        mintExistsOnNetwork={false}
        isPercolatorMirror={false}
      />
    );
    // The red banner includes the text; the button label also includes it.
    // Use getAllByText to allow multiple matches — both should exist.
    const matches = screen.getAllByText(/Invalid mint/i);
    expect(matches.length).toBeGreaterThan(0);
  });

  it("shows mint-not-found warning when mintValid=true but mintExistsOnNetwork=false", () => {
    render(
      <StepReview
        {...baseProps}
        mintValid={true}
        mintExistsOnNetwork={false}
        isPercolatorMirror={false}
      />
    );
    expect(screen.getByText(/Mint not found on devnet/i)).toBeDefined();
  });

  it("shows verified banner when mint is valid and exists on network", () => {
    render(<StepReview {...baseProps} />);
    expect(screen.getByText(/Mint verified on devnet/i)).toBeDefined();
  });
});

describe("StepReview — SOL balance display", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows current SOL balance when sufficient", () => {
    render(<StepReview {...baseProps} walletBalanceSol={1.5} hasSufficientBalance={true} />);
    expect(screen.getByText(/1\.5000 SOL/)).toBeDefined();
  });

  it("shows insufficient SOL error with required amount", () => {
    render(
      <StepReview
        {...baseProps}
        walletBalanceSol={0.1}
        hasSufficientBalance={false}
        requiredSol={0.46}
      />
    );
    // "Insufficient SOL" appears in both the balance row and the disabled button.
    // Use getAllByText to allow both; check at least one match exists.
    const insufficientMatches = screen.getAllByText(/Insufficient SOL/i);
    expect(insufficientMatches.length).toBeGreaterThan(0);
    // The balance detail row contains the required SOL amount
    expect(screen.getByText(/need ~0\.4600 SOL/)).toBeDefined();
  });
});
