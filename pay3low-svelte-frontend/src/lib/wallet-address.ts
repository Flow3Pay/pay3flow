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

/** Solana account keys are non-zero 32-byte base58 values. */
export function isSolanaAddress(address: string): boolean {
  if (!/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(address)) return false;
  const alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  let value = 0n;
  for (const char of address) value = value * 58n + BigInt(alphabet.indexOf(char));
  if (!value) return false;
  const leadingZeros = address.length - address.replace(/^1+/, "").length;
  return leadingZeros + Math.ceil(value.toString(16).length / 2) === 32;
}

const EVM_WALLET_NETWORKS = new Set([
  "ethereum", "gnosis", "arbitrum-one", "base", "polygon-pos",
  "avalanche-c", "bnb-smart-chain", "optimism", "sei",
]);

function tonCrc16(bytes: Uint8Array): number {
  let crc = 0;
  for (const byte of bytes) {
    crc ^= byte << 8;
    for (let bit = 0; bit < 8; bit += 1) crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff;
  }
  return crc;
}

/** TON addresses are either `workchain:hex` or a 36-byte URL-safe base64 user-friendly form with a CRC16. */
export function isTonAddress(address: string): boolean {
  if (address.includes(":")) {
    const [workchain, hash] = address.split(":");
    return (workchain === "0" || workchain === "-1") && /^[0-9a-fA-F]{64}$/.test(hash);
  }
  if (!/^[A-Za-z0-9_-]{48}$/.test(address)) return false;
  let bytes: Uint8Array;
  try {
    const base64 = address.replace(/-/g, "+").replace(/_/g, "/");
    const binary = atob(base64);
    bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
  } catch {
    return false;
  }
  if (bytes.length !== 36 || (bytes[0] !== 0x11 && bytes[0] !== 0x51)) return false;
  return tonCrc16(bytes.slice(0, 34)) === ((bytes[34] << 8) | bytes[35]);
}

/** Mirror the backend's per-network OTC receiving-address validation. */
export function isValidWalletAddress(network: string, address: string): boolean {
  const value = address.trim();
  if (!value) return false;
  if (EVM_WALLET_NETWORKS.has(network)) {
    return /^0x[0-9a-fA-F]{40}$/.test(value) && !/^0x0{40}$/i.test(value);
  }
  switch (network) {
    case "near":
      return /^(?=.{2,64}$)[a-z0-9]+(?:[._-][a-z0-9]+)*$/.test(value);
    case "tron":
      return /^T[1-9A-HJ-NP-Za-km-z]{33}$/.test(value);
    case "solana":
      return isSolanaAddress(value);
    case "sui": {
      const hex = value.replace(/^0x/, "");
      return hex.length === 64 && /^[0-9a-fA-F]+$/.test(hex) && !/^0+$/.test(hex);
    }
    case "aptos": {
      const hex = value.replace(/^0x/, "");
      return hex.length >= 1 && hex.length <= 64 && /^[0-9a-fA-F]+$/.test(hex) && !/^0+$/.test(hex);
    }
    case "ton":
      return isTonAddress(value);
    default:
      return false;
  }
}
