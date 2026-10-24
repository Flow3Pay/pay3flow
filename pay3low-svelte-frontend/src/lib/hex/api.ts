import { apiUrl } from "$lib/api";

export async function request<T>(path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const response = await fetch(apiUrl(`/api/hex${path}`), { method: body === undefined ? "GET" : "POST", cache: "no-store",
    headers: body === undefined ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body), signal: signal ?? AbortSignal.timeout(15_000) });
  const value = await response.json();
  if (!response.ok) throw new Error(value.error || "HEX request failed");
  return value as T;
}
