import { apiUrl } from "$lib/api";

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
  const response = await fetch(apiUrl("/api/banks?picker_visible=true&limit=100"));
  if (!response.ok) throw new Error(`payment-method catalog failed (${response.status})`);
  const page = await response.json() as PaymentMethodDirectoryPage;
  return page.items.flatMap((item) => item.method_id ? [{
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

function localIconUrl(value: string): string | undefined {
  return value.startsWith("/") || value.startsWith("data:") ? value : undefined;
}

export function paymentMethodFavicon(method: PaymentMethod | null | undefined): string | null {
  return method?.iconUrl ?? null;
}
