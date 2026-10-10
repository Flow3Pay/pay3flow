import { error } from '@sveltejs/kit';
import type { PageLoad } from './$types';
export const load: PageLoad = ({ params }) => {
  if (!/^[a-z0-9][a-z0-9-]{0,79}$/.test(params.slug)) error(404, 'Platform not found');
  return { slug: params.slug };
};
