import sharp from "sharp";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { shareAmount, shareCurrency } from "$lib/exchange-share";
import { networkIcon } from "$lib/icons";
import type { RequestHandler } from "./$types";

function xml(value: string): string {
  return value.replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&apos;" })[char]!);
}

function avatar(currency: string, x: number, y: number, data: string | null): string {
  if (data) return `<clipPath id="clip-${x}-${y}"><circle cx="${x + 29}" cy="${y + 29}" r="29"/></clipPath><image x="${x}" y="${y}" width="58" height="58" href="${data}" clip-path="url(#clip-${x}-${y})"/>`;
  return `<circle cx="${x + 29}" cy="${y + 29}" r="29" fill="#31442a"/><text x="${x + 29}" y="${y + 35}" text-anchor="middle" fill="#b5f500" font-size="16" font-weight="800">${xml(currency.slice(0, 3))}</text>`;
}

const FIAT_FLAGS: Record<string, string> = { AMD: "am", BYN: "by", KZT: "kz", RUB: "ru", UAH: "ua", USD: "us" };

async function localIcon(path: string): Promise<string | null> {
  if (!/^\/icons\/(?:assets|flags)\/[a-z0-9_-]+\.(?:png|webp|svg|jpg)$/i.test(path)) return null;
  for (const directory of ["build/client", "static"]) {
    try {
      const bytes = await readFile(join(process.cwd(), directory, path.slice(1)));
      if (bytes.length > 200_000) continue;
      const png = await sharp(bytes).resize(58, 58, { fit: "contain" }).png().toBuffer();
      return `data:image/png;base64,${png.toString("base64")}`;
    } catch { /* Try the other packaged asset directory. */ }
  }
  return null;
}

async function paymentDetail(params: URLSearchParams, side: "from" | "to", y: number): Promise<string> {
  const network = params.get(`${side}NetworkName`) || params.get(`${side}Network`);
  const name = network || params.get(`${side}Name`);
  if (!name) return "";
  const icon = await localIcon(network ? networkIcon(network) : params.get(`${side}Icon`) ?? "");
  const label = name.length > 49 ? `${name.slice(0, 48)}…` : name;
  return `${icon ? `<image x="318" y="${y - 20}" width="26" height="26" href="${icon}"/>` : ""}<text x="${icon ? 354 : 318}" y="${y}" fill="#c7d0c3" font-family="DejaVu Sans, sans-serif" font-size="18" font-weight="600">${xml(label)}</text>`;
}

async function assetIcon(currency: string): Promise<string | null> {
  if (!currency || !/^[a-z0-9]{2,12}$/.test(currency.toLowerCase())) return null;
  const key = currency.toLowerCase();
  const candidates = [
    ...(FIAT_FLAGS[currency] ? [`flags/${FIAT_FLAGS[currency]}.svg`] : []),
    `assets/${key}.png`,
    `assets/${key}.webp`,
    `assets/${key}.svg`,
  ];
  for (const directory of ["build/client/icons", "static/icons"]) {
    for (const candidate of candidates) {
      try {
        const bytes = await readFile(join(process.cwd(), directory, candidate));
        if (bytes.length > 200_000) continue;
        const png = await sharp(bytes).resize(58, 58, { fit: "contain" }).png().toBuffer();
        return `data:image/png;base64,${png.toString("base64")}`;
      } catch {
        // Try the next local format or directory.
      }
    }
  }
  return null;
}

async function homepageImage(): Promise<Buffer> {
  let logo: Buffer | null = null;
  for (const directory of ["build/client/icons/assets", "static/icons/assets"]) {
    try {
      logo = await readFile(join(process.cwd(), directory, "pay3flow_logo.png"));
      break;
    } catch {
      // The other directory covers local development and the packaged server.
    }
  }
  if (!logo) throw new Error("Pay3Flow logo is missing");
  const mark = await sharp(logo).resize(260, 260).png().toBuffer();
  return sharp({ create: { width: 1200, height: 630, channels: 4, background: "#101110" } })
    .composite([{ input: mark, left: 470, top: 185 }])
    .png()
    .toBuffer();
}

