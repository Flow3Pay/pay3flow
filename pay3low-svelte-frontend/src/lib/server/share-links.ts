import { env } from '$env/dynamic/private';
import { dev } from '$app/environment';
import { error } from '@sveltejs/kit';
import type { OtcPreview } from '$lib/share-links';

export interface StoredShare { id: string; target: string; preview: OtcPreview | null }

export async function shareApi(path: string, options?: RequestInit): Promise<StoredShare> {
  const base = env.SHARE_API_URL ?? (dev ? 'http://127.0.0.1:8080' : 'http://pay3flow-backend:8080');
  let response: Response;
  try { response = await fetch(new URL(path, base), { ...options, signal: AbortSignal.timeout(8000) }); }
  catch { error(503, 'Share links are temporarily unavailable'); }
  if (response.status === 404) error(404, 'Share link not found');
  if (response.status === 400 || response.status === 422) error(400, 'Invalid share settings');
  if (!response.ok) error(503, 'Share links are temporarily unavailable');
  return response.json();
}

export function readShare(id: string) {
  if (!/^[A-Za-z0-9_-]{16}$/.test(id)) error(404, 'Share link not found');
  return shareApi(`/api/share-links/${id}`);
}
