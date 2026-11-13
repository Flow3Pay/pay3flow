import { error } from '@sveltejs/kit';
import { shortPairLocation } from '$lib/short-pair';
import type { RequestHandler } from './$types';

const redirectToPair: RequestHandler = ({ params, url }) => {
  const location = shortPairLocation(params.ticker, url.searchParams);
  if (!location) error(404, 'Unknown asset');
  return new Response(null, {
    status: 302,
    headers: { location, 'cache-control': 'no-store' },
  });
};

export const GET = redirectToPair;
export const HEAD = redirectToPair;
