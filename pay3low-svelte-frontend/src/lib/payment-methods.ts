import { assetIcon } from "$lib/icons";
import { apiUrl } from "$lib/api";

export type PaymentMethodRole = "sender" | "recipient" | "both";

export interface PaymentCountry {
  code: string;
  name: string;
  currency: string;
  mark: string;
}

export interface PaymentMethod {
  id: string;
  name: string;
  country: string;
  currency: string;
  role: PaymentMethodRole;
  kind: "bank" | "cash" | "wallet";
  color: string;
  initials: string;
  popular?: boolean;
  iconUrl?: string;
  /** Estimated bank transfer fee for the selected local payment method. */
  bankFeePercent?: number;
  /** Name fragment understood by the public P2P venue filters. */
  p2pQuery: string;
  /** Stable institution/account family shared by the same method in multiple currencies. */
  currencyGroup?: string;
}

interface BankDirectoryPage {
  items: Array<{
    method_id?: string;
    name: string;
    display_name: string;
    role: PaymentMethodRole;
    country: string;
    currency: string;
    kind: "bank" | "cash";
    color: string;
    initials: string;
    popular: boolean;
    icon_url: string;
    bank_fee_percent?: number;
    p2p_query: string;
    currency_group?: string;
  }>;
}

/** Load the picker catalog generated from the backend Providerfiles. */
export async function fetchPaymentMethods(): Promise<PaymentMethod[]> {
  const response = await fetch(apiUrl("/api/banks?picker_visible=true&limit=100"));
  if (!response.ok) throw new Error(`payment-method catalog failed (${response.status})`);
  const page = await response.json() as BankDirectoryPage;
  const methods = page.items.flatMap((item) => item.method_id ? [{
    id: item.method_id,
    name: item.display_name || item.name,
    country: item.country,
    currency: item.currency,
    role: item.role,
    kind: item.kind,
    color: item.color,
    initials: item.initials,
    popular: item.popular || undefined,
    iconUrl: item.icon_url || undefined,
    bankFeePercent: item.bank_fee_percent,
    p2pQuery: item.p2p_query || item.display_name || item.name,
    currencyGroup: item.currency_group,
  }] : []);

  // Keep the UI usable during a rolling deployment where the frontend may be
  // newer than the backend catalog. Once the backend publishes any BY methods,
  // its enabled/disabled state remains authoritative.
  if (methods.some((method) => method.country === "BY")) return methods;
  return [...methods, ...PAYMENT_METHODS.filter((method) => method.country === "BY")];
}

/** Country labels used by the bootstrap fallback and compact fiat UI. */
export const PAYMENT_COUNTRIES: PaymentCountry[] = [
  { code: "GLOBAL", name: "International", currency: "USD", mark: "$" },
  { code: "AM", name: "Armenia", currency: "AMD", mark: "AM" },
  { code: "RU", name: "Russia", currency: "RUB", mark: "RU" },
  { code: "BY", name: "Belarus", currency: "BYN", mark: "BY" },
];

/**
 * Bootstrap fallback used before the Providerfile-generated API catalog loads.
 * The backend catalog is authoritative for picker-visible fiat methods.
 */
