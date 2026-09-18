import { API_BASE_URL } from "./api";

/* -------------------------------------------------------------------------- */
/*  Types — mirrors the Rust `ExchangePair` row (PLAN 2△ / 46c).               */
/* -------------------------------------------------------------------------- */

export interface ExchangePair {
  id: string;
  from_scheme: string;
  from_bank: string;
  from_bank_icon_url: string;
  to_scheme: string;
  to_bank: string;
  to_bank_icon_url: string;
  /** Receiving country (where the money lands). */
  country: string;
  /** Comma-separated `from_currency,to_currency` (e.g. `RUB,EUR`). */
  currencies: string;
  status: string;
  daily_limit_minor?: number | null;
  daily_limit_currency?: string | null;
  updated_at: string;
}

export interface PairFilters {
  country?: string;
  /** Either leg of the pair's currencies. */
  currency?: string;
  /** Either leg's scheme. */
  scheme?: string;
  from_scheme?: string;
  to_scheme?: string;
}

/** Split the pair's `currencies` field into `[from, to]`. */
export function pairCurrencies(pair: ExchangePair): [string, string] {
  const [from = "", to = ""] = pair.currencies
    .split(",")
    .map((c) => c.trim())
    .filter(Boolean);
  return [from, to];
}

/**
 * Fetch the backend's bank exchange-pair catalog. No auth: the catalog is the
 * source of truth for the payment form's "from → to" picker. Returns only
 * `enabled` pairs.
 */
export async function fetchExchangePairs(
  filters: PairFilters = {},
): Promise<ExchangePair[]> {
  const url = new URL("/api/exchange-pairs", API_BASE_URL);
  for (const [key, value] of Object.entries(filters)) {
    if (value) url.searchParams.set(key, value);
  }
  const response = await fetch(url, { headers: { Accept: "application/json" } });
  if (!response.ok) {
    throw new Error(`exchange pairs request failed: ${response.status}`);
  }
  return (await response.json()) as ExchangePair[];
}
