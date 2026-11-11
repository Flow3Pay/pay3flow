const AUTH_TOKEN_KEY = "pay3flow.auth.token";

/** Read the browser session token used by authenticated account pages. */
export function readAuthToken(): string | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage.getItem(AUTH_TOKEN_KEY);
  } catch {
    return null;
  }
}

/** Persist the token returned by `/api/auth/authenticate`. */
export function storeAuthToken(token: string): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(AUTH_TOKEN_KEY, token);
  } catch {
    /* A blocked storage keeps the session for this page only. */
  }
}

export function clearAuthToken(): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(AUTH_TOKEN_KEY);
  } catch {
    /* Nothing to clear when storage is blocked. */
  }
}
