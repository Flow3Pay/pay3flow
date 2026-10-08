import { Buffer } from 'node:buffer';
import { error } from '@sveltejs/kit';
import { guideParameters } from '$lib/guide-link';
import { renderGuidePreview } from '$lib/server/guide-preview';
import type { RequestHandler } from './$types';
const cache = new Map<string, Promise<Buffer>>();
export const GET: RequestHandler = async ({ params, url }) => {
  if (![params.source, params.target].every(value => /^[a-z0-9._-]{1,20}$/i.test(value))) error(404, 'Unknown currency');
  const source = params.source.toUpperCase(), target = params.target.toUpperCase();
  const query = guideParameters(url.searchParams);
  const key = `${source}|${target}|${query.get('venues') ?? ''}|${query.get('lang') ?? ''}`;
  let pending = cache.get(key);
  if (!pending) {
    if (cache.size >= 64) cache.delete(cache.keys().next().value!);
    pending = renderGuidePreview(source, target, query);
    cache.set(key, pending);
    pending.catch(() => cache.delete(key));
  }
  const png = await pending;
  return new Response(new Uint8Array(png), { headers: { 'Content-Type': 'image/png', 'Cache-Control': 'public, max-age=86400', 'X-Content-Type-Options': 'nosniff' } });
};
