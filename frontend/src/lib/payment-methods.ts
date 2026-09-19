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
  kind: "bank" | "wallet";
  color: string;
  initials: string;
  popular?: boolean;
  /** Name fragment understood by the public P2P venue filters. */
  p2pQuery: string;
}

/**
 * Frontend-owned payment-method directory.
 *
 * Add a country once, then append its banks below. The picker and search UI
 * derive their sections from this data, so adding another method does not
 * require changing the component.
 */
export const PAYMENT_COUNTRIES: PaymentCountry[] = [
  { code: "AM", name: "Armenia", currency: "AMD", mark: "AM" },
  { code: "RU", name: "Russia", currency: "RUB", mark: "RU" },
];

export const PAYMENT_METHODS: PaymentMethod[] = [
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
    p2pQuery: "Ameriabank",
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
    p2pQuery: "IDBank",
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
    p2pQuery: "ACBA",
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
    p2pQuery: "Ardshinbank",
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
    p2pQuery: "Inecobank",
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
    p2pQuery: "Evocabank",
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
    p2pQuery: "VTB",
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
    p2pQuery: "Sberbank",
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
    p2pQuery: "T-Bank",
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
    p2pQuery: "Alfa-Bank",
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
    p2pQuery: "VTB",
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
    p2pQuery: "Ozon Bank",
  },
];

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

const PAYMENT_METHOD_DOMAINS: Record<string, string> = {
  "am-ameriabank": "ameriabank.am",
  "am-idbank": "idbank.am",
  "am-acba": "acba.am",
  "am-ardshinbank": "ardshinbank.am",
  "am-inecobank": "inecobank.am",
  "am-evocabank": "evoca.am",
  "am-vtb": "vtb.am",
  "ru-sberbank": "sber-bank.by",
  "ru-tbank": "tbank.ru",
  "ru-alfabank": "alfabank.by",
  "ru-vtb": "vtb.by",
  "ru-gazprombank": "www.gazprombank.ru",
  "ru-raiffeisen": "raiffeisen.ru",
  "ru-ozon": "finance.ozon.ru",
};

export function paymentMethodFavicon(method: PaymentMethod | null | undefined): string | null {
  const domain = method ? PAYMENT_METHOD_DOMAINS[method.id] : undefined;
  return domain ? `https://${domain}/favicon.ico` : null;
}
