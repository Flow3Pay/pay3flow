import { error, json } from '@sveltejs/kit';
import { shareApi } from '$lib/server/share-links';
import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ request }) => {
  const body = await request.text();
  if (body.length > 25000) error(413, 'Share settings are too large');
  return json(await shareApi('/api/share-links', { method: 'POST', headers: { 'content-type': 'application/json' }, body }), { headers: { 'cache-control': 'no-store' } });
};
