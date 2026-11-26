import { error } from '@sveltejs/kit';
import { dev } from '$app/environment';
import { SITE_URL } from '$lib/home-content';
import { assetDisplayName, isPopularSwapPair } from '$lib/swap-landing';
import { exchangeShareImage, exchangeShareParameters, shareCurrency } from '$lib/exchange-share';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = ({ params, url }) => {
  const source = shareCurrency(params.source), target = shareCurrency(params.target);
  if (!source || !target) error(404, 'Unknown exchange pair');
  const query = exchangeShareParameters(url.searchParams);
  const amount = query.get('amount');
  const russian = query.get('lang') === 'ru';
  const sourceName = assetDisplayName(source), targetName = assetDisplayName(target);
  const origin = dev ? url.origin : SITE_URL.replace(/\/$/, '');
  // The clean pair URL is what search engines index; amount and filters stay in
  // the app link and the Open Graph preview.
  const canonical = `${origin}/swap/${source}/${target}`;
  const title = russian
    ? `Обмен ${source} на ${targetName} · Pay3Flow`
    : `Exchange ${source} for ${targetName} · Pay3Flow`;
  const description = russian
    ? `Сравните живые маршруты обмена ${sourceName} (${source}) на ${targetName} (${target}) по подключённым площадкам, банкам и сетям. Сумма и предложения пересчитываются при открытии обмена.`
    : `Compare live routes to exchange ${sourceName} (${source}) for ${targetName} (${target}) across connected providers, banks and networks. Amounts and quotes refresh when the exchange opens.`;
  const imageUrl = exchangeShareImage(origin, source, target, query);
  query.delete('receive');
  return {
    source,
    target,
    sourceName,
    targetName,
    amount,
    title,
    description,
    canonical,
    indexable: isPopularSwapPair(source, target),
    imageUrl,
    exchangeUrl: `/#/swap/${source}/${target}?${query}`,
    russian,
  };
};
