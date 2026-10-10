import { Buffer } from 'node:buffer';
import sharp from 'sharp';
import { read } from '$app/server';
import { assetIcon, venueIcon } from '$lib/icons';
import { sharedIdentifiers } from '$lib/guide-link';

const escape = (text: string) => text.replace(/[<>&"']/g, char => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;', "'": '&apos;' }[char]!));
const bundledIcons = import.meta.glob<string>('/static/icons/{assets,venues}/*', { eager: true, query: '?url', import: 'default' });
const icons = new Map<string, Promise<string>>();
async function icon(path: string) {
  let pending = icons.get(path);
  if (!pending) {
    pending = (async () => {
      const asset = bundledIcons[`/static${path}`];
      if (!asset) return '';
      const response = read(asset);
      const bytes = await sharp(Buffer.from(await response.arrayBuffer())).resize(80, 80).png().toBuffer();
      return `data:image/png;base64,${bytes.toString('base64')}`;
    })();
    icons.set(path, pending);
    pending.catch(() => icons.delete(path));
  }
  return pending;
}
const names: Record<string, string> = { whitebird: 'WhiteBird', bestchange: 'BestChange', 'cow-swap': 'CoW Swap', 'near-intents': 'NEAR Intents', '1inch': '1inch', '0x': '0x', 'id-pay': 'ID Pay', 'papa-change': 'Papa Change', 'cifra-broker': 'Cifra Broker' };
const label = (value: string) => names[value] ?? value.split('-').map(part => part[0]?.toUpperCase() + part.slice(1)).join(' ');
export async function renderGuidePreview(source: string, target: string, params: URLSearchParams) {
  const venues = sharedIdentifiers(params, 'venues').slice(0, 3);
  const [brand, fromIcon, toIcon, ...venueIcons] = await Promise.all([icon("/icons/assets/pay3flow-mark.svg"), icon(assetIcon(source)), icon(assetIcon(target)), ...venues.map(venue => icon(venueIcon(venue)))]);
  const russian = params.get('lang') === 'ru';
  const text = (x: number, y: number, size: number, value: string, fill = '#f3f7ef', weight = 400) => `<text x="${x}" y="${y}" font-family="DejaVu Sans,sans-serif" font-size="${size}" font-weight="${weight}" fill="${fill}">${escape(value)}</text>`;
  const image = (href: string, x: number, y: number, size: number) => href ? `<image href="${href}" x="${x}" y="${y}" width="${size}" height="${size}"/>` : '';
  const rows = venues.length ? venues : [''];
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
    <defs><pattern id="grid" width="36" height="36" patternUnits="userSpaceOnUse"><path d="M36 0H0V36" fill="none" stroke="#c9ddbd" stroke-opacity=".05"/></pattern><radialGradient id="glow"><stop stop-color="#b5f500" stop-opacity=".10"/><stop offset="1" stop-color="#19211d" stop-opacity="0"/></radialGradient></defs>
    <rect width="1200" height="630" fill="#19211d"/><rect width="1200" height="630" fill="url(#grid)"/><ellipse cx="910" cy="300" rx="330" ry="300" fill="url(#glow)"/>
    <circle cx="915" cy="307" r="220" fill="none" stroke="#b5f500" stroke-opacity=".09"/><circle cx="915" cy="307" r="285" fill="none" stroke="#b5f500" stroke-opacity=".05"/>
    ${image(brand, 64, 60, 43)}${text(122, 93, 28, 'Pay3Flow', '#f3f7ef', 700)}
    ${text(64, 202, 14, russian ? 'ГИД ПО ОБМЕНУ' : 'EXCHANGE GUIDE', '#b5f500', 700)}
    ${text(62, 286, source.length > 7 ? 48 : 68, source, '#f3f7ef', 700)}${text(64, 375, target.length > 7 ? 48 : 68, `→ ${target}`, '#b5f500', 700)}
    ${text(64, 453, 22, russian ? 'Один маршрут.' : 'One route.', '#c5cfc1')}${text(64, 488, 22, russian ? 'Шаг за шагом.' : 'One step at a time.', '#c5cfc1')}
    ${text(64, 572, 14, 'pay3flow.lefine.pro', '#97aa97')}
    ${image(fromIcon, 760, 86, 70)}${image(toIcon, 976, 86, 70)}${text(766, 186, 18, source, '#f3f7ef', 700)}${text(981, 186, 18, target, '#f3f7ef', 700)}
    <path d="M854 121H948" stroke="#b5f500" stroke-width="2" stroke-dasharray="4 12"/><path d="M936 113l12 8-12 8" fill="none" stroke="#b5f500" stroke-width="2"/>
    <g transform="rotate(-4 913 360)"><rect x="680" y="225" width="458" height="${148 + rows.length * 61}" rx="20" fill="#f4f6ed"/>${image(brand, 705, 250, 30)}${text(750, 272, 15, russian ? 'Вы контролируете каждый шаг' : 'You control every step', '#19211d', 700)}
    ${rows.map((venue, index) => `<rect x="705" y="${301 + index * 61}" width="408" height="49" rx="10" fill="#e9eddf"/>${text(719, 332 + index * 61, 13, String(index + 1).padStart(2, '0'), '#637452')}${venue ? image(venueIcons[index], 757, 311 + index * 61, 28) : ''}${text(798, 333 + index * 61, 16, venue ? label(venue).slice(0, 27) : `${source} → ${target}`, '#19211d', 700)}${text(1080, 333 + index * 61, 20, '↗', '#637452')}`).join('')}
    ${text(708, 384 + (rows.length - 1) * 61, 13, russian ? 'В вашем темпе' : 'At your own pace', '#637452')}</g>
    </svg>`;
  return sharp(Buffer.from(svg)).png().toBuffer();
}
