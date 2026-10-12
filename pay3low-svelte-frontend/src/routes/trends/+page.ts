import { dev } from '$app/environment';
import { SITE_URL } from '$lib/home-content';
import { trendsCopy } from '$lib/trends/copy';
import { t, type Locale } from '$lib/i18n';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ url }) => {
  const requested = url.searchParams.get('lang');
  const language: Locale = requested === 'ru' || requested === 'hy' ? requested : 'en';
  const origin = dev ? url.origin : SITE_URL.replace(/\/$/, '');
  const canonical = `${origin}/trends`;
  return {
    language,
    requestedLanguage: requested === 'en' || requested === 'ru' || requested === 'hy' ? language : null,
    title: `${t('TRENDS', {}, language)} · Pay3Flow`,
    description: trendsCopy[language].intro,
    imageAlt: trendsCopy[language].wall,
    canonical,
    shareUrl: requested ? `${canonical}?lang=${language}` : canonical,
    imageUrl: `${canonical}/preview.png?lang=${language}&v=1`,
  };
};
