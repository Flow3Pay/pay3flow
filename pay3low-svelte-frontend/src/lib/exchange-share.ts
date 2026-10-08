export type ExchangeShareState = {
  source: string;
  target: string;
  amount: string;
  receive: string;
  from: string;
  to: string;
  fromName: string;
  toName: string;
  fromNetwork: string;
  toNetwork: string;
  sources: string[];
  methods: string[];
  assets: string[];
};

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
  return `${origin}/#/swap/${encodeURIComponent(state.source)}/${encodeURIComponent(state.target)}?${params}`;
}
