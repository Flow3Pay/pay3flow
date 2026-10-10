import type { Page } from "@playwright/test";
import { demoCandles, demoSnapshot, markets, otcFallbackNetworks, type DemoOrder, type OtcMarket } from "../src/lib/otc/model";

export function testOtcSnapshot(market: OtcMarket, orders: DemoOrder[] = []) {
  return { type: "snapshot", mode: "test", market, snapshot: { ...demoSnapshot(market), mode: "test" },
    candles: Object.fromEntries((["1D", "7D", "1M", "1Y"] as const).map((range) => [range, demoCandles(market, range)])),
    orders, networks: otcFallbackNetworks };
}

/** Deterministic test peer; the production workspace never uses these generators. */
export async function mockOtcSocket(page: Page) {
  const sessions = new Map<string, DemoOrder[]>();
  await page.routeWebSocket("**/ws/otc", (socket) => {
    let session = "", market = markets[0];
    let timer: ReturnType<typeof setInterval> | undefined;
    const snapshot = () => socket.send(JSON.stringify(testOtcSnapshot(market, sessions.get(session))));
    socket.onMessage((raw) => {
      const message = JSON.parse(String(raw));
      if (message.type === "subscribe") {
        session = message.sessionId;
        market = markets.find((item) => item.id === message.marketId)!;
        if (!sessions.has(session)) sessions.set(session, []);
        snapshot();
        clearInterval(timer); timer = setInterval(snapshot, 5000);
      } else if (message.type === "create") {
        const orders = sessions.get(session)!;
        const order: DemoOrder = orders.find((item) => item.id === message.id) ?? {
          ...message.order, id: message.id, createdAt: Date.now(), status: message.order.type === "market" ? "simulated" : "open",
        };
        if (!orders.includes(order)) orders.unshift(order);
        socket.send(JSON.stringify({ type: "order", id: message.id, order }));
      } else if (message.type === "cancel") {
        const order = sessions.get(session)!.find((item) => item.id === message.orderId)!;
        order.status = "cancelled";
        socket.send(JSON.stringify({ type: "order", id: message.id, order }));
      }
    });
    socket.onClose(() => clearInterval(timer));
  });
}
