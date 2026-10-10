export interface OtcPreview {
  closes: number[];
  bids: { price: number; amount: number }[];
  asks: { price: number; amount: number }[];
  capturedAt: number;
}

export async function createShortShare(target: string, preview?: OtcPreview | null): Promise<string> {
  const response = await fetch('/share-links', {
    method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ target, ...(preview ? { preview } : {}) }),
  });
  if (!response.ok) throw new Error('Share link unavailable');
  const { id } = await response.json();
  if (typeof id !== 'string' || !/^[A-Za-z0-9_-]{16}$/.test(id)) throw new Error('Invalid share link');
  return `${location.origin}/${target.startsWith('/#/otc?') ? 'otc' : 's'}/${id}`;
}
