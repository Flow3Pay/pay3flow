import assert from "node:assert/strict";
import test from "node:test";
import { OtcClient } from "../src/lib/otc/client.ts";
import { demoCandles, demoSnapshot, markets, otcFallbackNetworks, type OrderDraft } from "../src/lib/otc/model.ts";

class Socket {
  readyState = 0;
  sent: Record<string, unknown>[] = [];
  onopen?: () => void;
  onmessage?: (event: { data: string }) => void;
  onclose?: () => void;
  open() { this.readyState = 1; this.onopen?.(); }
  send(message: string) { this.sent.push(JSON.parse(message)); }
  receive(message: unknown) { this.onmessage?.({ data: JSON.stringify(message) }); }
  close() { this.readyState = 3; this.onclose?.(); }
}
const state = (market = markets[0]) => ({ type: "snapshot", market, snapshot: { ...demoSnapshot(market), mode: "test" },
  candles: Object.fromEntries((["1D", "7D", "1M", "1Y"] as const).map((range) => [range, demoCandles(market, range)])),
  orders: [], networks: otcFallbackNetworks });
const draft: OrderDraft = { marketId: "EVER-USDT", side: "buy", type: "limit", price: 0.01, amount: 100, total: 1, sendNetwork: "Ethereum (ERC-20)", receiveNetwork: "Everscale" };
function setup() {
  const socket = new Socket(), states: unknown[] = [], connections: string[] = [], errors: string[] = [];
  const client = new OtcClient("ws://example/otc", "session", "EVER-USDT", {
    state: (value) => states.push(value), connection: (value) => connections.push(value), error: (value) => errors.push(value),
  }, () => socket as unknown as WebSocket);
  return { socket, states, connections, errors, client };
}
test("requires a snapshot and waits for the matching server acknowledgment", async () => {
  const { client, socket, states } = setup();
  try {
    socket.open();
    assert.deepEqual(socket.sent[0], { type: "subscribe", sessionId: "session", marketId: "EVER-USDT" });
    await assert.rejects(client.createOrder(draft), /disconnected/);
    socket.receive(state()); assert.equal(states.length, 1);
    let completed = false;
    const result = client.createOrder(draft).then((value) => { completed = true; return value; });
    const message = socket.sent.at(-1)!;
    socket.receive({ type: "order", id: "unrelated", order: {} });
    await Promise.resolve(); assert.equal(completed, false);
    const order = { ...draft, id: message.id, createdAt: 1, status: "open" };
    socket.receive({ type: "order", id: message.id, order });
    assert.deepEqual(await result, order);
  } finally { client.close(); }
});
test("disconnect rejects pending operations; explicit retries reuse the creation UUID", async () => {
  const { client, socket, connections } = setup();
  try {
    socket.open(); socket.receive(state());
    const result = client.createOrder(draft), id = socket.sent.at(-1)!.id;
    socket.close(); await assert.rejects(result, /disconnected/);
    assert.equal(connections.at(-1), "reconnecting");
    socket.open(); socket.receive(state());
    const retry = client.createOrder(draft); assert.equal(socket.sent.at(-1)!.id, id);
    socket.receive({ type: "error", id, code: "liquidity" });
    await assert.rejects(retry, /liquidity/);
  } finally { client.close(); }
});
test("stale market snapshots cannot unlock operations after a market change", async () => {
  const { client, socket, states } = setup();
  try {
    socket.open(); socket.receive(state()); client.subscribe("BTC-USDT"); socket.receive(state());
    assert.equal(states.length, 1); await assert.rejects(client.createOrder(draft), /disconnected/);
    socket.receive(state(markets[1])); assert.equal(states.length, 2);
  } finally { client.close(); }
});
test("malformed snapshots are rejected and disposal stops updates", async () => {
  const { client, socket, states, errors, connections } = setup();
  try {
    socket.open(); socket.receive({ ...state(), candles: {} });
    assert.deepEqual(errors, ["invalid_message"]); assert.equal(states.length, 0);
    await assert.rejects(client.createOrder(draft), /disconnected/);
    client.close(); socket.open(); socket.receive(state());
    assert.equal(states.length, 0); assert.equal(connections.at(-1), "closed");
  } finally { client.close(); }
});
