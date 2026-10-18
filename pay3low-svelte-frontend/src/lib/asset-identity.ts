/** Preserve links and selections created before the native TON coin became GRAM. */
export function canonicalAssetTicker(value: string): string {
  const ticker = value.trim().toUpperCase();
  return ticker === "TON" ? "GRAM" : ticker;
}
