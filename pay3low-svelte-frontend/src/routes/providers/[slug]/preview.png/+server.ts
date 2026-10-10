import { Buffer } from 'node:buffer';
import { error } from '@sveltejs/kit';
import { profileVenues } from '$lib/provider-profile';
import { renderProviderPreview } from '$lib/server/provider-preview';
import type { RequestHandler } from './$types';

const cache = new Map<string, Promise<Buffer>>();
export const GET: RequestHandler = async ({ params, url }) => {
  const venue = profileVenues.find(value => value.slug === params.slug);
  if (!venue) error(404, 'Platform not found');
  const language = url.searchParams.get('lang') === 'ru' ? 'ru' : url.searchParams.get('lang') === 'hy' ? 'hy' : 'en';
  const key = `${venue.slug}:${language}`;
  let pending = cache.get(key);
  if (!pending) {
    pending = renderProviderPreview(venue, language);
    cache.set(key, pending);
    pending.catch(() => cache.delete(key));
  }
  const png = await pending;
  return new Response(new Uint8Array(png), { headers: {
    'Content-Type': 'image/png', 'Cache-Control': 'public, max-age=86400', 'X-Content-Type-Options': 'nosniff',
  } });
};
