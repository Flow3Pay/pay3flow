import { error } from '@sveltejs/kit';
import { dev } from '$app/environment';
import { SITE_URL } from '$lib/home-content';
import { exchangeShareImage, exchangeShareParameters, shareCurrency } from '$lib/exchange-share';
import { guideHash, guideParameters } from '$lib/guide-link';
import { marketFromHash } from '$lib/otc/model';
import { parseOtcHash } from '$lib/otc/link';
import { readShare } from './share-links';

export async function sharedPage(id: string, origin: string, otc: boolean) {
  origin = dev ? origin : SITE_URL.replace(/\/$/, "");
  const record = await readShare(id);
  const sharingOtc = record.target.startsWith('/#/otc?');
  if (sharingOtc !== otc) error(404, 'Share link not found');
  const shareUrl = `${origin}/${otc ? 'otc' : 's'}/${id}`;
  if (otc) {
    const state = parseOtcHash(record.target.slice(1));
    const market = marketFromHash(record.target.slice(1));
    const from = state.side === 'sell' ? market.base : market.quote, to = state.side === 'sell' ? market.quote : market.base;
    const query = new URLSearchParams(record.target.split('?')[1]);
    const russian = query.get('lang') === 'ru';
    const description = russian ? `OTC ${from} → ${to}. Сумма: ${state.amount || '—'}. Цена: ${state.type === 'market' ? 'рыночная' : state.price || '—'} ${market.quote}. Bridge, Chart и Orderbook.` : `OTC ${from} → ${to}. Amount: ${state.amount || '—'}. Price: ${state.type === 'market' ? 'market' : state.price || '—'} ${market.quote}. Bridge, Chart and Orderbook.`;
    return { title: `${from} → ${to} · Pay3Flow OTC`, description, shareUrl, imageUrl: `${shareUrl}/preview.png?v=2`, exchangeUrl: record.target, russian, otc, imageAlt: 'Pay3Flow OTC · Bridge · Chart · Orderbook' };
  }
  const target = new URL(record.target, origin);
  const guide = target.pathname.startsWith('/share/guide/');
  const segments = target.pathname.split('/');
  const source = shareCurrency(segments.at(-2) ?? ''), destination = shareCurrency(segments.at(-1) ?? '');
  if (!source || !destination) error(404, 'Unknown exchange pair');
  const query = guide ? guideParameters(target.searchParams) : exchangeShareParameters(target.searchParams);
  const russian = query.get('lang') === 'ru';
  const imageUrl = guide ? `${origin}${target.pathname}/preview.png?${query}&v=2` : exchangeShareImage(origin, source, destination, query);
  const title = `${query.get('amount') ? `${query.get('amount')} ` : ''}${source} → ${destination} · Pay3Flow`;
  const description = russian ? `Откройте обмен ${source} на ${destination} с выбранными активами, банками, сетями и настройками.` : `Open ${source} to ${destination} with the selected assets, banks, networks and settings.`;
  query.delete('receive');
  return { title, description, shareUrl, imageUrl, exchangeUrl: guide ? `/${guideHash(source, destination, query)}` : `/#/swap/${source}/${destination}?${query}`, russian, otc, imageAlt: `${source} → ${destination} · Pay3Flow ${guide ? 'guide' : 'Bridge'}` };
}
