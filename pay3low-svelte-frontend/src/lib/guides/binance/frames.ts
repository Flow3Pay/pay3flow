import type { TutorialFrame } from "$lib/route-tutorial";

type Copy = (key: string, params?: Record<string, string | number>) => string;

export function buildBinanceProfileFrames(buying: boolean, asset: string, fiat: string, copy: Copy): TutorialFrame[] {
  return [
    { kind: "open", title: copy("Open the advertiser profile"), text: copy("Open the Binance advertiser profile and check the nickname and trading statistics.") },
    { kind: "review", title: copy("Open More details"), text: copy("Hover over the three dots beside the statistics and open More details. Read the trading information in the window.") },
    { kind: "verify", title: copy("Close details and find the advertisement"), text: copy(buying ? "Close the window and find an active advertisement to buy {asset} with {fiat}. Compare its price, limits and payment method." : "Close the window and find an active advertisement to sell {asset} for {fiat}. Compare its price, limits and payment method.", { asset, fiat }) },
    { kind: "act", title: copy(buying ? "Press Buy {asset}" : "Press Sell {asset}", { asset }), text: copy(buying ? "Click Buy {asset} on the matching advertisement." : "Click Sell {asset} on the matching advertisement.", { asset }) },
  ];
}
