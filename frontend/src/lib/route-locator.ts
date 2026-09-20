import { P2pOffer, RouteCandidate } from "./exchange";

const LOCATOR_PREFIX = "#/locator/";

function encode(value: string): string {
  const bytes = new TextEncoder().encode(value);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function decode(value: string): string {
  const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
  const padded = normalized + "=".repeat((4 - (normalized.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

function isOffer(value: unknown): value is P2pOffer {
  if (!value || typeof value !== "object") return false;
  const offer = value as Partial<P2pOffer>;
  return typeof offer.source === "string"
    && typeof offer.ad_id === "string"
    && typeof offer.fiat === "string"
    && typeof offer.asset === "string"
    && typeof offer.price === "string"
    && typeof offer.source_url === "string"
    && Boolean(offer.advertiser && typeof offer.advertiser === "object");
}

function isRoute(value: unknown): value is RouteCandidate {
  if (!value || typeof value !== "object") return false;
  const route = value as Partial<RouteCandidate>;
  return typeof route.route_id === "string"
    && typeof route.source_currency === "string"
    && typeof route.entry_asset === "string"
    && typeof route.target_currency === "string"
    && typeof route.source_amount_minor === "number"
    && typeof route.target_amount_minor === "number"
    && Array.isArray(route.legs)
    && (route.entry_offer_snapshot == null || isOffer(route.entry_offer_snapshot))
    && (route.exit_offer_snapshot == null || isOffer(route.exit_offer_snapshot));
}

export function routeLocatorPath(route: RouteCandidate): string {
  return `${LOCATOR_PREFIX}${encode(JSON.stringify({ version: 1, route }))}`;
}

export function routeLocatorUrl(route: RouteCandidate): string {
  if (typeof window === "undefined") return routeLocatorPath(route);
  return `${window.location.origin}${window.location.pathname}${window.location.search}${routeLocatorPath(route)}`;
}

export function routeFromLocatorHash(hash: string): RouteCandidate | null {
  if (!hash.startsWith(LOCATOR_PREFIX)) return null;
  try {
    const payload = JSON.parse(decode(hash.slice(LOCATOR_PREFIX.length))) as { version?: number; route?: unknown };
    return payload.version === 1 && isRoute(payload.route) ? payload.route : null;
  } catch {
    return null;
  }
}
