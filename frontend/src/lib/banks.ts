import { API_BASE_URL } from "./api";

export interface Bank {
  id: number;
  name: string;
  role: "sender" | "receiver" | "both";
  country: string;
  currency: string;
  domain: string;
  icon_url: string;
  schemes: string[];
  status: string;
  updated_at?: string;
}

export interface BankPage {
  items: Bank[];
  total: number;
  limit: number;
  offset: number;
}

export interface BankFilters {
  limit?: number;
  offset?: number;
  q?: string;
  role?: string;
  country?: string;
  currency?: string;
  scheme?: string;
  signal?: AbortSignal;
}

const SCHEME_ICONS: Record<string, string> = {
  mastercard:
    "https://thumb.wikimedia.org/wikipedia/commons/thumb/a/a4/Mastercard_2019_logo.svg/1280px-Mastercard_2019_logo.svg.png?utm_source=en.wikipedia.org&utm_campaign=index&utm_content=thumbnail",
  mir: "https://evgenykatyshev.ru/projects/mir-logo/mir-logo.svg",
  paypal:
    "https://thumb.wikimedia.org/wikipedia/commons/thumb/0/0e/PayPal_2024_%28Icon%29.svg/250px-PayPal_2024_%28Icon%29.svg.png?utm_source=en.wikipedia.org&utm_campaign=parser&utm_content=thumbnail",
  visa: "https://upload.wikimedia.org/wikipedia/commons/9/98/Visa_Inc._logo_%282005%E2%80%932014%29.svg?utm_source=commons.wikimedia.org&utm_campaign=index&utm_content=original",
};

function normalizeScheme(value: string): string {
  return value.trim().toLowerCase().replace(/\s+/g, "");
}

export function schemeIconUrl(scheme: string): string | undefined {
  return SCHEME_ICONS[normalizeScheme(scheme)];
}

function methodName(name: string, scheme: string): string {
  return normalizeScheme(name).endsWith(normalizeScheme(scheme)) ? name : `${name} ${scheme}`;
}

export function paymentMethodBaseName(name: string, scheme?: string): string {
  if (!scheme) return name;
  const suffix = ` ${scheme}`;
  return name.endsWith(suffix) ? name.slice(0, -suffix.length) : name;
}

export function paymentMethodVariants(bank: Bank): Bank[] {
  if (bank.schemes.length <= 1) return [bank];
  return bank.schemes.map((scheme) => ({
    ...bank,
    name: methodName(bank.name, scheme),
    schemes: [scheme],
  }));
}

export function uniquePaymentMethods(banks: Bank[]): Bank[] {
  const seen = new Set<string>();
  return banks.filter((bank) => {
    const key = `${normalizeScheme(bank.name)}:${normalizeScheme(bank.schemes[0] ?? "")}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

/**
 * Fetch a paged slice of the backend payment-method directory. No auth: the
 * directory is the source of truth for the swap form's sell/buy method pickers.
 */
export async function fetchBanks(filters: BankFilters = {}): Promise<BankPage> {
  const { signal, ...params } = filters;
  const url = new URL("/api/banks", API_BASE_URL);
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== "") url.searchParams.set(key, String(value));
  }
  const response = await fetch(url, {
    headers: { Accept: "application/json" },
    signal,
  });
  if (!response.ok) {
    throw new Error(`banks request failed: ${response.status}`);
  }
  return (await response.json()) as BankPage;
}
