import type { TutorialFrame } from "$lib/route-tutorial";

type Copy = (key: string, params?: Record<string, string | number>) => string;

export function buildP2pProfileFrames(venue: "Bybit" | "MEXC", buying: boolean, asset: string, fiat: string, copy: Copy): TutorialFrame[] {
  const offerText = venue === "MEXC" ? copy(buying
    ? 'In Ads, use “Buy from the User” to buy {asset} with {fiat}. Compare the price, limits and payment method with your route.'
    : 'In Ads, use “Sell to the User” to sell {asset} for {fiat}. Compare the price, limits and payment method with your route.', { asset, fiat })
    : copy(buying
      ? "Return to Ads and find the offer to buy {asset} with {fiat}. Compare the price, limits and payment method with your route."
      : "Return to Ads and find the offer to sell {asset} for {fiat}. Compare the price, limits and payment method with your route.", { asset, fiat });
  return [
    { kind: "open", title: copy("Open the advertiser profile"), text: copy("The link opens the advertiser's {venue} profile. Check the nickname and scroll down to Ads and Reviews.", { venue }) },
    { kind: "review", title: copy("Read the reviews"), text: copy("Open Reviews and read the counterparty's feedback before choosing an advertisement.") },
    { kind: "verify", title: copy("Return to Ads"), text: offerText },
    { kind: "act", title: copy(buying ? "Press Buy {asset}" : "Press Sell {asset}", { asset }), text: copy(buying
      ? "Click Buy {asset} on the matching advertisement."
      : "Click Sell {asset} on the matching advertisement.", { asset }) },
  ];
}
