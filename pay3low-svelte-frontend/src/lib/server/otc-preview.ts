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
  const text = (x: number, y: number, size: number, value: string, color = '#eeeeee', weight = 400) => `<text x="${x}" y="${y}" font-family="DejaVu Sans,sans-serif" font-size="${size}" font-weight="${weight}" fill="${color}">${xml(value)}</text>`;
  const image = (href: string, x: number, y: number, size: number) => href ? `<image href="${href}" x="${x}" y="${y}" width="${size}" height="${size}"/>` : '';
  const fieldNumber = (value: string) => value.length > 15 ? `${value.slice(0, 14)}…` : value || '—';
  const amountSize = (value: string) => value.length > 13 ? 14 : value.length > 10 ? 17 : 22;
  const network = (id?: string) => otcFallbackNetworks.find(network => network.id === id)?.name ?? '';
  const closes = preview?.closes.filter(Number.isFinite) ?? [];
  // Match the Buy/Sell offsets used by OtcChart, with a shared scale for both lines.
  const offset = market.price * .0015, padding = market.price * .004;
  const minimum = closes.length ? Math.min(...closes) - offset - padding : 0;
  const maximum = closes.length ? Math.max(...closes) + offset + padding : 0;
  const spread = maximum - minimum || maximum * .02 || 1;
  const points = (offset: number) => closes.map((close, index) => `${404 + index / Math.max(1, closes.length - 1) * 424},${478 - (close + offset - minimum) / spread * 250}`).join(' ');
  const buyPoints = points(-offset), sellPoints = points(offset);
  const rows = (levels: NonNullable<OtcPreview>['bids'], y: number, color: string) => levels.map((level, index) => {
    const maxAmount = Math.max(...levels.map(item => item.amount), 1);
    return `<rect x="914" y="${y + index * 30 - 20}" width="${Math.max(4, level.amount / maxAmount * 222)}" height="27" fill="${color}" opacity=".10"/>${text(924, y + index * 30, 14, level.price.toFixed(market.priceDecimals), color)}${text(1044, y + index * 30, 14, level.amount.toLocaleString('en-US', { maximumFractionDigits: 3 }), '#cccccc')}`;
  }).join('');
  const stamp = preview?.capturedAt ? new Date(preview.capturedAt).toISOString().slice(0, 16).replace('T', ' ') + ' UTC' : '';
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
    <defs><linearGradient id="chartFill" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#a1ff2b" stop-opacity=".07"/><stop offset="1" stop-color="#a1ff2b" stop-opacity="0"/></linearGradient></defs>
    <rect width="1200" height="630" fill="#181818"/>
    ${image(brand, 34, 28, 44)}${text(92, 59, 25, 'Pay3Flow', '#f2f2f2', 700)}${text(262, 58, 19, 'OTC', '#a1ff2b', 700)}
    ${text(35, 112, 30, `${from} → ${to}`, '#f2f2f2', 700)}${text(920, 111, 17, 'Your price. Your amount.', '#b5b5b5')}
    <rect x="30" y="143" width="312" height="416" rx="16" fill="#292929" stroke="#494949"/>
    <rect x="362" y="143" width="510" height="416" rx="16" fill="#292929" stroke="#494949"/>
    <rect x="892" y="143" width="278" height="416" rx="16" fill="#292929" stroke="#494949"/>
    ${text(51, 181, 20, 'Bridge', '#f2f2f2', 700)}${text(384, 181, 20, 'Chart', '#f2f2f2', 700)}${text(914, 181, 20, 'Orderbook', '#f2f2f2', 700)}
    <circle cx="718" cy="175" r="3" fill="#a1ff2b"/>${text(728, 180, 12, 'Buy', '#b8b8b8')}<circle cx="782" cy="175" r="3" fill="#f17484"/>${text(792, 180, 12, 'Sell', '#b8b8b8')}
    ${text(50, 218, 14, `${selling ? 'Sell' : 'Buy'} · ${marketOrder ? 'Market' : 'Limit'}`, selling ? '#f17484' : '#a1ff2b', 700)}
    <rect x="48" y="232" width="276" height="55" rx="10" fill="#383838" stroke="#494949"/>${text(62, 252, 11, `Price · ${market.quote}`, '#b8b8b8')}${text(62, 275, 20, marketOrder ? 'Market price' : fieldNumber(price), '#f2f2f2', 700)}
    <rect x="48" y="299" width="276" height="94" rx="10" fill="#383838" stroke="#494949"/>
    ${text(62, 322, 12, 'You send', '#b8b8b8')}${text(62, 358, amountSize(amount), fieldNumber(amount), '#f2f2f2', 700)}<rect x="226" y="334" width="86" height="38" rx="9" fill="#292929"/>${image(fromIcon, 233, 342, 22)}${text(261, 358, 13, from, '#f2f2f2', 700)}${text(62, 376, 11, network(state.sendNetworkId), '#b8b8b8')}
    <circle cx="186" cy="410" r="15" fill="#292929" stroke="#555555"/>${text(178, 417, 20, '↓', '#cccccc', 700)}
    <rect x="48" y="427" width="276" height="94" rx="10" fill="#383838" stroke="#494949"/>
    ${text(62, 450, 12, 'You receive', '#b8b8b8')}${text(62, 486, marketOrder ? 14 : amountSize(receive), marketOrder ? 'Market estimate' : fieldNumber(receive), '#f2f2f2', 700)}<rect x="226" y="462" width="86" height="38" rx="9" fill="#292929"/>${image(toIcon, 233, 470, 22)}${text(261, 486, 13, to, '#f2f2f2', 700)}${text(62, 504, 11, network(state.receiveNetworkId), '#b8b8b8')}
    ${text(385, 214, 14, `${market.base} / ${market.quote}`, '#b8b8b8')}
    ${[242, 300, 358, 416, 474].map(y => `<path d="M386 ${y}H850" stroke="#404040"/>`).join('')}
    ${[404, 510, 616, 722, 828].map(x => `<path d="M${x} 228V478" stroke="#404040"/>`).join('')}
    ${closes.length > 1 ? `<polygon points="404,478 ${buyPoints} 828,478" fill="url(#chartFill)"/><polyline points="${buyPoints}" fill="none" stroke="#a1ff2b" stroke-width="2.5" stroke-linejoin="round"/><polyline points="${sellPoints}" fill="none" stroke="#f17484" stroke-width="2.5" stroke-linejoin="round"/>` : text(470, 352, 17, 'Open OTC to load market data', '#b8b8b8')}
    ${text(386, 517, 12, 'Market snapshot · 1D', '#b8b8b8')}
    ${text(916, 214, 12, `Price (${market.quote})`, '#b8b8b8')}${text(1044, 214, 12, market.base, '#b8b8b8')}
    ${rows(preview?.asks ?? [], 244, '#f17484')}
    <path d="M914 423H1146" stroke="#494949"/>
    ${rows((preview?.bids ?? []).slice(0, 3), 453, '#a1ff2b')}
    ${text(35, 596, 14, 'Bridge · Chart · Orderbook', '#cccccc', 700)}${text(375, 596, 12, 'Snapshot at sharing · prices refresh on opening', '#b8b8b8')}${text(910, 596, 12, stamp, '#b8b8b8')}
  </svg>`;
  return sharp(Buffer.from(svg)).png().toBuffer();
}