export const PAYMENT_METHODS: PaymentMethod[] = [
  {
    id: "global-usd-cash",
    name: "Cash USD",
    country: "GLOBAL",
    currency: "USD",
    role: "both",
    kind: "cash",
    color: "#168451",
    initials: "$",
    popular: true,
    p2pQuery: "Cash",
  },
  {
    id: "am-ameriabank-usd-account",
    name: "Ameriabank",
    country: "AM",
    currency: "USD",
    role: "both",
    kind: "bank",
    color: "#6d2c91",
    initials: "AM",
    popular: true,
    iconUrl: "/icons/assets/ameriabank.png",
    bankFeePercent: 0,
    p2pQuery: "Ameriabank",
    currencyGroup: "ameriabank",
  },
  {
    id: "am-ameriabank",
    name: "Ameriabank",
    country: "AM",
    currency: "AMD",
    role: "both",
    kind: "bank",
    color: "#6d2c91",
    initials: "AM",
    popular: true,
    iconUrl: "/icons/assets/ameriabank.png",
    bankFeePercent: 0,
    p2pQuery: "Ameriabank",
    currencyGroup: "ameriabank",
  },
  {
    id: "am-idbank-usd-account",
    name: "IDBank",
    country: "AM",
    currency: "USD",
    role: "both",
    kind: "bank",
    color: "#21a366",
    initials: "ID",
    popular: true,
    iconUrl: "/icons/assets/idbank.png",
    bankFeePercent: 0.75,
    p2pQuery: "IDBank",
    currencyGroup: "idbank",
  },
  {
    id: "am-idbank",
    name: "IDBank",
    country: "AM",
    currency: "AMD",
    role: "both",
    kind: "bank",
    color: "#21a366",
    initials: "ID",
    popular: true,
    iconUrl: "/icons/assets/idbank.png",
    bankFeePercent: 0.75,
    p2pQuery: "IDBank",
    currencyGroup: "idbank",
  },
  {
    id: "am-acba-usd-account",
    name: "ACBA Bank",
    country: "AM",
    currency: "USD",
    role: "both",
    kind: "bank",
    color: "#ef7f1a",
    initials: "AC",
    popular: true,
    iconUrl: "/icons/assets/acba.png",
    bankFeePercent: 0,
    p2pQuery: "ACBA",
    currencyGroup: "acba",
  },
  {
    id: "am-acba",
    name: "ACBA Bank",
    country: "AM",
    currency: "AMD",
    role: "both",
    kind: "bank",
    color: "#ef7f1a",
    initials: "AC",
    popular: true,
    iconUrl: "/icons/assets/acba.png",
    bankFeePercent: 0,
    p2pQuery: "ACBA",
    currencyGroup: "acba",
  },
  {
    id: "am-ardshinbank-usd-account",
    name: "Ardshinbank",
    country: "AM",
    currency: "USD",
    role: "both",
    kind: "bank",
    color: "#0877bd",
    initials: "AR",
    iconUrl: "/icons/assets/ardshinbank.png",
    p2pQuery: "Ardshinbank",
    currencyGroup: "ardshinbank",
  },
  {
    id: "am-ardshinbank",
    name: "Ardshinbank",
    country: "AM",
    currency: "AMD",
    role: "both",
    kind: "bank",
    color: "#0877bd",
    initials: "AR",
    iconUrl: "/icons/assets/ardshinbank.png",
    p2pQuery: "Ardshinbank",
    currencyGroup: "ardshinbank",
  },
  {
    id: "am-inecobank-usd-account",
    name: "Inecobank",
    country: "AM",
    currency: "USD",
    role: "both",
    kind: "bank",
    color: "#263f91",
    initials: "IN",
    iconUrl: "/icons/assets/inecobank.png",
    bankFeePercent: 0.75,
    p2pQuery: "Inecobank",
    currencyGroup: "inecobank",
  },
  {
    id: "am-inecobank",
    name: "Inecobank",
    country: "AM",
    currency: "AMD",
    role: "both",
    kind: "bank",
    color: "#263f91",
    initials: "IN",
    iconUrl: "/icons/assets/inecobank.png",
    bankFeePercent: 0.75,
    p2pQuery: "Inecobank",
    currencyGroup: "inecobank",
  },
  {
    id: "am-evocabank-usd-account",
    name: "Evocabank",
    country: "AM",
    currency: "USD",
    role: "both",
    kind: "bank",
    color: "#111827",
    initials: "EV",
    iconUrl: "/icons/assets/evocabank.png",
    p2pQuery: "Evocabank",
    currencyGroup: "evocabank",
  },
  {
    id: "am-evocabank",
    name: "Evocabank",
    country: "AM",
    currency: "AMD",
    role: "both",
    kind: "bank",
    color: "#111827",
    initials: "EV",
    iconUrl: "/icons/assets/evocabank.png",
    p2pQuery: "Evocabank",
    currencyGroup: "evocabank",
  },
  {
    id: "am-vtb-usd-account",
    name: "VTB Armenia",
    country: "AM",
    currency: "USD",
    role: "both",
    kind: "bank",
    color: "#0a52bd",
    initials: "VT",
    iconUrl: "/icons/assets/vtb.webp",
    p2pQuery: "VTB",
    currencyGroup: "vtb-armenia",
  },
  {
    id: "am-vtb",
    name: "VTB Armenia",
    country: "AM",
    currency: "AMD",
    role: "both",
    kind: "bank",
    color: "#0a52bd",
    initials: "VT",
    iconUrl: "/icons/assets/vtb.webp",
    p2pQuery: "VTB",
    currencyGroup: "vtb-armenia",
  },
  {
    id: "ru-sberbank",
    name: "Sberbank",
    country: "RU",
    currency: "RUB",
    role: "both",
    kind: "bank",
    color: "#21a038",
    initials: "SB",
    popular: true,
    iconUrl: "/icons/assets/sberbank.webp",
    p2pQuery: "Sberbank",
    currencyGroup: "sberbank",
  },
  {
    id: "ru-tbank",
    name: "T-Bank",
    country: "RU",
    currency: "RUB",
    role: "both",
    kind: "bank",
    color: "#ffdd2d",
    initials: "TB",
    popular: true,
    iconUrl: "/icons/assets/tbank.webp",
    p2pQuery: "T-Bank",
    currencyGroup: "tbank",
  },
  {
    id: "ru-tbank-usd-account",
    name: "T-Bank",
    country: "RU",
    currency: "USD",
    role: "both",
    kind: "bank",
    color: "#ffdd2d",
    initials: "TB",
    popular: true,
    iconUrl: "/icons/assets/tbank.webp",
    p2pQuery: "T-Bank",
    currencyGroup: "tbank",
  },
  {
    id: "ru-alfabank",
    name: "Alfa-Bank",
    country: "RU",
    currency: "RUB",
    role: "both",
    kind: "bank",
    color: "#ef3124",
    initials: "AB",
    popular: true,
    iconUrl: "/icons/assets/alfabank.webp",
    p2pQuery: "Alfa-Bank",
    currencyGroup: "alfabank",
  },
  {
    id: "ru-vtb",
    name: "VTB",
    country: "RU",
    currency: "RUB",
    role: "both",
    kind: "bank",
    color: "#0a52bd",
    initials: "VT",
    iconUrl: "/icons/assets/vtb.webp",
    p2pQuery: "VTB",
    currencyGroup: "vtb",
  },
  {
    id: "ru-gazprombank",
    name: "Gazprombank",
    country: "RU",
    currency: "RUB",
    role: "both",
    kind: "bank",
    color: "#006db7",
    initials: "GP",
    iconUrl: "/icons/assets/gazprombank.webp",
    p2pQuery: "Gazprombank",
  },
  {
    id: "ru-raiffeisen",
    name: "Raiffeisenbank",
    country: "RU",
    currency: "RUB",
    role: "both",
    kind: "bank",
    color: "#ffe500",
    initials: "RB",
    iconUrl: "/icons/assets/raiffeisenbank.webp",
    p2pQuery: "Raiffeisenbank",
  },
  {
    id: "ru-ozon",
    name: "Ozon Bank",
    country: "RU",
    currency: "RUB",
    role: "both",
    kind: "bank",
    color: "#005bff",
    initials: "OZ",
    iconUrl: "/icons/assets/ozonbank.webp",
    p2pQuery: "Ozon Bank",
  },
  {
    id: "by-belarusbank",
    name: "Belarusbank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#006b3f",
    initials: "BB",
    iconUrl: "/icons/assets/belarusbank.webp",
    popular: true,
    p2pQuery: "Belarusbank",
    currencyGroup: "belarusbank",
  },
  {
    id: "by-belagroprombank",
    name: "Belagroprombank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#f58220",
    initials: "BA",
    iconUrl: "/icons/assets/belagroprombank.png",
    popular: true,
    p2pQuery: "Belagroprombank",
    currencyGroup: "belagroprombank",
  },
  {
    id: "by-priorbank",
    name: "Priorbank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#ffed00",
    initials: "PB",
    iconUrl: "/icons/assets/priorbank.jpg",
    popular: true,
    p2pQuery: "Priorbank",
    currencyGroup: "priorbank",
  },
  {
    id: "by-belinvestbank",
    name: "Belinvestbank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#009b77",
    initials: "BI",
    iconUrl: "/icons/assets/belinvestbank.jpg",
    p2pQuery: "Belinvestbank",
    currencyGroup: "belinvestbank",
  },
  {
    id: "by-alfabank",
    name: "Alfa-Bank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#ef3124",
    initials: "AB",
    popular: true,
    p2pQuery: "Alfa-Bank Belarus",
    currencyGroup: "alfabank",
  },
  {
    id: "by-belgazprombank",
    name: "Belgazprombank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#0079c2",
    initials: "BG",
    iconUrl: "/icons/assets/belgazprombank.png",
    popular: true,
    p2pQuery: "Belgazprombank",
    currencyGroup: "belgazprombank",
  },
  {
    id: "by-sberbank",
    name: "Sberbank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#21a038",
    initials: "SB",
    iconUrl: "/icons/assets/sberbank.webp",
    popular: true,
    p2pQuery: "Sber Bank Belarus",
    currencyGroup: "sberbank",
  },
  {
    id: "by-belveb",
    name: "Bank BelVEB",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#006fb9",
    initials: "BV",
    iconUrl: "/icons/assets/belveb.jpg",
    p2pQuery: "Bank BelVEB",
    currencyGroup: "belveb",
  },
  {
    id: "by-mtbank",
    name: "MTBank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#004ea3",
    initials: "MT",
    iconUrl: "/icons/assets/mtbank.svg",
    popular: true,
    p2pQuery: "MTBank",
    currencyGroup: "mtbank",
  },
  {
    id: "by-vtb",
    name: "VTB",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#0a52bd",
    initials: "VT",
    iconUrl: "/icons/assets/vtb.webp",
    popular: true,
    p2pQuery: "VTB Belarus",
    currencyGroup: "vtb",
  },
  {
    id: "by-dabrabyt",
    name: "Bank Dabrabyt",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#00a651",
    initials: "DB",
    iconUrl: "/icons/assets/dabrabyt.png",
    popular: true,
    p2pQuery: "Bank Dabrabyt",
    currencyGroup: "dabrabyt",
  },
  {
    id: "by-technobank",
    name: "Technobank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#ed1c24",
    initials: "TB",
    iconUrl: "/icons/assets/technobank.png",
    popular: true,
    p2pQuery: "Technobank",
    currencyGroup: "technobank",
  },
  {
    id: "by-btk",
    name: "BTK Bank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#263f91",
    initials: "BT",
    iconUrl: "/icons/assets/btk.png",
    p2pQuery: "BTK Bank",
    currencyGroup: "btk",
  },
  {
    id: "by-bnb",
    name: "BNB Bank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#e31e24",
    initials: "BN",
    iconUrl: "/icons/assets/bnb-bank.jpg",
    p2pQuery: "BNB Bank",
    currencyGroup: "bnb-bank",
  },
  {
    id: "by-bsb",
    name: "BSB Bank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#e30613",
    initials: "BS",
    iconUrl: "/icons/assets/bsb.webp",
    p2pQuery: "BSB Bank",
    currencyGroup: "bsb-bank",
  },
  {
    id: "by-paritetbank",
    name: "Paritetbank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#0083ca",
    initials: "PA",
    iconUrl: "/icons/assets/paritetbank.jpg",
    popular: true,
    p2pQuery: "Paritetbank",
    currencyGroup: "paritetbank",
  },
  {
    id: "by-bank-reshenie",
    name: "Bank Reshenie",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#6b2d90",
    initials: "BR",
    iconUrl: "/icons/assets/bank-reshenie.webp",
    popular: true,
    p2pQuery: "Bank Reshenie",
    currencyGroup: "bank-reshenie",
  },
  {
    id: "by-statusbank",
    name: "StatusBank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#003b71",
    initials: "ST",
    iconUrl: "/icons/assets/statusbank.png",
    p2pQuery: "StatusBank",
    currencyGroup: "statusbank",
  },
  {
    id: "by-neobank",
    name: "Neo Bank Asia",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#ff5a1f",
    initials: "NE",
    iconUrl: "/icons/assets/neobank.png",
    p2pQuery: "Neo Bank Asia",
    currencyGroup: "neo-bank",
  },
  {
    id: "by-zepterbank",
    name: "Zepter Bank",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#8b1e3f",
    initials: "ZE",
    p2pQuery: "Zepter Bank",
    currencyGroup: "zepterbank",
  },
  {
    id: "by-brrb",
    name: "Bank of Growth and Business Development",
    country: "BY",
    currency: "BYN",
    role: "both",
    kind: "bank",
    color: "#005ca9",
    initials: "BR",
    p2pQuery: "BRRB Bank",
    currencyGroup: "brrb",
  },
];

