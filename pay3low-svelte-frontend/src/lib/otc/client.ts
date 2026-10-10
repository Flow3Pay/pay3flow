import type { Candle, ChartRange, DemoOrder, OrderDraft, OtcMarket, OtcSnapshot } from "./model";
import type { CryptoNetwork } from "../networks";

export type Connection = "connecting" | "connected" | "reconnecting" | "closed";
export interface OtcState {
  market: OtcMarket;
  snapshot: OtcSnapshot;
  candles: Record<ChartRange, Candle[]>;
  orders: DemoOrder[];
  networks: CryptoNetwork[];
}
interface Callbacks {
  state: (state: OtcState) => void;
  connection: (connection: Connection) => void;
  error: (code: string) => void;
}
interface Pending {
  resolve: (order: DemoOrder) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}
const ranges: ChartRange[] = ["1D", "7D", "1M", "1Y"];
const positive = (value: unknown): value is number => typeof value === "number" && Number.isFinite(value) && value > 0;
function validOrder(order: DemoOrder): boolean {
  return !!order && typeof order.id === "string" && typeof order.marketId === "string"
    && ["buy", "sell"].includes(order.side) && ["limit", "market"].includes(order.type)
    && ["open", "cancelled", "simulated"].includes(order.status)
    && positive(order.amount) && positive(order.price) && positive(order.createdAt);
}
function validState(state: OtcState): boolean {
  return !!state.market && typeof state.market.id === "string" && positive(state.market.price)
    && !!state.snapshot && state.snapshot.mode === "test" && state.snapshot.marketId === state.market.id
    && [state.snapshot.bids, state.snapshot.asks].every((levels) => Array.isArray(levels) && levels.length > 0
      && levels.every((level) => positive(level.price) && positive(level.amount) && positive(level.depth)))
    && Array.isArray(state.snapshot.trades) && state.snapshot.trades.every((trade) => positive(trade.price) && positive(trade.amount) && positive(trade.time))
    && !!state.candles && ranges.every((range) => Array.isArray(state.candles[range]) && state.candles[range].length > 0
      && state.candles[range].every((candle) => [candle.time, candle.open, candle.close, candle.high, candle.low, candle.volume].every(positive)))
    && Array.isArray(state.orders) && state.orders.every(validOrder)
    && Array.isArray(state.networks) && state.networks.every((network) => typeof network.id === "string" && typeof network.name === "string" && Array.isArray(network.currencies));
}

/** One WS for OTC data and mutations. Mutations are never automatically replayed. */
export class OtcClient {
  private socket: WebSocket | null = null;
  private disposed = false;
  private ready = false;
  private attempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | undefined;
  private watchdog: ReturnType<typeof setTimeout> | undefined;
  private pending = new Map<string, Pending>();
  private createIds = new WeakMap<OrderDraft, string>();

  private url: string;
  private sessionId: string;
  private marketId: string;
  private callbacks: Callbacks;
  private openSocket: (url: string) => WebSocket;
  constructor(url: string, sessionId: string, marketId: string,
    callbacks: Callbacks, openSocket: (url: string) => WebSocket = (url) => new WebSocket(url)) {
    this.url = url; this.sessionId = sessionId; this.marketId = marketId;
    this.callbacks = callbacks; this.openSocket = openSocket;
    this.connect();
  }
  subscribe(marketId: string) {
    this.marketId = marketId;
    this.ready = false;
    this.callbacks.connection("connecting");
    if (this.socket?.readyState === 1) this.sendSubscription();
  }
  createOrder(draft: OrderDraft): Promise<DemoOrder> {
    let id = this.createIds.get(draft);
    if (!id) { id = crypto.randomUUID(); this.createIds.set(draft, id); }
    const { total: _total, ...order } = draft;
    return this.request(id, { type: "create", id, order });
  }
  cancelOrder(orderId: string): Promise<DemoOrder> {
    const id = crypto.randomUUID();
    return this.request(id, { type: "cancel", id, orderId });
  }
  close() {
    this.disposed = true;
    this.ready = false;
    clearTimeout(this.reconnectTimer);
    clearTimeout(this.watchdog);
    this.failPending("disconnected");
    this.socket?.close();
    this.callbacks.connection("closed");
  }
  private request(id: string, message: object): Promise<DemoOrder> {
    if (!this.ready || this.socket?.readyState !== 1) return Promise.reject(new Error("disconnected"));
    if (this.pending.has(id)) return Promise.reject(new Error("pending"));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error("timeout"));
        // A reply may have been lost after commit; reconnect to recover the server state.
        this.socket?.close();
      }, 15000);
      this.pending.set(id, { resolve, reject, timer });
      try { this.socket!.send(JSON.stringify(message)); }
      catch { this.socket?.close(); this.failPending("disconnected"); }
    });
  }
  private sendSubscription() {
    this.socket?.send(JSON.stringify({ type: "subscribe", sessionId: this.sessionId, marketId: this.marketId }));
  }
  private armWatchdog() {
    clearTimeout(this.watchdog);
    this.watchdog = setTimeout(() => this.socket?.close(), 20000);
  }
  private connect() {
    if (this.disposed) return;
    this.callbacks.connection(this.attempts ? "reconnecting" : "connecting");
    let socket: WebSocket;
    try { socket = this.openSocket(this.url); }
    catch { this.scheduleReconnect(); return; }
    this.socket = socket;
    this.armWatchdog();
    socket.onopen = () => { if (!this.disposed && socket === this.socket) this.sendSubscription(); };
    socket.onmessage = (event) => {
      if (this.disposed || socket !== this.socket) return;
      this.armWatchdog();
      try {
        const message = JSON.parse(event.data);
        if (message.type === "snapshot") {
          if (!validState(message)) throw new Error("invalid_message");
          if (message.market.id !== this.marketId) return;
          this.ready = true; this.attempts = 0;
          this.callbacks.state(message);
          this.callbacks.connection("connected");
        } else if (message.type === "order" || message.type === "error") {
          const request = this.pending.get(message.id);
          if (request) {
            clearTimeout(request.timer); this.pending.delete(message.id);
            if (message.type === "error") request.reject(new Error(message.code));
            else if (validOrder(message.order)) request.resolve(message.order);
            else { request.reject(new Error("invalid_message")); throw new Error("invalid_message"); }
          } else if (message.type === "error") this.callbacks.error(message.code);
        }
      } catch { this.callbacks.error("invalid_message"); socket.close(); }
    };
    socket.onerror = () => socket.close();
    socket.onclose = () => {
      if (socket !== this.socket) return;
      this.ready = false;
      clearTimeout(this.watchdog);
      this.failPending("disconnected");
      this.scheduleReconnect();
    };
  }
  private scheduleReconnect() {
    if (this.disposed) return;
    this.callbacks.connection("reconnecting");
    const delay = Math.min(1000 * 2 ** this.attempts++, 10000);
    this.reconnectTimer = setTimeout(() => this.connect(), delay);
  }
  private failPending(code: string) {
    for (const request of this.pending.values()) { clearTimeout(request.timer); request.reject(new Error(code)); }
    this.pending.clear();
  }
}
