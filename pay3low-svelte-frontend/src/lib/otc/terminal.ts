import type { Config, Direction, Proposal, Trade } from "./service";

export function proposalDirection(proposal: Proposal, config: Config | null): Direction | null {
  if (!config) return null;
  const output = proposal.publishes?.resourceConformsTo, input = proposal.reciprocal?.resourceConformsTo;
  if (output === config.ever_resource && input === config.usdt_resource) return "buy";
  if (output === config.usdt_resource && input === config.ever_resource) return "sell";
  return null;
}
/** Validate decimal text without converting executable values to a Number. */
export function exactInput(value: string, direction: Direction): string | null {
  const precision = direction === "buy" ? 6 : 9;
  if (value.length > 64 || !/^\d+(?:\.\d+)?$/.test(value)) return null;
  const [whole, fraction = ""] = value.split(".");
  if (fraction.length > precision) return null;
  const units = BigInt(whole) * 10n ** BigInt(precision) + BigInt(fraction.padEnd(precision, "0") || "0");
  if (units <= 0n || units >= 2n ** 128n) return null;
  const tail = fraction.replace(/0+$/, "");
  return `${BigInt(whole)}${tail ? `.${tail}` : ""}`;
}
export const terminalStates = new Set(["completed", "refunded", "rejected", "expired"]);
export function completedVolume(trades: Trade[]): string {
  return trades.filter(trade => trade.state === "completed").reduce((sum, trade) => sum + BigInt(trade.terms.fee_policy.basis_units), 0n).toString();
}
