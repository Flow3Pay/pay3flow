import { env } from "$env/dynamic/public";

// Keep local development compatible with the standalone backend while allowing
// a production deployment to use its own origin when no API URL is configured.
export const API_BASE_URL = env.PUBLIC_API_URL ?? (import.meta.env.DEV ? "http://localhost:8080" : "");

export function apiUrl(path: string): URL {
  const base = API_BASE_URL || (typeof window !== "undefined" ? window.location.origin : "http://localhost:8080");
  return new URL(path, base);
}

/** Resolve an absolute HTTP path against the backend, with `wss://` for https. */
export function wsUrl(path: string): string {
  const url = apiUrl(path);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}
