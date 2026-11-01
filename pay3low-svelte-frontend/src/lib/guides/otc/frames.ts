import type { TutorialFrame, TutorialStep } from "$lib/route-tutorial";
import type { Direction } from "$lib/otc/service";

type Copy = (key: string, params?: Record<string, string | number>) => string;
type OtcStep = Pick<TutorialStep, "from" | "to" | "amount" | "inputAmount" | "output">;

/** The OTC desk only quotes this pair, so the requested side follows the asset the traveller sends. */
export function otcSide(step: Pick<TutorialStep, "from">): Direction {
  return step.from.toUpperCase() === "USDT" ? "sell" : "buy";
}

export function otcTerminalHash(step: Pick<TutorialStep, "from">): string {
  return `#/otc?market=EVER-USDT&side=${otcSide(step)}`;
}

/** An offer settles the leg only when its own size already covers the amount being exchanged. */
export function otcOfferCovers(offer: { terms: { ever_amount: string; usdt_amount: string } }, step: OtcStep): boolean {
  const selling = otcSide(step) === "sell";
  const wanted = step.inputAmount ?? step.amount ?? step.output;
  const value = Number((wanted ?? "").replace(/[^0-9.]/g, ""));
  if (!Number.isFinite(value) || value <= 0) return true;
  return Number(selling ? offer.terms.usdt_amount : offer.terms.ever_amount) >= value;
}

export function buildOtcFrames(step: OtcStep, copy: Copy): TutorialFrame[] {
  const side = otcSide(step);
  return [
    { kind: "open", title: copy("Open the Pay3Flow OTC book"), text: copy("The live offers for your direction are listed in this step. Pay3Flow OTC exchanges {from} and {to} directly between the two wallets.", { from: step.from, to: step.to }) },
    { kind: "review", title: copy("Choose an offer that covers your amount"), text: copy("Compare the price and the volume of the best offers. Take an offer sized for your whole amount, not a smaller one, and check that it is still open.") },
    { kind: "verify", title: copy("Check both wallets in the offer"), text: copy(side === "sell"
      ? "Open the offer and check the seller's EVER wallet on Everscale and the USDT receiving address on Ethereum, plus the expiry time."
      : "Open the offer and check the buyer's EVER receiving wallet on Everscale and the USDT paying address on Ethereum, plus the expiry time.") },
    { kind: "act", title: copy("Accept the offer and settle"), text: copy("Press Accept offer and follow the two transfers. There is no escrow, so send your side only after checking the addresses, and only while the offer is open.") },
    { kind: "receive", title: copy("Confirm the balances"), text: copy("Check that {to} arrived in your wallet before you continue to the next step.", { to: step.to }) },
  ];
}
