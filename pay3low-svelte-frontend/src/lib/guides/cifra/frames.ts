import type { TutorialFrame, TutorialStep } from "$lib/route-tutorial";

type Copy = (key: string, params?: Record<string, string | number>) => string;
type CifraStep = Pick<TutorialStep, "kind" | "pair" | "from" | "to" | "amount" | "network">;
export const cifraTerminalUrl = "https://tradernet.by/terminal";
export const cifraGuideUrl = "https://cifra.by/knowledge-base";

/** Tradernet's web lessons: https://cifra.by/knowledge-base (desktop.cryptoBuy / desktop.cryptoSell). */
export function cifraTrade(step: Pick<CifraStep, "kind" | "pair" | "from" | "to">) {
  const asset = (value: string) => value.toUpperCase() === "RUR" ? "RUB" : value.toUpperCase();
  const from = asset(step.from), to = asset(step.to);
  if (step.kind === "transfer" || from === to) return null;
  let buying: boolean;
  if (step.pair) {
    const pair = step.pair.toUpperCase().replace(/\.IMEX$/, "").replace(/RUR/g, "RUB").replace(/[^A-Z0-9]/g, "");
    if (pair === `${to}${from}`) buying = true;
    else if (pair === `${from}${to}`) buying = false;
    else return null;
  } else if (step.kind === "buy" || step.kind === "sell") buying = step.kind === "buy";
  else return null;
  const base = buying ? to : from, quote = buying ? from : to;
  return { base, quote, buying, ticker: `${base}-${quote}.IMEX`, field: buying ? "Sum" : "Quantity" };
}

export function buildCifraFrames(step: CifraStep, copy: Copy): TutorialFrame[] {
  const trade = cifraTrade(step);
  const pair = trade?.ticker ?? `${step.from} → ${step.to}`;
  const action = copy(trade ? trade.buying ? "Place a buy order" : "Place a sell order" : "Check the trading pair");
  const fiatSource = ["RUB", "RUR", "BYN", "USD", "EUR"].includes(step.from.toUpperCase());
  return [
    { kind: "open", title: copy("Open your Cifra account"), text: copy("Sign in to Tradernet or open an account from cifra.by. Complete the account verification requested by Cifra. Crypto withdrawals require the advanced verification level.") },
    { kind: "verify", title: copy("Prepare the trading balance"), text: copy(fiatSource
      ? "Press Top up, select {asset} and an available bank payment method. Use the details in your own account and wait for the deposit."
      : "Press Top up and select the source asset {asset}. For a crypto deposit, match the network and copy the address from your own Cifra account. Wait for the deposit.", { asset: step.from }) },
    { kind: "open", title: copy("Find {pair} in Trade", { pair }), text: copy("Open Trade, use the ticker search and select {pair}. Check that you exchange {from} for {to}; confirm that this market is available in your account.", { pair, from: step.from, to: step.to }) },
    { kind: "review", title: copy("Set up a market order"), text: copy(trade?.buying
      ? "Select Buy and Market. In the Sum tab, enter the amount you spend in {asset}. Check the estimated quantity and order total."
      : trade ? "Select Sell and Market. In the Quantity tab, enter the amount of {asset} you sell. Check the order total."
      : "Check the market's base and quote currencies before selecting Buy or Sell. Confirm the source amount and order total.", { asset: trade?.buying ? trade.quote : trade?.base ?? step.from }) },
    { kind: "act", title: copy("Review and submit the order"), text: copy("Check the side, amount, current price and fees. Press {action} only after reviewing them. A market order executes at the available market prices.", { action }) },
    { kind: "receive", title: copy("Check execution and the balance"), text: copy("Check the actual fills in Orders and the {asset} balance in Portfolio. For a bank withdrawal, use Withdraw money. For a crypto withdrawal, check the receiving address, supported network and fee before continuing.", { asset: step.to }) },
  ];
}
