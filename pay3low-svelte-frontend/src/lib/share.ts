export type ShareState = { source: string; target: string; amount: string; receive: string };

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
  return url;
}
