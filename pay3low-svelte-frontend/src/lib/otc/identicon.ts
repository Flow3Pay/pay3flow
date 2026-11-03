/**
 * Deterministic wallet identicons.
 *
 * The backend stores the same image as an SVG data URI. Keeping the generator
 * here as well lets the wallet menu and any optimistic orderbook row render the
 * avatar before a session exists. Inputs are lowercased so an EVM checksum
 * address and the backend's stored form produce the same picture.
 */
const GRID = 5;
const SIZE = 420;
const PADDING = 42;

const MD5_SHIFTS = [
  7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
  5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
  4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
  6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];
const MD5_K = Array.from({ length: 64 }, (_, index) =>
  Math.floor(Math.abs(Math.sin(index + 1)) * 0x100000000) >>> 0,
);

function md5Bytes(input: string): Uint8Array {
  const bytes = new TextEncoder().encode(input);
  const bitLength = bytes.length * 8;
  const paddedLength = (((bytes.length + 8) >> 6) + 1) * 64;
  const buffer = new Uint8Array(paddedLength);
  buffer.set(bytes);
  buffer[bytes.length] = 0x80;
  const view = new DataView(buffer.buffer);
  view.setUint32(paddedLength - 8, bitLength >>> 0, true);
  view.setUint32(paddedLength - 4, Math.floor(bitLength / 0x100000000), true);

  let a0 = 0x67452301;
  let b0 = 0xefcdab89;
  let c0 = 0x98badcfe;
  let d0 = 0x10325476;

  for (let offset = 0; offset < paddedLength; offset += 64) {
    const words = new Array<number>(16);
    for (let index = 0; index < 16; index += 1) {
      words[index] = view.getUint32(offset + index * 4, true);
    }
    let a = a0;
    let b = b0;
    let c = c0;
    let d = d0;
    for (let index = 0; index < 64; index += 1) {
      let f: number;
      let word: number;
      if (index < 16) {
        f = (b & c) | (~b & d);
        word = index;
      } else if (index < 32) {
        f = (d & b) | (~d & c);
        word = (5 * index + 1) % 16;
      } else if (index < 48) {
        f = b ^ c ^ d;
        word = (3 * index + 5) % 16;
      } else {
        f = c ^ (b | ~d);
        word = (7 * index) % 16;
      }
      const sum = (a + f + MD5_K[index] + words[word]) >>> 0;
      const shift = MD5_SHIFTS[index];
      const rotated = ((sum << shift) | (sum >>> (32 - shift))) >>> 0;
      a = d;
      d = c;
      c = b;
      b = (b + rotated) >>> 0;
    }
    a0 = (a0 + a) >>> 0;
    b0 = (b0 + b) >>> 0;
    c0 = (c0 + c) >>> 0;
    d0 = (d0 + d) >>> 0;
  }

  const digest = new Uint8Array(16);
  const digestView = new DataView(digest.buffer);
  digestView.setUint32(0, a0, true);
  digestView.setUint32(4, b0, true);
  digestView.setUint32(8, c0, true);
  digestView.setUint32(12, d0, true);
  return digest;
}

function bits(nibbles: Uint8Array, start: number, length: number): number {
  let value = 0;
  for (let index = start; index < start + length; index += 1) {
    value = (value << 1) | ((nibbles[Math.floor(index / 8)] >> (7 - (index % 8))) & 1);
  }
  return value;
}

function hsl(hue: number, saturation: number, lightness: number): [number, number, number] {
  const s = saturation / 100;
  const l = lightness / 100;
  const chroma = (1 - Math.abs(2 * l - 1)) * s;
  const sector = hue / 60;
  const x = chroma * (1 - Math.abs((sector % 2) - 1));
  let red = 0;
  let green = 0;
  let blue = 0;
  switch (Math.floor(sector)) {
    case 0: red = chroma; green = x; break;
    case 1: red = x; green = chroma; break;
    case 2: green = chroma; blue = x; break;
    case 3: green = x; blue = chroma; break;
    case 4: red = x; blue = chroma; break;
    default: red = chroma; blue = x; break;
  }
  const offset = l - chroma / 2;
  const channel = (value: number) => Math.round(Math.min(255, Math.max(0, (value + offset) * 255)));
  return [channel(red), channel(green), channel(blue)];
}

export function md5Hex(input: string): string {
  return Array.from(md5Bytes(input), byte => byte.toString(16).padStart(2, "0")).join("");
}

export function identiconSvg(address: string): string {
  const digest = md5Bytes(address.trim().toLowerCase());
  const nibbles = new Uint8Array(32);
  digest.forEach((byte, index) => {
    nibbles[index * 2] = byte >> 4;
    nibbles[index * 2 + 1] = byte & 0x0f;
  });

  const grid = Array.from({ length: GRID }, () => Array<boolean>(GRID).fill(false));
  for (let index = 0; index < 15; index += 1) {
    const column = [2, 1, 0][Math.floor(index / GRID)];
    const row = index % GRID;
    const filled = nibbles[index] % 2 === 0;
    grid[row][column] = filled;
    grid[row][GRID - 1 - column] = filled;
  }

  const hue = (bits(nibbles, 100, 12) / 4095) * 360;
  const saturation = 42 + (bits(nibbles, 112, 8) / 255) * 22;
  const lightness = 43 + (bits(nibbles, 120, 8) / 255) * 17;
  const [red, green, blue] = hsl(hue, saturation, lightness);
  const cell = (SIZE - PADDING * 2) / GRID;
  let cells = "";
  for (let row = 0; row < GRID; row += 1) {
    for (let column = 0; column < GRID; column += 1) {
      if (!grid[row][column]) continue;
      cells += `<rect x="${PADDING + column * cell}" y="${PADDING + row * cell}" width="${cell}" height="${cell}"/>`;
    }
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${SIZE}" height="${SIZE}" viewBox="0 0 ${SIZE} ${SIZE}" shape-rendering="crispEdges"><g fill="rgb(${red},${green},${blue})">${cells}</g></svg>`;
}

export function identiconDataUri(address: string): string {
  return `data:image/svg+xml,${encodeURIComponent(identiconSvg(address))}`;
}