/** Kept in the same order as the backend intermediary catalog. */
export const CRYPTO_ASSETS = [
  ["USDT", "Tether", "#26a17b"],
  ["USDC", "USD Coin", "#2775ca"],
  ["BTC", "Bitcoin", "#f7931a"],
  ["ETH", "Ethereum", "#627eea"],
  ["BNB", "BNB", "#f3ba2f"],
  ["SOL", "Solana", "#14f195"],
  ["TRX", "TRON", "#ef0027"],
  ["TON", "Toncoin", "#0098ea"],
  ["DOGE", "Dogecoin", "#c2a633"],
  ["LTC", "Litecoin", "#345d9d"],
  ["DAI", "Dai", "#f5ac37"],
  ["FDUSD", "First Digital USD", "#1d1d1d"],
  ["XRP", "XRP", "#23292f"],
  ["ADA", "Cardano", "#0033ad"],
  ["DOT", "Polkadot", "#e6007a"],
  ["LINK", "Chainlink", "#2a5ada"],
  ["AVAX", "Avalanche", "#e84142"],
  ["MATIC", "Polygon", "#8247e5"],
  ["BCH", "Bitcoin Cash", "#8dc351"],
  ["NEAR", "NEAR Protocol", "#111827"],
  ["APT", "Aptos", "#111827"],
  ["ATOM", "Cosmos", "#2e3148"],
  ["UNI", "Uniswap", "#ff007a"],
  ["SUI", "Sui", "#6fbcf0"],
] as const;

