import type { P2pOffer, RouteCandidate } from './exchange';

const identifier = /^[a-z0-9._@:-]{1,100}$/i;
export const GUIDE_PARAMETERS = ['amount', 'from', 'to', 'fromNetwork', 'toNetwork', 'sources', 'methods', 'assets', 'path', 'venues', 'lang'] as const;
export function guideParameters(input: URLSearchParams) {
  const output = new URLSearchParams();
  for (const key of GUIDE_PARAMETERS) {
    const value = input.get(key);
    if (value && value.length <= (key === 'path' ? 2500 : 500)) output.set(key, value);
  }
  return output;
}
export function parseExchangeHash(hash: string) {
  const match = hash.match(/^#\/(swap|guide)\/([a-z0-9._-]{1,20})\/([a-z0-9._-]{1,20})(?:\?([^#]*))?$/i);
  if (!match) return null;
  const params = guideParameters(new URLSearchParams(match[4] ?? ''));
  const amount = params.get('amount');
  return { guide: match[1].toLowerCase() === 'guide', sourceCurrency: match[2].toUpperCase(), targetCurrency: match[3].toUpperCase(), amount: amount && /^\d+(?:[.,]\d+)?$/.test(amount) && Number(amount.replace(',', '.')) > 0 ? amount : null, params };
}
// Describe operations, not a short-lived quote, order or tracking token.
export function guideRoutePath(route: RouteCandidate): string {
  const advertiser = (offer?: P2pOffer) => offer ? [offer.source, offer.advertiser.id ?? offer.advertiser.nickname, offer.fiat, offer.asset, offer.network ?? ""] : null;
  return JSON.stringify([advertiser(route.entry_offer_snapshot), advertiser(route.exit_offer_snapshot), route.route_kind ?? '', route.route_provider ?? '', route.entry_asset, route.entry_network ?? '', route.route_path ?? [], route.legs.map(leg => [leg.kind, leg.provider]), route.market_path ? [route.market_path.venue, route.market_path.source_pair, route.market_path.target_pair] : null, route.cycle_legs?.map(leg => [leg.provider, leg.from_asset, leg.to_asset, leg.market_pair ?? '']) ?? []]);
}
export function guideHash(source: string, target: string, params: URLSearchParams) {
  return `#/guide/${encodeURIComponent(source)}/${encodeURIComponent(target)}?${guideParameters(params)}`;
}
export function guideSharePath(hash: string) {
  const shared = parseExchangeHash(hash);
  return shared?.guide ? `/share/guide/${shared.sourceCurrency}/${shared.targetCurrency}?${shared.params}` : '/';
}
export function sharedIdentifiers(params: URLSearchParams, key: string) {
  return (params.get(key) ?? '').split(',').filter(value => identifier.test(value));
}
