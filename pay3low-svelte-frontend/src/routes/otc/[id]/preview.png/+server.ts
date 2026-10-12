import { error } from '@sveltejs/kit';
import { readShare } from '$lib/server/share-links';
import { renderOtcPreview } from '$lib/server/otc-preview';
import type { RequestHandler } from './$types';
export const GET: RequestHandler = async ({ params }) => {
  const record = await readShare(params.id);
  if (!record.target.startsWith('/#/otc?')) error(404, 'OTC link not found');
  const png = await renderOtcPreview(record.target, record.preview);
  return new Response(new Uint8Array(png), { headers: { 'content-type': 'image/png', 'content-length': String(png.length), 'cache-control': 'public, max-age=86400' } });
};