type CryptoAssetCode = (typeof CRYPTO_ASSETS)[number][0];

/** Crypto assets available on both sides of an exchange. */
export const DIGITAL_ASSETS: PaymentMethod[] = CRYPTO_ASSETS.map(
  ([currency, name, color]) => ({
    id: `global-${currency.toLowerCase()}`,
    name,
    country: "GLOBAL",
    currency,
    role: "both",
    kind: "wallet",
    color,
    initials: currency,
    p2pQuery: currency,
    iconUrl: assetIcon(currency),
  }),
);

export function paymentCountry(code: string, currency?: string): PaymentCountry | undefined {
  return PAYMENT_COUNTRIES.find(
    (country) => country.code === code && (!currency || country.currency === currency),
  );
}

export function paymentMethodsFor(
  country: string,
  currency: string,
  role: Exclude<PaymentMethodRole, "both">,
): PaymentMethod[] {
  return PAYMENT_METHODS.filter(
    (method) =>
      method.country === country &&
      method.currency === currency &&
      (method.role === role || method.role === "both"),
  );
}

export function paymentMethodFavicon(method: PaymentMethod | null | undefined): string | null {
  if (!method) return null;
  if (method.kind === "wallet") return assetIcon(method.currency);
  // External bank logos are deliberately ignored. Bank initials are rendered
  // by the picker, so a remote icon can never add a third-party request.
  return method.iconUrl?.startsWith("/") || method.iconUrl?.startsWith("data:")
    ? method.iconUrl
    : null;
}
