import { apiUrl } from "$lib/api";
import { anonymousHeaders } from "$lib/anonymous-user";
import { canonicalAssetTicker } from "./asset-identity";

export type PaymentMethodRole = "sender" | "recipient" | "both";
export type PaymentMethodKind = "bank" | "cash" | "currency" | "wallet";

/** A backend-owned option rendered by the shared picker card. */
export interface PaymentMethod {
  id: string;
  name: string;
  country: string;
  currency: string;
  role: PaymentMethodRole;
  kind: PaymentMethodKind;
  color: string;
  initials: string;
  popular?: boolean;
  iconUrl?: string;
  bankFeePercent?: number;
  p2pQuery: string;
  currencyGroup?: string;
}

interface PaymentMethodDirectoryPage {
  total: number;
  limit: number;
  offset: number;
  items: Array<{
    id?: string;
    method_id?: string;
    name: string;
    display_name: string;
    role: PaymentMethodRole | "receiver";
    country: string;
    currency: string;
    kind: PaymentMethodKind;
    color: string;
    initials: string;
    popular: boolean;
    icon_url?: string | null;
    bank_fee_percent?: number;
    p2p_query: string;
    currency_group?: string;
  }>;
}

/** Load the complete card catalog generated from backend Providerfiles. */
export async function fetchPaymentMethods(): Promise<PaymentMethod[]> {
  const requestInit = { headers: anonymousHeaders({ Accept: "application/json" }) };
  const response = await fetch(apiUrl("/api/banks?limit=100"), requestInit);
  if (!response.ok) throw new Error(await catalogError(response));

  const firstPage = await response.json() as PaymentMethodDirectoryPage;
  const items = [...firstPage.items];
  const pageSize = Math.max(1, firstPage.limit || 100);
  for (let offset = firstPage.offset + pageSize; offset < firstPage.total; offset += pageSize) {
    const params = new URLSearchParams({ limit: String(pageSize), offset: String(offset) });
    const nextResponse = await fetch(apiUrl(`/api/banks?${params}`), requestInit);
    if (!nextResponse.ok) throw new Error(await catalogError(nextResponse));
    items.push(...((await nextResponse.json()) as PaymentMethodDirectoryPage).items);
  }

  return items.flatMap((item) => item.method_id || item.id ? [{
    id: item.method_id ?? `directory-${item.id}`,
    name: item.kind === "wallet" && item.currency === "TON" ? "Gram" : item.display_name || item.name,
    country: item.country,
    currency: item.kind === "wallet" ? canonicalAssetTicker(item.currency) : item.currency,
    role: item.role === "receiver" ? "recipient" : item.role,
    kind: item.kind,
    color: item.color || "#272727",
    initials: item.kind === "wallet" && item.currency === "TON" ? "GRAM" : item.initials || item.currency,
    popular: item.popular || undefined,
    iconUrl: item.kind === "wallet" && ["TON", "GRAM"].includes(item.currency) ? "/icons/assets/gram.png" : localIconUrl(item.icon_url),
    bankFeePercent: item.bank_fee_percent,
    p2pQuery: item.name === "SBP Fast Payments" ? "SBP" : item.p2p_query || item.display_name || item.name,
    currencyGroup: item.currency_group,
  }] : []);
}

async function catalogError(response: Response): Promise<string> {
  const detail = (await response.text()).trim();
  return `payment-method catalog failed (${response.status})${detail ? `: ${detail}` : ""}`;
}

function localIconUrl(value?: string | null): string | undefined {
  return value && (value.startsWith("/") || value.startsWith("data:")) ? value : undefined;
}

export function paymentMethodFavicon(method: PaymentMethod | null | undefined): string | null {
  return method?.iconUrl ?? null;
}
