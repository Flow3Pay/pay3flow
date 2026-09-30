import { apiUrl } from "$lib/api";
import { anonymousHeaders } from "$lib/anonymous-user";

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
    method_id?: string;
    name: string;
    display_name: string;
    role: PaymentMethodRole;
    country: string;
    currency: string;
    kind: PaymentMethodKind;
    color: string;
    initials: string;
    popular: boolean;
    icon_url: string;
    bank_fee_percent?: number;
    p2p_query: string;
    currency_group?: string;
  }>;
}

/** Load the complete card catalog generated from backend Providerfiles. */
export async function fetchPaymentMethods(): Promise<PaymentMethod[]> {
  const requestInit = { headers: anonymousHeaders({ Accept: "application/json" }) };
  let response = await fetch(apiUrl("/api/banks?picker_visible=true&limit=100"), requestInit);
  let unfilteredFallback = false;
  if (response.status === 400) {
    // Rolling deployments can briefly serve a backend from before the
    // picker_visible filter existed. Its rows still carry method_id, so page
    // through that directory and filter the catalog client-side.
    unfilteredFallback = true;
    response = await fetch(apiUrl("/api/banks?limit=100&offset=0"), requestInit);
  }
  if (!response.ok) throw new Error(await catalogError(response));

  const firstPage = await response.json() as PaymentMethodDirectoryPage;
  const items = [...firstPage.items];
  if (unfilteredFallback) {
    const pageSize = Math.max(1, firstPage.limit || 100);
    for (let offset = firstPage.offset + pageSize; offset < firstPage.total; offset += pageSize) {
      const nextResponse = await fetch(apiUrl(`/api/banks?limit=100&offset=${offset}`), requestInit);
      if (!nextResponse.ok) throw new Error(await catalogError(nextResponse));
      items.push(...((await nextResponse.json()) as PaymentMethodDirectoryPage).items);
    }
  }

  return items.flatMap((item) => item.method_id ? [{
    id: item.method_id,
    name: item.display_name || item.name,
    country: item.country,
    currency: item.currency,
    role: item.role,
    kind: item.kind,
    color: item.color,
    initials: item.initials,
    popular: item.popular || undefined,
    iconUrl: localIconUrl(item.icon_url),
    bankFeePercent: item.bank_fee_percent,
    p2pQuery: item.p2p_query || item.display_name || item.name,
    currencyGroup: item.currency_group,
  }] : []);
}

async function catalogError(response: Response): Promise<string> {
  const detail = (await response.text()).trim();
  return `payment-method catalog failed (${response.status})${detail ? `: ${detail}` : ""}`;
}

function localIconUrl(value: string): string | undefined {
  return value.startsWith("/") || value.startsWith("data:") ? value : undefined;
}

export function paymentMethodFavicon(method: PaymentMethod | null | undefined): string | null {
  return method?.iconUrl ?? null;
}
