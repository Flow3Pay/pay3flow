/** Convert a positive decimal amount without truncation or floating point. */
export function decimalToAtomic(value: string, decimals: number): bigint {
  if (!Number.isInteger(decimals) || decimals < 0 || decimals > 255) throw new Error("Invalid token precision");
  if (!/^\d+(?:\.\d+)?$/.test(value)) throw new Error("Invalid token amount");
  const [whole, fraction = ""] = value.split(".");
  if (fraction.slice(decimals).replace(/0/g, "")) throw new Error("Amount exceeds token precision");
  return BigInt(`${whole}${fraction.slice(0, decimals).padEnd(decimals, "0")}`);
}

export function atomicToDecimal(value: string, decimals: number): string {
  if (!/^\d+$/.test(value) || !Number.isInteger(decimals) || decimals < 0 || decimals > 255) throw new Error("Invalid atomic amount");
  if (decimals === 0) return BigInt(value).toString();
  const padded = value.padStart(decimals + 1, "0");
  const split = padded.length - decimals;
  const fraction = padded.slice(split).replace(/0+$/, "");
  const whole = BigInt(padded.slice(0, split)).toString();
  return fraction ? `${whole}.${fraction}` : whole;
}

/** EVM addresses are case-insensitive; NEAR and TRON identities are not. */
export function sameWalletAddress(family: "evm" | "near" | "tron", a: string, b: string): boolean {
  return family === "evm" ? a.toLowerCase() === b.toLowerCase() : a === b;
}
