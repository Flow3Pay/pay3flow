import { guideParameters } from "./guide-link";

export type ExchangeShareState = {
  source: string;
  target: string;
  amount: string;
  receive: string;
  from: string;
  to: string;
  fromName: string;
  toName: string;
  fromIcon?: string;
  toIcon?: string;
  fromNetworkName?: string;
  toNetworkName?: string;
  fromNetwork: string;
  toNetwork: string;
  sources: string[];
  methods: string[];
  assets: string[];
};

const previewParameters = ['fromName', 'toName', 'fromIcon', 'toIcon', 'fromNetworkName', 'toNetworkName'] as const;
function previewValue(key: string, value: string | null | undefined) {
  if (!value || value.length > 100) return null;
  if (key.endsWith('Icon')) return /^\/icons\/assets\/[a-z0-9_-]+\.(?:png|webp|svg|jpg)$/i.test(value) ? value : null;
  return value.replace(/[\u0000-\u001f]/g, '').trim() || null;
}

export function exchangeShareUrl(origin: string, state: ExchangeShareState, language: string) {
  const params = new URLSearchParams();
  const amount = state.amount.replace(',', '.');
  if (/^\d+(?:\.\d+)?$/.test(amount) && Number(amount) > 0) params.set('amount', amount);
  for (const key of ['from', 'to', 'fromNetwork', 'toNetwork'] as const) {
    if (state[key]) params.set(key, state[key]);
  }
  for (const key of ['sources', 'methods', 'assets'] as const) {
    if (state[key].length) params.set(key, state[key].join(','));
  }
  params.set('lang', language);
  for (const key of previewParameters) {
    const value = previewValue(key, state[key]);
    if (value) params.set(key, value);
  }
  const receive = shareAmount(state.receive);
  if (receive) params.set('receive', receive);
  return `${origin}/swap/${encodeURIComponent(state.source)}/${encodeURIComponent(state.target)}?${params}`;
}

export function shareCurrency(value: string) {
  return /^[a-z0-9._-]{1,20}$/i.test(value) ? value.toUpperCase() : null;
}
export function shareAmount(value: string | null) {
  const normalized = value?.replace(',', '.');
  return normalized && normalized.length <= 30 && /^\d+(?:\.\d+)?$/.test(normalized) && Number(normalized) > 0 ? normalized : null;
}
export function exchangeShareParameters(input: URLSearchParams) {
  const normalized = new URLSearchParams(input);
  for (const [oldKey, key] of [['sm', 'from'], ['tm', 'to'], ['sn', 'fromNetwork'], ['tn', 'toNetwork'], ['modes', 'methods']]) {
    if (!normalized.has(key) && normalized.has(oldKey)) normalized.set(key, normalized.get(oldKey)!);
  }
  const params = guideParameters(normalized);
  params.delete('path'); params.delete('venues');
  const amount = shareAmount(params.get('amount'));
  if (amount) params.set('amount', amount); else params.delete('amount');
  const receive = shareAmount(input.get('receive'));
  if (receive) params.set('receive', receive);
  for (const key of previewParameters) {
    const value = previewValue(key, input.get(key));
    if (value) params.set(key, value);
  }
  return params;
}
export function exchangeShareImage(origin: string, source: string, target: string, params: URLSearchParams) {
  const image = new URL('/share-image.png', origin);
  image.searchParams.set('v', 'bridge-6');
  image.searchParams.set('from', source); image.searchParams.set('to', target);
  for (const key of ['amount', 'receive']) {
    const value = shareAmount(params.get(key));
    if (value) image.searchParams.set(key, value);
  }
  for (const key of previewParameters) {
    const value = previewValue(key, params.get(key));
    if (value) image.searchParams.set(key, value);
  }
  for (const key of ['fromNetwork', 'toNetwork']) {
    const value = previewValue(key, params.get(key));
    if (value) image.searchParams.set(key, value);
  }
  return image.toString();
}
