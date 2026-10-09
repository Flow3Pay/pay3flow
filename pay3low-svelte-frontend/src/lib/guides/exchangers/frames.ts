import type { TutorialFrame, TutorialStep } from "$lib/route-tutorial";

type Copy = (key: string, params?: Record<string, string | number>) => string;
export const swapGuideVenues = ["bncex", "bitcoin-center", "dzengi", "symbiosis", "cow-swap"];

export function swapGuideAction(provider: string): string {
  return ({ bncex: "Continue", "bitcoin-center": "Sign in to continue", dzengi: "Convert", symbiosis: "Swap", "cow-swap": "Review swap" } as Record<string, string>)[provider] ?? "Exchange";
}

export function buildVenueSwapFrames(step: Pick<TutorialStep, "provider" | "venue" | "from" | "to">, copy: Copy): TutorialFrame[] {
  const provider = step.provider.toLowerCase();
  const wallet = provider === "symbiosis" || provider === "cow-swap";
  const open = provider === "bncex" ? "On bncex, choose Buy or Sell for your direction, then select the payment method and crypto network."
    : provider === "bitcoin-center" ? "On Bitcoin Center, choose the asset or payment method in You send and You receive. Match the crypto network to your route."
    : provider === "dzengi" ? "On Dzengi, select the currencies in the converter. Its calculator gives an indicative quote; the actual conversion takes place on the trading platform."
    : "Select the token and network you send, then the token and network you receive. Connect your wallet on {venue}.";
  const review = provider === "bncex" ? "Review the received amount and the calculator's fee breakdown. Refresh the quote if its timer expires."
    : provider === "bitcoin-center" ? "Review the amount in You receive and the exchange terms before opening Order setup."
    : provider === "dzengi" ? "Review the estimated received amount. Check the actual trading price and fees after signing in to Dzengi."
    : "Review the estimated received amount, provider fees and slippage on {venue}.";
  const act = provider === "cow-swap" ? "Open Review swap on CoW Swap. Review the order, approve the token if requested, and sign only after checking the amounts in your wallet."
    : wallet ? "Press Swap on Symbiosis after connecting your wallet and reviewing the quote. Check any approval and transaction in your wallet."
    : provider === "bitcoin-center" ? "Open Sign in to continue in Order setup. Sign in or register on Bitcoin Center to proceed with the selected exchange."
    : provider === "dzengi" ? "Press Convert to open Dzengi's trading platform. Sign in and check the actual trading terms there."
    : "Press Continue on bncex. Sign in or complete verification if requested and follow the provider's instructions.";
  return [
    { kind: "open", title: copy(provider === "dzengi" ? "Select currencies" : "Select currencies and networks"), text: copy(open, { venue: step.venue }) },
    { kind: "verify", title: copy("Enter the amount"), text: copy(provider === "dzengi" ? "Enter the amount you want to convert from {from} to {to}." : "Enter the amount you send for {from} → {to}. Check the selected payment method or network.", { from: step.from, to: step.to }) },
    { kind: "review", title: copy("Check the amount you receive"), text: copy(review, { venue: step.venue }) },
    { kind: "act", title: copy("Press {action}", { action: copy(swapGuideAction(provider)) }), text: copy(act) },
  ];
}
