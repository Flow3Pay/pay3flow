import sharp from 'sharp';
import { read } from '$app/server';
import { assetIcon } from '$lib/icons';
import { formatAmount, formatPrice, marketFromHash, otcFallbackNetworks, prepareDemoOrder } from '$lib/otc/model';
import { parseOtcHash } from '$lib/otc/link';
import type { OtcPreview } from '$lib/share-links';

const assets = import.meta.glob<string>('/static/icons/assets/*', { eager: true, query: '?url', import: 'default' });
const xml = (text: string) => text.replace(/[<>&"']/g, char => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;', "'": '&apos;' }[char]!));
async function icon(path: string) {
  const asset = assets[`/static${path}`];
  if (!asset) return '';
  const png = await sharp(Buffer.from(await read(asset).arrayBuffer())).resize(48, 48, { fit: 'contain' }).png().toBuffer();
  return `data:image/png;base64,${png.toString('base64')}`;
}

export async function renderOtcPreview(target: string, preview: OtcPreview | null) {
  const market = marketFromHash(target.slice(1));
  const state = parseOtcHash(target.slice(1));
  const selling = state.side === 'sell', marketOrder = state.type === 'market';
  const from = selling ? market.base : market.quote, to = selling ? market.quote : market.base;
  const amount = state.amount ?? '', price = state.price ?? '';
  const draft = !marketOrder ? prepareDemoOrder(market, { marketId: market.id, mode: "test", bids: [], asks: [], trades: [] }, selling ? "sell" : "buy", "limit", amount, price).draft : null;
  const receive = draft ? selling ? formatPrice(draft.total, market) : formatAmount(draft.amount, market) : "—";
  const [brand, fromIcon, toIcon] = await Promise.all([icon('/icons/assets/pay3flow-mark.svg'), icon(assetIcon(from)), icon(assetIcon(to))]);
  const text = (x: number, y: number, size: number, value: string, color = '#edf1ed', weight = 400) => `<text x="${x}" y="${y}" font-family="DejaVu Sans,sans-serif" font-size="${size}" font-weight="${weight}" fill="${color}">${xml(value)}</text>`;
  const image = (href: string, x: number, y: number, size: number) => href ? `<image href="${href}" x="${x}" y="${y}" width="${size}" height="${size}"/>` : '';
  const fieldNumber = (value: string) => value.length > 15 ? `${value.slice(0, 14)}…` : value || '—';
  const network = (id?: string) => otcFallbackNetworks.find(network => network.id === id)?.name ?? '';
  const closes = preview?.closes.filter(Number.isFinite) ?? [];
  const minimum = closes.length ? Math.min(...closes) : 0, maximum = closes.length ? Math.max(...closes) : 0;
  const spread = maximum - minimum || maximum * .02 || 1;
  const points = closes.map((close, index) => `${404 + index / Math.max(1, closes.length - 1) * 424},${435 - (close - minimum) / spread * 208}`).join(' ');
  const rows = (levels: NonNullable<OtcPreview>['bids'], y: number, color: string) => levels.map((level, index) => {
    const maxAmount = Math.max(...levels.map(item => item.amount), 1);
    return `<rect x="914" y="${y + index * 30 - 20}" width="${Math.max(4, level.amount / maxAmount * 222)}" height="27" fill="${color}" opacity=".10"/>${text(924, y + index * 30, 14, level.price.toFixed(market.priceDecimals), color)}${text(1044, y + index * 30, 14, level.amount.toLocaleString('en-US', { maximumFractionDigits: 3 }), '#c5cec5')}`;
  }).join('');
  const stamp = preview?.capturedAt ? new Date(preview.capturedAt).toISOString().slice(0, 16).replace('T', ' ') + ' UTC' : '';
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
    <defs><linearGradient id="chartFill" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#a1ff2b" stop-opacity=".18"/><stop offset="1" stop-color="#a1ff2b" stop-opacity="0"/></linearGradient></defs>
    <rect width="1200" height="630" fill="#101310"/>
    ${image(brand, 34, 28, 44)}${text(92, 59, 25, 'Pay3Flow', '#f2f5ef', 700)}${text(262, 58, 19, 'OTC', '#a1ff2b', 700)}
    ${text(35, 112, 30, `${from} → ${to}`, '#f2f5ef', 700)}${text(920, 111, 17, 'Your price. Your amount.', '#abb5aa')}
    <rect x="30" y="143" width="312" height="416" rx="16" fill="#1c201c" stroke="#353d35"/>
    <rect x="362" y="143" width="510" height="416" rx="16" fill="#1c201c" stroke="#353d35"/>
    <rect x="892" y="143" width="278" height="416" rx="16" fill="#1c201c" stroke="#353d35"/>
    ${text(51, 181, 20, 'Bridge', '#f2f5ef', 700)}${text(384, 181, 20, 'Chart', '#f2f5ef', 700)}${text(914, 181, 20, 'Orderbook', '#f2f5ef', 700)}
    ${text(50, 218, 14, `${selling ? 'Sell' : 'Buy'} · ${marketOrder ? 'Market' : 'Limit'}`, selling ? '#f17484' : '#a1ff2b', 700)}
    ${text(50, 253, 12, `Price · ${market.quote}`, '#aeb9ad')}${text(50, 282, 23, marketOrder ? 'Market price' : fieldNumber(price), '#f2f5ef', 700)}
    <rect x="48" y="299" width="276" height="94" rx="10" fill="#272c27"/>
    ${text(62, 322, 12, 'You send', '#aeb9ad')}${text(62, 351, 22, fieldNumber(amount), '#f2f5ef', 700)}${image(fromIcon, 256, 313, 26)}${text(252, 373, 13, from, '#f2f5ef', 700)}${text(62, 376, 11, network(state.sendNetworkId), '#aeb9ad')}
    ${text(177, 417, 24, '↓', '#a1ff2b', 700)}
    <rect x="48" y="427" width="276" height="94" rx="10" fill="#272c27"/>
    ${text(62, 450, 12, 'You receive', '#aeb9ad')}${text(62, 479, 22, marketOrder ? 'Market estimate' : fieldNumber(receive), '#f2f5ef', 700)}${image(toIcon, 256, 441, 26)}${text(252, 501, 13, to, '#f2f5ef', 700)}${text(62, 504, 11, network(state.receiveNetworkId), '#aeb9ad')}
    ${text(385, 214, 14, `${market.base} / ${market.quote}`, '#aeb9ad')}
    ${[242, 300, 358, 416, 474].map(y => `<path d="M386 ${y}H850" stroke="#333c33"/>`).join('')}
    ${[404, 510, 616, 722, 828].map(x => `<path d="M${x} 228V478" stroke="#333c33"/>`).join('')}
    ${closes.length > 1 ? `<polygon points="404,478 ${points} 828,478" fill="url(#chartFill)"/><polyline points="${points}" fill="none" stroke="#a1ff2b" stroke-width="3" stroke-linejoin="round"/>` : text(470, 352, 17, 'Open OTC to load market data', '#aeb9ad')}
    ${text(386, 517, 12, 'Market snapshot · 1D', '#aeb9ad')}
    ${text(916, 214, 12, `Price (${market.quote})`, '#aeb9ad')}${text(1044, 214, 12, market.base, '#aeb9ad')}
    ${rows(preview?.asks ?? [], 244, '#f17484')}
    <path d="M914 423H1146" stroke="#3d473d"/>
    ${rows((preview?.bids ?? []).slice(0, 3), 453, '#a1ff2b')}
    ${text(35, 596, 14, 'Bridge · Chart · Orderbook', '#a1ff2b', 700)}${text(375, 596, 12, 'Snapshot at sharing · prices refresh on opening', '#aeb9ad')}${text(910, 596, 12, stamp, '#aeb9ad')}
  </svg>`;
  return sharp(Buffer.from(svg)).png().toBuffer();
}
