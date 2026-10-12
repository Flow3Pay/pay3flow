import { sharedPage } from '$lib/server/shared-page';
import type { PageServerLoad } from './$types';
export const load: PageServerLoad = ({ params, url }) => sharedPage(params.id, url.origin, true);
