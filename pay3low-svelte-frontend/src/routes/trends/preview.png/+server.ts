import { Buffer } from 'node:buffer';
import { renderTrendsPreview } from '$lib/server/trends-preview';
import type { Locale } from '$lib/i18n';
import type { RequestHandler } from './$types';

const cache = new Map<Locale, Promise<Buffer>>();
export const GET: RequestHandler = async ({ url }) => {
  const requested = url.searchParams.get('lang');
  const language: Locale = requested === 'ru' || requested === 'hy' ? requested : 'en';
  let pending = cache.get(language);
  if (!pending) {
    pending = renderTrendsPreview(language);
    cache.set(language, pending);
    pending.catch(() => cache.delete(language));
  }
  const png = await pending;
  return new Response(new Uint8Array(png), { headers: {
    'Content-Type': 'image/png', 'Cache-Control': 'public, max-age=86400', 'X-Content-Type-Options': 'nosniff',
  } });
};
