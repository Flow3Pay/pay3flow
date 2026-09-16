import { wsUrl } from "./api";

/* -------------------------------------------------------------------------- */
/*  Types — mirrors the Rust `Quote` / `RateIn` / `RateOut` enums.            */
/* -------------------------------------------------------------------------- */

export type RouteSource = "fmatch" | "fallback";

export interface QuoteRequest {
  amount: number;
  currency: string;
  from: string;
  to: string;
}

export interface QuoteCandidate {
  name: string;
  shortId?: string;
  rank: number;
  /** Effective fee ratio as a fraction (0.031 = 3.1 %). */
  price?: number;
  quality?: number;
}

export interface Quote {
  request: QuoteRequest;
  source: RouteSource;
  candidates: QuoteCandidate[];
  best?: QuoteCandidate;
  quotedAt: string;
}

export type RatesStatus = "connecting" | "open" | "closed";

export interface RatesSocket {
  /** Send a quote request to the backend. Returns `false` if the socket is not open. */
  sendQuote(req: QuoteRequest, id?: string): boolean;
  close(): void;
}

/* -------------------------------------------------------------------------- */
/*  WebSocket client                                                          */
/* -------------------------------------------------------------------------- */

const RECONNECT_MS = 2_500;

export function createRatesSocket(handlers: {
  onQuote: (quote: Quote, id?: string) => void;
  onStatus: (status: RatesStatus) => void;
}): RatesSocket {
  let ws: WebSocket | null = null;
  let closedByUser = false;
  let retryTimer: ReturnType<typeof setTimeout> | null = null;

  const setStatus = (s: RatesStatus) => handlers.onStatus(s);

  const connect = () => {
    setStatus("connecting");
    try {
      ws = new WebSocket(wsUrl("/ws/rates"));
    } catch {
      scheduleReconnect();
      return;
    }

    ws.onopen = () => setStatus("open");

    ws.onmessage = (ev) => {
      try {
        const data = JSON.parse(
          typeof ev.data === "string" ? ev.data : "",
        ) as {
          type: string;
          id?: string;
          quote?: Quote;
          message?: string;
        };
        if (data.type === "quote" && data.quote) {
          handlers.onQuote(data.quote, data.id);
        } else if (data.type === "error") {
          console.warn("[rates-ws]", data.message);
        }
      } catch (err) {
        console.error("[rates-ws] bad frame", err);
      }
    };

    ws.onclose = () => {
      setStatus("closed");
      scheduleReconnect();
    };

    ws.onerror = () => ws?.close();
  };

  const scheduleReconnect = () => {
    if (closedByUser || retryTimer !== null) return;
    retryTimer = setTimeout(() => {
      retryTimer = null;
      if (!closedByUser) connect();
    }, RECONNECT_MS);
  };

  connect();

  return {
    sendQuote(request: QuoteRequest, id?: string): boolean {
      if (!ws || ws.readyState !== WebSocket.OPEN) return false;
      ws.send(JSON.stringify({ type: "quote", id, request }));
      return true;
    },
    close() {
      closedByUser = true;
      if (retryTimer !== null) {
        clearTimeout(retryTimer);
        retryTimer = null;
      }
      ws?.close();
      ws = null;
    },
  };
}