export const GET: RequestHandler = async ({ url }) => {
  if (!url.searchParams.has("from") || !url.searchParams.has("to")) {
    const png = await homepageImage();
    return new Response(new Uint8Array(png), {
      headers: { "content-type": "image/png", "cache-control": "public, max-age=86400", "content-length": String(png.length) },
    });
  }
  const source = shareCurrency(url.searchParams.get("from") ?? "") ?? "SEND";
  const target = shareCurrency(url.searchParams.get("to") ?? "") ?? "GET";
  const amount = shareAmount(url.searchParams.get("amount"));
  const receive = amount ? shareAmount(url.searchParams.get("receive")) : null;
  const [sourceIcon, targetIcon, sourceDetail, targetDetail] = await Promise.all([
    assetIcon(source), assetIcon(target), paymentDetail(url.searchParams, "from", 278), paymentDetail(url.searchParams, "to", 516),
  ]);
  const sourceSize = !amount ? 28 : amount.length > 18 ? 22 : amount.length > 13 ? 30 : 39;
  const targetSize = !receive ? 25 : receive.length > 18 ? 22 : receive.length > 13 ? 30 : 39;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="1200" height="630" viewBox="0 0 1200 630">
    <rect width="1200" height="630" fill="#101110"/>
    <circle cx="80" cy="80" r="230" fill="#263222" opacity=".55"/>
    <circle cx="1150" cy="590" r="270" fill="#263222" opacity=".45"/>
    <text x="72" y="557" fill="#f4f6f2" font-family="DejaVu Sans, sans-serif" font-size="30" font-weight="800">Pay3Flow</text>
    <text x="74" y="591" fill="#a6afa3" font-family="DejaVu Sans, sans-serif" font-size="18">Live exchange routes</text>
    <rect x="274" y="34" width="652" height="562" rx="26" fill="#070a07" opacity=".9"/>
    <rect x="264" y="26" width="652" height="562" rx="25" fill="#1b1d1b" stroke="#414740" stroke-width="2"/>
    <rect x="292" y="54" width="128" height="42" rx="12" fill="#b5f500"/>
    <text x="319" y="82" fill="#172015" font-family="DejaVu Sans, sans-serif" font-size="19" font-weight="700">Bridge</text>
    <text x="804" y="81" fill="#a6afa3" font-family="DejaVu Sans, sans-serif" font-size="16">Pay3Flow</text>
    <text x="297" y="135" fill="#b1bdad" font-family="DejaVu Sans, sans-serif" font-size="17" font-weight="700">SELL</text>
    <rect x="292" y="151" width="596" height="146" rx="18" fill="#242724" stroke="#414740" stroke-width="2"/>
    <text x="318" y="187" fill="#b3bdb0" font-family="DejaVu Sans, sans-serif" font-size="18">You send</text>
    <text x="318" y="239" fill="#f5f7f3" font-family="DejaVu Sans, sans-serif" font-size="${sourceSize}" font-weight="700">${xml(amount ?? "Choose amount")}</text>
    ${sourceDetail}
    ${avatar(source, 730, 194, sourceIcon)}
    <text x="860" y="234" text-anchor="end" fill="#f5f7f3" font-family="DejaVu Sans, sans-serif" font-size="22" font-weight="700">${xml(source)}</text>
    <line x1="296" y1="327" x2="884" y2="327" stroke="#414740" stroke-width="3"/>
    <circle cx="590" cy="327" r="27" fill="#b5f500" stroke="#8dcb00" stroke-width="2"/>
    <text x="590" y="337" text-anchor="middle" fill="#172015" font-family="DejaVu Sans, sans-serif" font-size="27" font-weight="700">↓</text>
    <text x="297" y="371" fill="#b1bdad" font-family="DejaVu Sans, sans-serif" font-size="17" font-weight="700">BUY</text>
    <rect x="292" y="387" width="596" height="146" rx="18" fill="#242724" stroke="#414740" stroke-width="2"/>
    <text x="318" y="423" fill="#b3bdb0" font-family="DejaVu Sans, sans-serif" font-size="18">Recipient gets</text>
    <text x="318" y="477" fill="#f5f7f3" font-family="DejaVu Sans, sans-serif" font-size="${targetSize}" font-weight="700">${xml(receive ?? "Live quote on open")}</text>
    ${targetDetail}
    ${avatar(target, 730, 432, targetIcon)}
    <text x="860" y="473" text-anchor="end" fill="#f5f7f3" font-family="DejaVu Sans, sans-serif" font-size="22" font-weight="700">${xml(target)}</text>
    <text x="306" y="566" fill="#a6afa3" font-family="DejaVu Sans, sans-serif" font-size="15">${receive ? "Quote at sharing · live routes refresh on open" : "Find current routes and quotes on Pay3Flow"}</text>
  </svg>`;
  const png = await sharp(Buffer.from(svg)).png().toBuffer();
  return new Response(new Uint8Array(png), {
    headers: { "content-type": "image/png", "cache-control": "public, max-age=86400", "content-length": String(png.length) },
  });
};
