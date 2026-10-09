/** Validate a TRON Base58Check address without requiring a connected wallet. */
export async function isTronAddress(address: string): Promise<boolean> {
  if (!/^T[1-9A-HJ-NP-Za-km-z]{33}$/.test(address)) return false;
  const alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  let value = 0n;
  for (const char of address) value = value * 58n + BigInt(alphabet.indexOf(char));
  const hex = value.toString(16).padStart(50, "0");
  if (hex.length !== 50) return false;
  const bytes = Uint8Array.from(hex.match(/../g)!, byte => parseInt(byte, 16));
  if (bytes[0] !== 0x41) return false;
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", await crypto.subtle.digest("SHA-256", bytes.slice(0, 21))));
  return bytes.slice(21).every((byte, index) => byte === hash[index]);
}
