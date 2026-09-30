import { apiUrl } from "$lib/api";

export const ANONYMOUS_ID_STORAGE_KEY = "pay3flow.anonymous.user-id";
const LEGACY_ANONYMOUS_ID_STORAGE_KEY = "pay3flow.reputation.anonymous-id";
const UUID_PATTERN = /^[0-9a-f-]{36}$/i;

/** Return the only browser identifier used for anonymous usage metrics. */
export function getAnonymousUserId(): string | null {
  if (typeof window === "undefined") return null;

  try {
    const saved = localStorage.getItem(ANONYMOUS_ID_STORAGE_KEY)
      ?? localStorage.getItem(LEGACY_ANONYMOUS_ID_STORAGE_KEY);
    if (saved && UUID_PATTERN.test(saved)) {
      localStorage.setItem(ANONYMOUS_ID_STORAGE_KEY, saved);
      localStorage.removeItem(LEGACY_ANONYMOUS_ID_STORAGE_KEY);
      return saved;
    }

    const created = crypto.randomUUID();
    localStorage.setItem(ANONYMOUS_ID_STORAGE_KEY, created);
    return created;
  } catch {
    // Private browsing or a disabled storage API should not block the app.
    return null;
  }
}

/** Add the pseudonymous ID to an API request without adding any other data. */
export function anonymousHeaders(init?: HeadersInit): Headers {
  const headers = new Headers(init);
  const anonymousId = getAnonymousUserId();
  if (anonymousId) headers.set("X-Anonymous-User-Id", anonymousId);
  return headers;
}

/** Persist the browser ID in the backend. Registration is intentionally silent. */
export async function registerAnonymousUser(anonymousId = getAnonymousUserId()): Promise<void> {
  if (!anonymousId) return;

  const response = await fetch(apiUrl("/api/anonymous/register"), {
    method: "POST",
    headers: anonymousHeaders({ "Content-Type": "application/json" }),
    body: JSON.stringify({ anonymous_id: anonymousId }),
  });
  if (!response.ok) throw new Error(`Anonymous registration failed (${response.status})`);
}
