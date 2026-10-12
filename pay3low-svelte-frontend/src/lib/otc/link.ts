import type { OrderSide, OrderType } from "./model";

export interface OtcLinkState {
  marketId: string;
  side: OrderSide;
  type: OrderType;
  amount: string;
  price: string;
  sendNetworkId: string;
  receiveNetworkId: string;
}

// Preserve decimal precision (including empty fields) without floating-point conversion.
function decimal(value: string | null): string | undefined {
  if (value === null) return undefined;
  const normalized = value.trim().replace(",", ".");
  if (normalized.length > 64 || !/^(?:\d+(?:\.\d*)?|\.\d+)?$/.test(normalized)) return undefined;
  return normalized === "" || Number(normalized) <= 1e12 ? normalized : undefined;
}

function network(value: string | null): string | undefined {
  return value !== null && /^[a-z0-9][a-z0-9_-]{0,79}$/.test(value) ? value : undefined;
}

export function parseOtcHash(hash: string): Partial<OtcLinkState> {
  if (!/^#\/otc(?:\?|$)/.test(hash)) return {};
  const params = new URLSearchParams(hash.split("?")[1] ?? "");
  const state: Partial<OtcLinkState> = {};
  const marketId = params.get("market");
  if (marketId && /^[A-Z0-9]+-[A-Z0-9]+$/.test(marketId) && marketId.length <= 40) state.marketId = marketId;
  const side = params.get("side"), type = params.get("type");
  if (side === "buy" || side === "sell") state.side = side;
  if (type === "limit" || type === "market") state.type = type;
  const amount = decimal(params.get("amount")), price = decimal(params.get("price"));
  if (amount !== undefined) state.amount = amount;
  if (price !== undefined) state.price = price;
  const sendNetworkId = network(params.get("sendNetwork")), receiveNetworkId = network(params.get("receiveNetwork"));
  if (sendNetworkId) state.sendNetworkId = sendNetworkId;
  if (receiveNetworkId) state.receiveNetworkId = receiveNetworkId;
  return state;
}

export function otcHash(state: OtcLinkState): string {
  const params = new URLSearchParams({ market: state.marketId, side: state.side, type: state.type });
  const amount = decimal(state.amount), price = decimal(state.price);
  if (amount !== undefined) params.set("amount", amount);
  if (price !== undefined) params.set("price", price);
  if (network(state.sendNetworkId)) params.set("sendNetwork", state.sendNetworkId);
  if (network(state.receiveNetworkId)) params.set("receiveNetwork", state.receiveNetworkId);
  return `#/otc?${params}`;
}
