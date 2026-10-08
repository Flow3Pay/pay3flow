import { error } from '@sveltejs/kit';
import { guideHash, guideParameters, sharedIdentifiers } from '$lib/guide-link';
import type { PageServerLoad } from './$types';
export const load: PageServerLoad = ({ params, url }) => {
  if (![params.source, params.target].every(value => /^[a-z0-9._-]{1,20}$/i.test(value))) error(404, 'Unknown currency');
  const source = params.source.toUpperCase(), target = params.target.toUpperCase();
  const query = guideParameters(url.searchParams);
  const russian = query.get('lang') === 'ru';
  const venues = sharedIdentifiers(query, 'venues').slice(0, 8).join(' → ');
  const title = `${source} → ${target} · Pay3Flow`;
  const description = russian ? `Пошаговая анимированная инструкция по обмену ${source} на ${target}${venues ? ` через ${venues}` : ''}.` : `An animated, step-by-step guide to exchanging ${source} for ${target}${venues ? ` through ${venues}` : ''}.`;
  const path = `/share/guide/${source}/${target}`;
  return { source, target, title, description, guideUrl: `/${guideHash(source, target, query)}`, shareUrl: `${url.origin}${path}?${query}`, imageUrl: `${url.origin}${path}/preview.png?${query}`, russian };
};
