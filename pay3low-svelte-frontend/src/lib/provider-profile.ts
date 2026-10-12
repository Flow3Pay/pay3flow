import { apiUrl } from './api';
import type { ProviderDefinition } from './exchange';

export type ProfilePeriod = '7d' | '30d' | '90d';
export type MarketType = 'p2p' | 'spot' | 'exchanger';
export interface ProfileDay { started_at: string; searches: number; top10: number; top1: number }
export interface ProfileDirection { source_currency: string; target_currency: string; searches: number; top10: number; best_rank: number | null }
export interface ProviderStatistics {
  slug: string; period_days: number; searches: number; top10: number; top1: number;
  average_rank: number | null; response_samples: number; successful_responses: number;
  average_response_ms: number | null; site_opens: number; first_seen: string | null;
  updated_at: string; days: ProfileDay[]; directions: ProfileDirection[];
}
export interface ProfileVenue {
  slug: string; name: string; url: string; types: MarketType[]; currencies: string[];
  banks: string[]; searchable: boolean | null; guidance: ProviderDefinition['guidance'];
  feeModel: ProviderDefinition['fee_model'];
}

// Offline identity only; numerical statistics always come from the API.
const identities: Array<[string, string, string, MarketType[]]> = [
  ['binance', 'Binance', 'https://www.binance.com', ['p2p', 'spot']],
  ['bybit', 'Bybit', 'https://www.bybit.com', ['p2p', 'spot']],
  ['okx', 'OKX', 'https://www.okx.com', ['p2p', 'spot']],
  ['bitget', 'Bitget', 'https://www.bitget.com', ['p2p', 'spot']],
  ['mexc', 'MEXC', 'https://www.mexc.com', ['p2p', 'spot']],
  ['rapira', 'Rapira', 'https://rapira.net', ['p2p']],
  ['whitebird', 'Whitebird', 'https://whitebird.io', ['exchanger']],
  ['cifra-broker', 'Cifra Markets', 'https://cifra.by', ['exchanger']],
  ['skylabs', 'SkyLabs', 'https://skylabs.world', ['exchanger']],
  ['bncex', 'bncex', 'https://bncex.com', ['exchanger']],
  ['bitcoin-center', 'Bitcoin Center', 'https://www.bitcoincenter.am', ['exchanger']],
  ['bestchange', 'BestChange', 'https://www.bestchange.com', ['exchanger']],
  ['dzengi', 'Dzengi', 'https://dzengi.com', ['spot']],
  ['cow-swap', 'CoW Swap', 'https://swap.cow.fi', ['exchanger']],
  ['near-intents', 'NEAR Intents', 'https://app.near-intents.org', ['exchanger']],
  ['symbiosis', 'Symbiosis', 'https://app.symbiosis.finance', ['exchanger']],
  ['id-pay', 'ID Pay', 'https://id-pay.ru', ['exchanger']],
  ['papa-change', 'Papa Change', 'https://papa-change.biz', ['exchanger']],
];
export const profileVenues: ProfileVenue[] = identities.map(([slug, name, url, types]) => ({
  slug, name, url, types, currencies: [], banks: [], searchable: null, guidance: null, feeModel: null,
}));
export function fallbackVenue(slug: string): ProfileVenue {
  return profileVenues.find(venue => venue.slug === slug) ?? {
    slug, name: slug, url: '', types: [], currencies: [], banks: [], searchable: null, guidance: null, feeModel: null,
  };
}
export function venuesFromCatalog(catalog: ProviderDefinition[]): ProfileVenue[] {
  const groups = new Map<string, ProviderDefinition[]>();
  for (const definition of catalog) {
    groups.set(definition.slug, [...(groups.get(definition.slug) ?? []), definition]);
  }
  return [...groups.entries()].map(([slug, definitions]) => {
    const first = definitions[0];
    const fallback = fallbackVenue(slug);
    const declared = definitions.flatMap(definition => definition.market_types ?? []);
    return {
      slug, name: first.name.replace(/\s+(buy|sell)$/i, '').trim(),
      url: first.guidance?.links.find(link => /open .*webapp/i.test(link.label))?.url ?? first.source_url,
      types: [...new Set(declared.length ? declared : fallback.types.length ? fallback.types : definitions.flatMap(definition => definition.exchange_methods))] as MarketType[],
      currencies: [...new Set(definitions.flatMap(definition => definition.currencies))].filter(value => value.toUpperCase() !== 'ALL'),
      banks: [...new Set(definitions.flatMap(definition => definition.banks))],
      searchable: definitions.some(definition => definition.searchable),
      guidance: definitions.find(definition => definition.guidance)?.guidance,
      feeModel: definitions.find(definition => definition.fee_model)?.fee_model,
    };
  }).sort((a, b) => a.name.localeCompare(b.name));
}
export async function fetchProviderStatistics(slug: string, period: ProfilePeriod, signal?: AbortSignal): Promise<ProviderStatistics> {
  const url = apiUrl(`/api/providers/${encodeURIComponent(slug)}/statistics`);
  url.searchParams.set('period', period);
  const response = await fetch(url, { signal });
  if (!response.ok) throw new Error(`Provider statistics request failed: ${response.status}`);
  return response.json();
}
export function percent(count: number, total: number): number | null { return total > 0 ? count / total * 100 : null; }
export function providerSwapHref(slug: string, source?: string, target?: string): string {
  const params = new URLSearchParams({ sources: slug });
  return `/#/swap/${encodeURIComponent(source ?? 'RUB')}/${encodeURIComponent(target ?? 'USDT')}?${params}`;
}
