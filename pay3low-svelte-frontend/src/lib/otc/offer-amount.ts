function decimal(value: string, precision: number): string {
  const n=BigInt(value),scale=10n**BigInt(precision);
  const fraction=(n%scale).toString().padStart(precision,"0").replace(/0+$/,"");
  return `${n/scale}${fraction ? `.${fraction}` : ""}`;
}
/** Normalize decimal entry without floating point, including decimal commas. */
export function offerDecimal(value: string, precision: number): string | null {
  let text = value.trim().replace(",", ".");
  if (text.length > 80 || !/^(?:\d+(?:\.\d*)?|\.\d+)$/.test(text)) return null;
  if (text.startsWith(".")) text = `0${text}`;
  if (text.endsWith(".")) text = text.slice(0,-1);
  const [whole, fraction = ""] = text.split(".");
  const tail = fraction.replace(/0+$/, "");
  const normalized = `${BigInt(whole)}${tail ? `.${tail}` : ""}`;
  if (tail.length > precision) return null;
  const amount = units(normalized,precision);
  return amount > 0n && amount < 2n**128n ? normalized : null;
}
function units(value: string, precision: number): bigint {
  const [whole, fraction = ""] = value.split(".");
  return BigInt(whole + fraction.padEnd(precision,"0"));
}
export function validOfferAmount(value: string, precision: 6 | 9, max: bigint): boolean {
  const normalized=offerDecimal(value,precision);
  return !!normalized && units(normalized,precision)<=max;
}
export function saleRate(ever: string, usdt: string): string {
  const e=offerDecimal(ever,9),u=offerDecimal(usdt,6);
  return e && u ? decimal((units(u,6)*10n**15n/units(e,9)).toString(),12) : "—";
}
/** Compare two unit prices using the original token units, without rounded rates. */
export function compareOfferRates(everA: string, usdtA: string, everB: string, usdtB: string): number {
  const a = offerDecimal(everA,9), b = offerDecimal(everB,9);
  const u = offerDecimal(usdtA,6), v = offerDecimal(usdtB,6);
  if (!a || !b || !u || !v) return 0;
  const left = units(u,6)*units(b,9), right = units(v,6)*units(a,9);
  return left < right ? -1 : left > right ? 1 : 0;
}
/** USDT totals round down to its six decimals and are reviewed before publication. */
export function offerTotal(ever: string, price: string): { amount: string; rounded: boolean } | null {
  const e=offerDecimal(ever,9),p=offerDecimal(price,12);
  if(!e || !p) return null;
  const product=units(e,9)*units(p,12),scale=10n**15n;
  return { amount:decimal((product/scale).toString(),6),rounded:product%scale!==0n };
}

/** Calculate EVER from an exact USDT budget and a unit price. */
export function offerEver(usdt: string, price: string): { amount: string; rounded: boolean } | null {
  const u=offerDecimal(usdt,6),p=offerDecimal(price,12);
  if(!u || !p) return null;
  const numerator=units(u,6)*10n**15n,denominator=units(p,12);
  return {amount:decimal((numerator/denominator).toString(),9),rounded:numerator%denominator!==0n};
}
