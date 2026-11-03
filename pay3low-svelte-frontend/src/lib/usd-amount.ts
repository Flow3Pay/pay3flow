/** Helpers for money inputs that can switch between token units and USD. */

/** Keep digits and a single decimal separator, accepting decimal commas and typographic dots. */
export function sanitizeDecimalInput(value: string): string {
  const sanitized = value
    .normalize("NFKC")
    .replace(/[\u00a0\u200b-\u200d\ufeff]/g, "")
    .replace(/[бБ]/g, ",")
    .replace(/[юЮ٫٬]/g, ".")
    .replace(/[^0-9.,]/g, "");
  const separator = Math.max(sanitized.lastIndexOf("."), sanitized.lastIndexOf(","));
  if (separator < 0) return sanitized.replace(/^0+(?=\d)/, "");
  const integer = sanitized.slice(0, separator).replace(/[.,]/g, "").replace(/^0+(?=\d)/, "");
  return `${integer}.${sanitized.slice(separator + 1).replace(/[.,]/g, "")}`;
}

/** Parse a token or USD amount without treating an empty field as zero. */
export function decimalNumber(value: string): number | null {
  const normalized = value.trim().replace(",", ".");
  if (!/^(?:\d+(?:\.\d*)?|\.\d+)$/.test(normalized)) return null;
  const number = Number(normalized);
  return Number.isFinite(number) ? number : null;
}

/** USD value of a token quantity, or null while the price is unknown or the amount is empty. */
export function usdForTokenAmount(value: string, usdPrice: number): number | null {
  const quantity = decimalNumber(value);
  if (quantity === null || quantity <= 0 || !Number.isFinite(usdPrice) || usdPrice <= 0) return null;
  const total = quantity * usdPrice;
  return Number.isFinite(total) && total > 0 ? total : null;
}

/** Token quantity for a USD amount, formatted for the existing amount field. */
export function tokenAmountForUsd(value: string, usdPrice: number, maxFractionDigits = 8): string {
  const usd = decimalNumber(value);
  if (usd === null || usd <= 0 || !Number.isFinite(usdPrice) || usdPrice <= 0) return "";
  const amount = usd / usdPrice;
  if (!Number.isFinite(amount) || amount <= 0) return "";
  return amount.toLocaleString("en-US", { maximumFractionDigits: maxFractionDigits, useGrouping: false });
}

/** USD shown inside the amount field, without grouping and always with cents. */
export function formatUsdAmount(value: number | null): string {
  if (value === null || !Number.isFinite(value) || value <= 0) return "";
  return value.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2, useGrouping: false });
}

/** The small market-value line under an amount field, e.g. `< $0.01` or `$99.85`. */
export function formatUsdMarketValue(value: number | null): string {
  if (value === null || !Number.isFinite(value) || value <= 0) return "";
  if (value < 0.01) return "<$0.01";
  return `$${value.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

/** The same line when the input itself is in USD, e.g. `99.85 USDT` or `95,169.7 EVER`. */
export function formatTokenMarketValue(value: number | null, symbol: string): string {
  if (value === null || !Number.isFinite(value) || value <= 0) return "";
  if (value < 0.00000001) return `<0.00000001 ${symbol}`;
  return `${value.toLocaleString("en-US", { maximumFractionDigits: 8 })} ${symbol}`;
}
