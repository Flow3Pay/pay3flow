import { error } from '@sveltejs/kit';
import { exchangeShareImage, exchangeShareParameters, shareCurrency } from '$lib/exchange-share';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = ({ params, url }) => {
  const source = shareCurrency(params.source), target = shareCurrency(params.target);
  if (!source || !target) error(404, 'Unknown exchange pair');
  const query = exchangeShareParameters(url.searchParams);
  const amount = query.get('amount'), receive = query.get('receive');
  const title = `${amount ? `${amount} ` : ''}${source} → ${receive ? `${receive} ` : ''}${target} · Pay3Flow`;
  const russian = query.get('lang') === 'ru';
  const description = russian
    ? `Маршрут обмена ${source} на ${target}. Откройте бридж с выбранными банками, сетями и настройками поиска. Расчёт обновляется при открытии.`
    : `Exchange ${source} for ${target}. Open the bridge with the selected banks, networks and search settings. Quotes refresh on opening.`;
  const shareUrl = `${url.origin}/swap/${source}/${target}?${query}`;
  const imageUrl = exchangeShareImage(url.origin, source, target, query);
  query.delete('receive');
  return { source, target, title, description, shareUrl, imageUrl, exchangeUrl: `/#/swap/${source}/${target}?${query}`, russian };
};
