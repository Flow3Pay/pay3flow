import type { P2pOffer, RouteCandidate } from './exchange';
import { canonicalAssetTicker } from './asset-identity.ts';

const identifier = /^[a-z0-9._@:-]{1,100}$/i;
export const GUIDE_PARAMETERS = ['amount', 'from', 'to', 'fromNetwork', 'toNetwork', 'sources', 'methods', 'assets', 'path', 'venues', 'lang', 'view'] as const;
export function guideParameters(input: URLSearchParams) {
  const output = new URLSearchParams();
  for (const key of GUIDE_PARAMETERS) {
    const value = input.get(key);
    if (key === 'view' && value !== 'swap' && value !== 'guide') continue;
    if (value && value.length <= (key === 'path' ? 2500 : 500)) output.set(key, value);
  }
  return output;
}

// Rewrite only asset-bearing fields in the saved operation signature. Provider,
// advertiser and network identifiers may also contain TON and must stay intact.
function canonicalRoutePath(value: string, source: string, target: string): string {
  try {
    const path: unknown = JSON.parse(value);
    if (!Array.isArray(path) || path.length < 9) return value;
    const symbols = new Set<string>([source.toUpperCase(), target.toUpperCase()]);
    const remember = (asset: unknown) => { if (typeof asset === 'string') symbols.add(asset.split('@')[0].toUpperCase()); };
    remember(path[4]);
    for (const index of [0, 1]) if (Array.isArray(path[index])) { remember(path[index][2]); remember(path[index][3]); }
    if (Array.isArray(path[6])) path[6].forEach(remember);
    if (Array.isArray(path[9])) for (const leg of path[9]) if (Array.isArray(leg)) { remember(leg[1]); remember(leg[2]); }
    const asset = (value: unknown) => {
      if (typeof value !== 'string') return value;
      const at = value.indexOf('@');
      return at < 0 ? canonicalAssetTicker(value) : `${canonicalAssetTicker(value.slice(0, at))}${value.slice(at)}`;
    };
    const pair = (value: unknown) => {
      if (typeof value !== 'string') return value;
      for (const other of symbols) {
        if (other === 'TON') continue;
        if (value === `TON${other}`) return `GRAM${other}`;
        if (value === `${other}TON`) return `${other}GRAM`;
      }
      return value;
    };
    for (const index of [0, 1]) if (Array.isArray(path[index])) { path[index][2] = asset(path[index][2]); path[index][3] = asset(path[index][3]); }
    path[4] = asset(path[4]);
    if (Array.isArray(path[6])) path[6] = path[6].map(asset);
    if (Array.isArray(path[8])) { path[8][1] = pair(path[8][1]); path[8][2] = pair(path[8][2]); }
    if (Array.isArray(path[9])) for (const leg of path[9]) if (Array.isArray(leg)) { leg[1] = asset(leg[1]); leg[2] = asset(leg[2]); leg[3] = pair(leg[3]); }
    return JSON.stringify(path);
  } catch { return value; }
}

export function parseExchangeHash(hash: string) {
  const match = hash.match(/^#\/(swap|guide)\/([a-z0-9._-]{1,20})\/([a-z0-9._-]{1,20})(?:\?([^#]*))?$/i);
  if (!match) return null;
  const params = guideParameters(new URLSearchParams(match[4] ?? ''));
  const path = params.get('path');
  if (path) params.set('path', canonicalRoutePath(path, match[2], match[3]));
  const amount = params.get('amount');
  return { guide: match[1].toLowerCase() === 'guide', sourceCurrency: canonicalAssetTicker(match[2]), targetCurrency: canonicalAssetTicker(match[3]), amount: amount && /^\d+(?:[.,]\d+)?$/.test(amount) && Number(amount.replace(',', '.')) > 0 ? amount : null, params };
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
