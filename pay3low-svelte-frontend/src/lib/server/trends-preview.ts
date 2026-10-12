import { Buffer } from 'node:buffer';
import sharp from 'sharp';
import { read } from '$app/server';
import { assetIcon, networkIcon, venueIcon } from '$lib/icons';
import { t, type Locale } from '$lib/i18n';
import { demoTrendActivity } from '$lib/trends/demo';
import { aggregateTrends, layoutWall } from '$lib/trends/model';
import { emptyWallSlots } from '$lib/trends/background';
import { venueNames } from '$lib/trends/copy';

const assets = import.meta.glob<string>('/static/icons/{assets,venues}/*', { eager: true, query: '?url', import: 'default' });
const xml = (value: string) => value.replace(/[<>&"']/g, char => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;', "'": '&apos;' }[char]!));

async function icon(path: string) {
  const asset = assets[`/static${path}`];
  if (!asset) return '';
  const png = await sharp(Buffer.from(await read(asset).arrayBuffer())).resize(48, 48, { fit: 'contain' }).png().toBuffer();
  return `data:image/png;base64,${png.toString('base64')}`;
}

/** Show the public example wall using the same ranking, courses and native icons. */
export async function renderTrendsPreview(language: Locale) {
  const now = Date.now();
  const items = aggregateTrends(demoTrendActivity(now), '1w', 'searches', now);
  const widths: Record<string, number> = {};
  const assetWidth = (symbol: string) => 38 + symbol.length * 8;
  for (const item of items) {
    const name = venueNames[item.provider] ?? item.provider;
    widths[item.id] = assetWidth(item.asset) + 33 + (item.provider ? 40 + name.length * 7.5 : 0) + 2;
  }
  const cells = layoutWall(items, 1200, widths);
  const height = 41, pitch = height + 8;
  const wallHeight = (cells.at(-1)?.row ?? 0) * pitch + height + 4;
  const scale = Math.min(.96, 452 / wallHeight);
  const offsetX = (1200 - 1200 * scale) / 2, offsetY = 132;
  const left = -offsetX / scale, right = (1200 - offsetX) / scale;
  const top = (94 - offsetY) / scale, bottom = (594 - offsetY) / scale;
  const naturalWidths = [...new Set(cells.map(cell => cell.width))];
  const slots = emptyWallSlots(cells, naturalWidths, left, right, top, bottom, height);
  const paths = [...new Set(['/icons/assets/pay3flow-mark.svg', ...items.flatMap(item => [assetIcon(item.asset), networkIcon(item.network), ...(item.provider ? [venueIcon(item.provider)] : [])])])];
  const icons = new Map(await Promise.all(paths.map(async path => [path, await icon(path)] as const)));
  const image = (path: string, x: number, y: number, size: number) => icons.get(path) ? `<image href="${icons.get(path)}" x="${x}" y="${y}" width="${size}" height="${size}"/>` : '';
  const text = (x: number, y: number, size: number, value: string, color = '#172117', weight = 700) => `<text x="${x}" y="${y}" font-family="DejaVu Sans,sans-serif" font-size="${size}" font-weight="${weight}" fill="${color}">${xml(value)}</text>`;
  const slot = (x: number, y: number, width: number) => `<rect x="${x}" y="${y}" width="${width}" height="${height}" rx="5" fill="none" stroke="#dce1d8"/>`;
  const patternWidths = naturalWidths.slice(0, 6);
  const patternWidth = patternWidths.reduce((sum, width) => sum + width + 6, 0);
  let pattern = '';
  for (let row = 0; row < 2; row++) {
    let x = row ? 46 : 0;
    for (const width of patternWidths) {
      pattern += slot(x, row * pitch, width);
      if (x + width > patternWidth) pattern += slot(x - patternWidth, row * pitch, width);
      x += width + 6;
    }
  }
  const mask = [...new Set(cells.map(cell => cell.row))].map(row => `<rect x="${left}" y="${row * pitch - 1}" width="${right - left}" height="${pitch + 1}" fill="black"/>`).join('');
  const bricks = cells.map(cell => {
    const { item, left: x, width } = cell, y = cell.row * pitch;
    const divider = x + assetWidth(item.asset), venueStart = divider + 33;
    const name = venueNames[item.provider] ?? item.provider;
    return `<rect x="${x}" y="${y + 4}" width="${width}" height="${height}" rx="5" fill="#cdd3c5"/>
      <rect x="${x}" y="${y}" width="${width}" height="${height}" rx="5" fill="#e7ebe2" stroke="#d7dcd3"/>
      ${image(assetIcon(item.asset), x + 7, y + 11.5, 18)}${text(x + 31, y + 25, 12, item.asset)}
      <path d="M${divider} ${y}v${height}" stroke="#d7dcd3"/>${image(networkIcon(item.network), divider + 7, y + 11.5, 18)}
      ${item.provider ? `<path d="M${venueStart} ${y}v${height}" stroke="#d7dcd3"/>${image(venueIcon(item.provider), venueStart + 7, y + 11.5, 18)}${text(venueStart + 31, y + 25, 12, name)}` : ''}`;
  }).join('');
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
    <defs>
      <pattern id="slots" width="${patternWidth}" height="${pitch * 2}" patternUnits="userSpaceOnUse">${pattern}</pattern>
      <mask id="emptyCourses" maskUnits="userSpaceOnUse" x="${left}" y="${top}" width="${right - left}" height="${bottom - top}"><rect x="${left}" y="${top}" width="${right - left}" height="${bottom - top}" fill="white"/>${mask}</mask>
      <clipPath id="wall"><rect y="94" width="1200" height="500"/></clipPath>
    </defs>
    <rect width="1200" height="630" fill="#ffffff"/>
    ${image('/icons/assets/pay3flow-mark.svg', 40, 25, 44)}${text(99, 57, 27, 'Pay3Flow')}
    <rect x="285" y="25" width="210" height="44" rx="7" fill="#efffc3"/>${text(307, 56, 25, t('TRENDS', {}, language))}
    <path d="M0 89H1200" stroke="#e1e5de"/>
    <g clip-path="url(#wall)"><g transform="translate(${offsetX} ${offsetY}) scale(${scale})">
      <rect x="${left}" y="${top}" width="${right - left}" height="${bottom - top}" fill="url(#slots)" mask="url(#emptyCourses)"/>
      ${slots.map(value => slot(value.left, value.top, value.width)).join('')}${bricks}
    </g></g>
    ${text(40, 616, 15, 'pay3flow.lefine.pro/trends', '#697565', 400)}
  </svg>`;
  return sharp(Buffer.from(svg)).png().toBuffer();
}
