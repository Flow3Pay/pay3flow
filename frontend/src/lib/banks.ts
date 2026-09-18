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

/**
 * Fetch a paged slice of the backend bank directory. No auth: the directory is
 * the source of truth for the payment form's "sending → receiving" pickers.
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