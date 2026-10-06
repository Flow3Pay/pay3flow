export type ShareState = { source: string; target: string; amount: string; receive: string; searchId?: string; routeId?: string; sourceMethodId?: string; targetMethodId?: string; sourceNetworkId?: string; targetNetworkId?: string; sources?: string[]; exchangeMethods?: string[]; assets?: string[] };
export const SHARE_IMAGE_VERSION = "dark-3";
export const SHARE_SEARCH_ID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function shareCurrency(value: string): string | null {
  const currency = value.toUpperCase();
  return /^[A-Z0-9]{2,12}$/.test(currency) ? currency : null;
}

export function shareAmount(value: string | null): string | null {
  if (!value || value.length > 32 || !/^(?:0|[1-9]\d{0,13})(?:\.\d{1,12})?$/.test(value)) return null;
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? value : null;
}

export function shareUrl(origin: string, state: ShareState): URL {
  const url = new URL(`/swap/${encodeURIComponent(state.source)}/${encodeURIComponent(state.target)}`, origin);
  const amount = shareAmount(state.amount.replace(",", "."));
  const receive = shareAmount(state.receive.replace(",", "."));
  if (amount) url.searchParams.set("amount", amount);
  if (receive) url.searchParams.set("receive", receive);
  if (state.sourceMethodId) url.searchParams.set("sm", state.sourceMethodId);
  if (state.targetMethodId) url.searchParams.set("tm", state.targetMethodId);
  if (state.sourceNetworkId) url.searchParams.set("sn", state.sourceNetworkId);
  if (state.targetNetworkId) url.searchParams.set("tn", state.targetNetworkId);
  if (state.sources?.length) url.searchParams.set("sources", state.sources.join(","));
  if (state.exchangeMethods?.length) url.searchParams.set("modes", state.exchangeMethods.join(","));
  if (state.assets?.length) url.searchParams.set("assets", state.assets.join(","));
  if (receive && state.searchId && SHARE_SEARCH_ID_PATTERN.test(state.searchId)) {
    url.searchParams.set("search", state.searchId);
    if (state.routeId) url.searchParams.set("route", state.routeId);
  }
  return url;
}
