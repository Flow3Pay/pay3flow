/** The OTC terminal markets that can be selected from the pair card. */
export interface OtcMarket {
  id: string;
  base: string;
  quote: string;
  icon: string;
  network: string;
  venue: string;
  keywords: string;
}

export const otcMarkets: OtcMarket[] = [
  {
    id: "EVER-USDT",
    base: "EVER",
    quote: "USDT",
    icon: "/icons/assets/ever.svg",
    network: "Everscale ↔ Ethereum",
    venue: "OTC",
    keywords: "everscale ethereum ever usdt tether",
  },
  {
    id: "GNK-USDT",
    base: "GNK",
    quote: "USDT",
    icon: "/icons/assets/gnk.jpg",
    network: "Gonka · HOT Omni Balance",
    venue: "HEX",
    keywords: "gonka hot omni balance gnk usdt tether",
  },
];

export function marketByBase(base: string): OtcMarket {
  return otcMarkets.find((market) => market.base.toUpperCase() === base.toUpperCase()) ?? otcMarkets[0];
}
