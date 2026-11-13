import { shareAmount, shareCurrency } from './exchange-share.ts';

/** Wildcard side used by short asset links: /EVER expands to EVER/ANY. */
export const SHORT_PAIR_TARGET = 'ANY';

/**
 * Build the Exchange hash for a short asset link such as /EVER.
 * The asset is the base side; the quote side stays on the wildcard currency so
 * the existing discovery search compares every available destination.
 */
export function shortPairHash(value: string, input: URLSearchParams = new URLSearchParams()) {
  const source = shareCurrency(value);
  if (!source) return null;
  const params = new URLSearchParams();
  const amount = shareAmount(input.get('amount'));
  if (amount) params.set('amount', amount);
  params.set('from', `any-${source}`);
  params.set('to', `any-${SHORT_PAIR_TARGET}`);
  const language = input.get('lang');
  if (language === 'en' || language === 'ru' || language === 'hy') params.set('lang', language);
  return `#/swap/${encodeURIComponent(source)}/${SHORT_PAIR_TARGET}?${params}`;
}

export function shortPairLocation(value: string, input: URLSearchParams = new URLSearchParams()) {
  const hash = shortPairHash(value, input);
  return hash ? `/${hash}` : null;
}
