import { Buffer } from 'node:buffer';
import sharp from 'sharp';
import { read } from '$app/server';
import { venueIcon } from '$lib/icons';
import { profileCopy } from '$lib/provider-profile-copy';
import type { Locale } from '$lib/i18n';
import type { ProfileVenue } from '$lib/provider-profile';

const assets = import.meta.glob<string>('/static/icons/{assets,venues}/*', { eager: true, query: '?url', import: 'default' });
const escape = (value: string) => value.replace(/[<>&"']/g, char => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;', "'": '&apos;' }[char]!));

async function icon(path: string, size: number) {
  const asset = assets[`/static${path}`];
  if (!asset) return '';
  const png = await sharp(Buffer.from(await read(asset).arrayBuffer())).resize(size, size, { fit: 'contain' }).png().toBuffer();
  return `data:image/png;base64,${png.toString('base64')}`;
}

export async function renderProviderPreview(venue: ProfileVenue, language: Locale) {
  const copy = profileCopy(language);
  const [brand, avatar] = await Promise.all([icon('/icons/assets/pay3flow-mark.svg', 44), icon(venueIcon(venue.slug), 168)]);
  const text = (x: number, y: number, size: number, value: string, color = '#f3f3f3', weight = 400) => `<text x="${x}" y="${y}" font-family="DejaVu Sans,sans-serif" font-size="${size}" font-weight="${weight}" fill="${color}">${escape(value)}</text>`;
  const image = (href: string, x: number, y: number, size: number) => href ? `<image href="${href}" x="${x}" y="${y}" width="${size}" height="${size}"/>` : '';
  let badgeX = 356;
  const badges = venue.types.map(type => {
    const label = copy[type], width = Math.max(96, label.length * 15 + 36), x = badgeX;
    badgeX += width + 14;
    return `<rect x="${x}" y="314" width="${width}" height="46" rx="9" fill="#b5f500" fill-opacity=".12" stroke="#b5f500"/>${text(x + 18, 344, 22, label, '#b5f500', 700)}`;
  }).join('');
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
    <rect width="1200" height="630" fill="#181b16"/>
    ${image(brand, 60, 46, 44)}${text(119, 78, 27, 'Pay3Flow', '#f3f3f3', 700)}
    <rect x="60" y="137" width="1080" height="352" rx="28" fill="#23271f" stroke="#414a35"/>
    <rect x="60" y="137" width="1080" height="5" rx="2" fill="#b5f500"/>
    <rect x="104" y="211" width="206" height="206" rx="36" fill="#f5f7f0"/>
    ${image(avatar, 123, 230, 168)}
    ${text(356, 210, 19, copy.profile, '#bac2b1')}
    ${text(352, 285, venue.name.length > 17 ? 48 : 60, venue.name, '#f3f3f3', 700)}
    ${badges}
    ${text(356, 418, 23, `${copy.ranking} · ${copy.directions} · ${copy.reviews}`, '#bac2b1')}
    ${text(60, 568, 20, `pay3flow.lefine.pro/providers/${venue.slug}`, '#b5f500')}
  </svg>`;
  return sharp(Buffer.from(svg)).png().toBuffer();
}
