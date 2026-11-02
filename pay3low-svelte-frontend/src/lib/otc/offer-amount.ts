function decimal(value: string, precision: number): string {
  const n=BigInt(value),scale=10n**BigInt(precision);
  const fraction=(n%scale).toString().padStart(precision,"0").replace(/0+$/,"");
  return `${n/scale}${fraction ? `.${fraction}` : ""}`;
}
/** Normalize decimal entry without floating point, including decimal commas. Extra fraction digits are dropped. */
export function offerDecimal(value: string, precision: number): string | null {
  return parse(value, precision)?.amount ?? null;
}
/** Exact base units after flooring; null when malformed, zero, or out of range. */
export function offerUnits(value: string, precision: number): bigint | null {
  return parse(value, precision)?.units ?? null;
}
/** Whether the entered fraction was longer than the asset precision. */
export function offerFloored(value: string, precision: number): boolean {
  return parse(value, precision)?.floored ?? false;
}
/** Base units of a balance the server reports. Zero is a real balance here, so unlike {@link offerUnits} it is not null. */
export function balanceUnits(value: string | null | undefined, precision: number): bigint | null {
  const text = value?.trim() ?? "";
  if (!text || text.length > 80 || !/^\d+(?:\.\d+)?$/.test(text)) return null;
  const [whole, fraction = ""] = text.split(".");
  return BigInt(`${whole}${fraction.slice(0, precision).padEnd(precision, "0")}`);
}
/** Warning only: the entered amount is strictly larger than the wallet holds, so it cannot be paid from it. */
export function offerShortfall(enteredUnits: bigint | null, balance: bigint | null): boolean {
  return enteredUnits !== null && balance !== null && enteredUnits > balance;
}
export const MAX_OFFER_EVER_UNITS = 1_000_000_000_000n * 10n**9n;
export const MAX_OFFER_USDT_UNITS = 1_000_000n * 10n**6n;
function parse(value: string, precision: number): { amount: string; units: bigint; floored: boolean } | null {
  let text = value.trim().replace(",", ".");
  if (text.length > 80 || !/^(?:\d+(?:\.\d*)?|\.\d+)$/.test(text)) return null;
  if (text.startsWith(".")) text = `0${text}`;
  if (text.endsWith(".")) text = text.slice(0,-1);
  const [whole, fraction = ""] = text.split(".");
  const tail = fraction.replace(/0+$/, "");
  const floored = tail.length > precision;
  const kept = tail.slice(0, precision).replace(/0+$/, "");
  const amount = `${BigInt(whole)}${kept ? `.${kept}` : ""}`;
  const units = toUnits(amount, precision);
  return units > 0n && units < 2n**128n ? { amount, units, floored } : null;
}
function toUnits(value: string, precision: number): bigint {
  const [whole, fraction = ""] = value.split(".");
  return BigInt(whole + fraction.padEnd(precision,"0"));
}
export function saleRate(ever: string, usdt: string): string {
  const e=offerDecimal(ever,9),u=offerDecimal(usdt,6);
  return e && u ? decimal((toUnits(u,6)*10n**15n/toUnits(e,9)).toString(),12) : "—";
}
/** Compare two unit prices using the original token units, without rounded rates. */
export function compareOfferRates(everA: string, usdtA: string, everB: string, usdtB: string): number {
  const a = offerDecimal(everA,9), b = offerDecimal(everB,9);
  const u = offerDecimal(usdtA,6), v = offerDecimal(usdtB,6);
  if (!a || !b || !u || !v) return 0;
  const left = toUnits(u,6)*toUnits(b,9), right = toUnits(v,6)*toUnits(a,9);
  return left < right ? -1 : left > right ? 1 : 0;
}
/** USDT totals round down to its six decimals and are reviewed before publication. */
export function offerTotal(ever: string, price: string): { amount: string; rounded: boolean } | null {
  const e=offerDecimal(ever,9),p=offerDecimal(price,12);
  if(!e || !p) return null;
  const product=toUnits(e,9)*toUnits(p,12),scale=10n**15n;
  return { amount:decimal((product/scale).toString(),6),rounded:product%scale!==0n };
}

/** USD reference price as exact 12-decimal units; feed values are doubles, so no float maths continues below. */
function priceUnits(value: number | undefined): bigint | null {
  if (value === undefined || !Number.isFinite(value) || value <= 0 || value >= 1e18) return null;
  return toUnits(value.toFixed(12), 12);
}

/** USDT per EVER from USD reference prices, floored into the twelve decimals the price field accepts. */
export function marketUsdtPerEver(everUsd: number | undefined, usdtUsd: number | undefined): string | null {
  const e = priceUnits(everUsd), u = priceUnits(usdtUsd);
  if (e === null || u === null) return null;
  const rate = e * 10n**12n / u;
  return rate > 0n ? decimal(rate.toString(), 12) : null;
}

/** Calculate EVER from an exact USDT budget and a unit price. */
export function offerEver(usdt: string, price: string): { amount: string; rounded: boolean } | null {
  const u=offerDecimal(usdt,6),p=offerDecimal(price,12);
  if(!u || !p) return null;
  const numerator=toUnits(u,6)*10n**15n,denominator=toUnits(p,12);
  return {amount:decimal((numerator/denominator).toString(),9),rounded:numerator%denominator!==0n};
}

/** One 3% service fee per P2P deal, rounded up to its six USDT decimals. */
export function p2pFeeSplit(usdt: string): { basis: string; fee: string; net: string } | null {
  const units = offerUnits(usdt, 6);
  if (units === null) return null;
  const fee = (units * 3n + 99n) / 100n;
  return { basis: decimal(units.toString(), 6), fee: decimal(fee.toString(), 6), net: decimal((units - fee).toString(), 6) };
}
