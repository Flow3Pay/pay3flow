import sharp from "sharp";
import { shareAmount, shareCurrency } from "$lib/share";
import type { RequestHandler } from "./$types";

function xml(value: string): string {
  return value.replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&apos;" })[char]!);
}

function avatar(currency: string, x: number, y: number, data: string | null): string {
  if (data) return `<image x="${x}" y="${y}" width="58" height="58" href="${data}"/>`;
  return `<circle cx="${x + 29}" cy="${y + 29}" r="29" fill="#e5f2de"/><text x="${x + 29}" y="${y + 35}" text-anchor="middle" fill="#315a37" font-size="16" font-weight="800">${xml(currency.slice(0, 3))}</text>`;
}

async function assetIcon(fetch: typeof globalThis.fetch, currency: string): Promise<string | null> {
  if (!currency || !/^[a-z0-9]{2,12}$/.test(currency.toLowerCase())) return null;
  try {
    const response = await fetch(`/icons/assets/${currency.toLowerCase()}.png`);
    if (!response.ok) return null;
    const bytes = await response.arrayBuffer();
    if (bytes.byteLength > 100_000) return null;
    return `data:image/png;base64,${Buffer.from(bytes).toString("base64")}`;
  } catch {
    return null;
  }
}

export const GET: RequestHandler = async ({ url, fetch }) => {
  const source = shareCurrency(url.searchParams.get("from") ?? "") ?? "SEND";
  const target = shareCurrency(url.searchParams.get("to") ?? "") ?? "GET";
  const amount = shareAmount(url.searchParams.get("amount"));
  const receive = amount ? shareAmount(url.searchParams.get("receive")) : null;
  const [sourceIcon, targetIcon] = await Promise.all([assetIcon(fetch, source), assetIcon(fetch, target)]);
  const sourceSize = amount && amount.length > 18 ? 22 : amount && amount.length > 13 ? 30 : 39;
  const targetSize = receive && receive.length > 18 ? 22 : receive && receive.length > 13 ? 30 : 39;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="1200" height="630" viewBox="0 0 1200 630">
    <rect width="1200" height="630" fill="#f3f6f0"/>
    <circle cx="80" cy="80" r="230" fill="#e5eedf" opacity=".72"/>
    <circle cx="1150" cy="590" r="270" fill="#e5eedf" opacity=".65"/>
    <text x="72" y="557" fill="#17341d" font-family="Arial, sans-serif" font-size="30" font-weight="800">Pay3Flow</text>
    <text x="74" y="591" fill="#56715a" font-family="Arial, sans-serif" font-size="18">Live exchange routes</text>
    <rect x="274" y="34" width="652" height="562" rx="26" fill="#d8e3d3" opacity=".7"/>
    <rect x="264" y="26" width="652" height="562" rx="25" fill="#ffffff" stroke="#d4dfd0" stroke-width="2"/>
    <rect x="292" y="54" width="128" height="42" rx="12" fill="#ecf4e8"/>
    <text x="319" y="82" fill="#2c6038" font-family="Arial, sans-serif" font-size="19" font-weight="700">Bridge</text>
    <text x="804" y="81" fill="#899989" font-family="Arial, sans-serif" font-size="16">Pay3Flow</text>
    <text x="297" y="135" fill="#506955" font-family="Arial, sans-serif" font-size="17" font-weight="700">SELL</text>
    <rect x="292" y="151" width="596" height="146" rx="18" fill="#f7faf5" stroke="#dce7d7" stroke-width="2"/>
    <text x="318" y="187" fill="#637566" font-family="Arial, sans-serif" font-size="18">You send</text>
    <text x="318" y="253" fill="#1c2a1e" font-family="Arial, sans-serif" font-size="${sourceSize}" font-weight="700">${xml(amount ?? "Choose amount")}</text>
    ${avatar(source, 730, 194, sourceIcon)}
    <text x="860" y="234" text-anchor="end" fill="#243b29" font-family="Arial, sans-serif" font-size="22" font-weight="700">${xml(source)}</text>
    <line x1="296" y1="327" x2="884" y2="327" stroke="#dce7d7" stroke-width="3"/>
    <circle cx="590" cy="327" r="27" fill="#e6f2df" stroke="#cbdcc3" stroke-width="2"/>
    <text x="590" y="337" text-anchor="middle" fill="#386d43" font-family="Arial, sans-serif" font-size="27" font-weight="700">↓</text>
    <text x="297" y="371" fill="#506955" font-family="Arial, sans-serif" font-size="17" font-weight="700">BUY</text>
    <rect x="292" y="387" width="596" height="146" rx="18" fill="#f7faf5" stroke="#dce7d7" stroke-width="2"/>
    <text x="318" y="423" fill="#637566" font-family="Arial, sans-serif" font-size="18">Recipient gets</text>
    <text x="318" y="490" fill="#1c2a1e" font-family="Arial, sans-serif" font-size="${targetSize}" font-weight="700">${xml(receive ?? "Live quote on open")}</text>
    ${avatar(target, 730, 432, targetIcon)}
    <text x="860" y="473" text-anchor="end" fill="#243b29" font-family="Arial, sans-serif" font-size="22" font-weight="700">${xml(target)}</text>
    <text x="306" y="566" fill="#768a78" font-family="Arial, sans-serif" font-size="15">${receive ? "Quote at sharing · live routes refresh on open" : "Find current routes and quotes on Pay3Flow"}</text>
  </svg>`;
  const png = await sharp(Buffer.from(svg)).png().toBuffer();
  return new Response(new Uint8Array(png), {
    headers: { "content-type": "image/png", "cache-control": "public, max-age=86400", "content-length": String(png.length) },
  });
};
