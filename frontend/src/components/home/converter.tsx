"use client";

import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  ConfirmOrderResponse,
  ExchangeCorridor,
  ExchangeOrder,
  FundingInstruction,
  LiveRouteEvent,
  RouteCandidate,
  authenticate,
  confirmFunding,
  confirmOrder,
  createOrder,
  fetchCorridors,
  fetchOrders,
  fetchQuotes,
  openLiveRoutes,
  quoteToCandidate,
  submitMockProof,
} from "@/lib/exchange";

import { SidePanel } from "./side-panel";
import styles from "./converter.module.css";

const STATUS_EN: Record<string, string> = {
  created: "Order created",
  discovering: "Finding providers",
  quoting: "Building routes",
  quoted: "Route selected",
  locked: "Route locked",
  token_settling: "Processing settlement",
  money_settling: "Sending funds to recipient",
  proof_pending: "Verifying confirmation",
  done: "Transfer completed",
  failed: "Transfer failed",
  expired: "Order expired",
  cancelled: "Order cancelled",
  disputed: "Manual review required",
};

const money = (minor: number, currency: string) =>
  `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency}`;

const locationKey = (country: string, currency: string) => `${country}:${currency}`;

const METHOD_LABELS: Record<string, string> = {
  bank_card: "Bank card",
  bank_transfer: "Bank transfer",
  p2p: "P2P",
  wallet: "Wallet",
};

const methodLabel = (method: string) => METHOD_LABELS[method] ?? method;

function locationLabel(country: string, currency: string): string {
  try {
    const region = new Intl.DisplayNames(["en"], { type: "region" }).of(country);
    return `${region ?? country} · ${currency}`;
  } catch {
    return `${country} · ${currency}`;
  }
}

function upsertRoute(routes: RouteCandidate[], fresh: RouteCandidate): RouteCandidate[] {
  const existing = routes.findIndex((route) => route.route_id === fresh.route_id);
  if (existing < 0) return [...routes, fresh];
  return routes.map((route, index) => (index === existing ? { ...route, ...fresh } : route));
}

interface ConverterProps {
  token: string | null;
  email: string;
  onAuthenticated: (token: string, email: string) => void;
  onRequireAuth: () => void;
}

export function Converter({ token, email: sessionEmail, onAuthenticated, onRequireAuth }: ConverterProps) {
  const [corridors, setCorridors] = useState<ExchangeCorridor[]>([]);
  const [corridorId, setCorridorId] = useState("");
  const [termsVersion, setTermsVersion] = useState("");
  const [amount, setAmount] = useState("100000");
  const [sourceMethod, setSourceMethod] = useState("bank_card");
  const [targetMethod, setTargetMethod] = useState("bank_card");
  const [recipient, setRecipient] = useState("");
  const [authEmail, setAuthEmail] = useState(sessionEmail || "demo@pay3flow.dev");
  const [authCode, setAuthCode] = useState("1234");
  const [authBusy, setAuthBusy] = useState(false);
  const [order, setOrder] = useState<ExchangeOrder | null>(null);
  const [routes, setRoutes] = useState<RouteCandidate[]>([]);
  const [selected, setSelected] = useState<RouteCandidate | null>(null);
  const [funding, setFunding] = useState<FundingInstruction | null>(null);
  const [settlement, setSettlement] = useState<{ id: string; solver_id: string } | null>(null);
  const [consent, setConsent] = useState(false);
  const [searching, setSearching] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [socketFallback, setSocketFallback] = useState(false);
  const [history, setHistory] = useState<ExchangeOrder[]>([]);
  const closeSocket = useRef<(() => void) | null>(null);

  const corridor = useMemo(
    () => corridors.find((item) => item.id === corridorId) ?? corridors[0],
    [corridorId, corridors],
  );

  const sourceLocations = useMemo(
    () => [
      ...new Map(
        corridors.map((item) => [
          locationKey(item.source_country, item.source_currency),
          { country: item.source_country, currency: item.source_currency },
        ]),
      ).values(),
    ],
    [corridors],
  );

  const targetLocations = useMemo(() => {
    const compatible = corridor
      ? corridors.filter(
          (item) =>
            item.source_country === corridor.source_country &&
            item.source_currency === corridor.source_currency,
        )
      : corridors;
    return [
      ...new Map(
        compatible.map((item) => [
          locationKey(item.target_country, item.target_currency),
          { country: item.target_country, currency: item.target_currency },
        ]),
      ).values(),
    ];
  }, [corridor, corridors]);

  useEffect(() => {
    fetchCorridors()
      .then((response) => {
        setCorridors(response.items);
        setCorridorId((current) => current || response.items[0]?.id || "");
        setTermsVersion(response.terms_version);
      })
      .catch((cause: Error) => setError(cause.message));
  }, []);

  const refreshHistory = useCallback(() => {
    if (!token) return;
    fetchOrders(token).then(setHistory).catch(() => undefined);
  }, [token]);

  useEffect(refreshHistory, [refreshHistory]);
  useEffect(() => () => closeSocket.current?.(), []);

  useEffect(() => {
    if (token) return;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previousOverflow;
    };
  }, [token]);

  const chooseSourceLocation = (value: string) => {
    const currentTarget = corridor
      ? locationKey(corridor.target_country, corridor.target_currency)
      : null;
    const matchingCorridors = corridors.filter(
      (item) => locationKey(item.source_country, item.source_currency) === value,
    );
    const next =
      matchingCorridors.find(
        (item) => locationKey(item.target_country, item.target_currency) === currentTarget,
      ) ?? matchingCorridors[0];
    if (next) setCorridorId(next.id);
  };

  const chooseTargetLocation = (value: string) => {
    const next = corridors.find(
      (item) =>
        item.source_country === corridor?.source_country &&
        item.source_currency === corridor?.source_currency &&
        locationKey(item.target_country, item.target_currency) === value,
    );
    if (next) setCorridorId(next.id);
  };

  const handleLiveEvent = useCallback((event: LiveRouteEvent) => {
    if (event.type === "entry_leg_found" || event.type === "route_candidate_found") {
      setRoutes((current) => upsertRoute(current, event));
      return;
    }
    if (event.type === "best_route_updated") {
      setRoutes((current) =>
        current.map((route) => ({ ...route, is_current_best: route.quote_id === event.quote_id })),
      );
      return;
    }
    if (event.type === "order_status") {
      setOrder((current) => (current ? { ...current, status: event.status } : current));
      return;
    }
    if (event.type === "search_finished") {
      setSearching(false);
      return;
    }
    if (event.type === "search_failed") {
      setSearching(false);
      setError(event.error);
    }
  }, []);

  useEffect(() => {
    if (!socketFallback || !token || !order || !searching) return;
    const timer = window.setInterval(() => {
      fetchQuotes(token, order.id)
        .then((quotes) => setRoutes(quotes.map(quoteToCandidate)))
        .catch(() => undefined);
    }, 1500);
    return () => window.clearInterval(timer);
  }, [order, searching, socketFallback, token]);

  const signIn = async (event: FormEvent) => {
    event.preventDefault();
    setAuthBusy(true);
    setError(null);
    try {
      const freshToken = await authenticate(authEmail.trim(), authCode.trim());
      onAuthenticated(freshToken, authEmail.trim());
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Sign-in failed");
    } finally {
      setAuthBusy(false);
    }
  };

  const startSearch = async () => {
    if (!token) {
      onRequireAuth();
      setError("Sign in before creating an order.");
      return;
    }
    if (!corridor) {
      setError("The backend did not return an available corridor.");
      return;
    }
    const numericAmount = Number(amount.replace(/\s/g, "").replace(",", "."));
    if (!Number.isFinite(numericAmount) || numericAmount <= 0) {
      setError("Enter an amount greater than zero.");
      return;
    }
    setBusy(true);
    setError(null);
    setRoutes([]);
    setSelected(null);
    setFunding(null);
    setSettlement(null);
    setConsent(false);
    closeSocket.current?.();
    try {
      const freshOrder = await createOrder(
        token,
        {
          source_country: corridor.source_country,
          source_currency: corridor.source_currency,
          source_amount_minor: Math.round(numericAmount * 100),
          source_method_type: sourceMethod,
          source_method_ref: null,
          target_country: corridor.target_country,
          target_currency: corridor.target_currency,
          target_amount_min_minor: null,
          target_method_type: targetMethod,
          target_method_ref: recipient.trim() || null,
        },
        crypto.randomUUID(),
      );
      setOrder(freshOrder);
      setSearching(true);
      setSocketFallback(false);
      closeSocket.current = openLiveRoutes(token, freshOrder.id, handleLiveEvent, () => setSocketFallback(true));
      refreshHistory();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not create the order");
    } finally {
      setBusy(false);
    }
  };

  const lockRoute = async () => {
    if (!token || !order || !selected?.quote_id) return;
    setBusy(true);
    setError(null);
    try {
      const response: ConfirmOrderResponse = await confirmOrder(token, order.id, selected.quote_id);
      setOrder(response.order);
      setFunding(response.funding_instruction);
      setSettlement(response.settlement);
      setSearching(false);
      closeSocket.current?.();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not lock the route");
    } finally {
      setBusy(false);
    }
  };

  const fundAndFinish = async () => {
    if (!token || !order || !selected || !settlement || !consent) return;
    setBusy(true);
    setError(null);
    try {
      const funded = await confirmFunding(token, order.id, termsVersion);
      setOrder(funded.order);
      const completed = await submitMockProof(
        token,
        order.id,
        settlement.id,
        settlement.solver_id,
        selected.target_amount_minor ?? 0,
        selected.target_currency ?? order.target_currency,
      );
      setOrder(completed.order);
      refreshHistory();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not confirm settlement");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className={styles.shell} id="swap">
      <div className={styles.dock}>
        <div className={styles.card}>
          <div className={styles.head}>
            <div>
              <div className={styles.productTitle}>Send money across borders</div>
              <div className={styles.productHint}>Enter an amount and Pay3Flow finds the best route</div>
            </div>
            <span className={styles.statusPill}>{order ? STATUS_EN[order.status] ?? order.status : "New order"}</span>
          </div>

          <div className={styles.locationGrid} aria-label="Transfer direction">
            <label className={styles.locationField}>
              <span>From</span>
              <select
                className={styles.select}
                value={corridor ? locationKey(corridor.source_country, corridor.source_currency) : ""}
                onChange={(event) => chooseSourceLocation(event.target.value)}
                disabled={searching || Boolean(funding)}
              >
                {sourceLocations.map((location) => (
                  <option key={locationKey(location.country, location.currency)} value={locationKey(location.country, location.currency)}>
                    {locationLabel(location.country, location.currency)}
                  </option>
                ))}
              </select>
            </label>
            <span className={styles.directionArrow} aria-hidden="true">→</span>
            <label className={styles.locationField}>
              <span>To</span>
              <select
                className={styles.select}
                value={corridor ? locationKey(corridor.target_country, corridor.target_currency) : ""}
                onChange={(event) => chooseTargetLocation(event.target.value)}
                disabled={searching || Boolean(funding)}
              >
                {targetLocations.map((location) => (
                  <option key={locationKey(location.country, location.currency)} value={locationKey(location.country, location.currency)}>
                    {locationLabel(location.country, location.currency)}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className={styles.panel}>
            <div className={styles.panelMain}>
              <label className={styles.slotLabel} htmlFor="exchange-amount">You send</label>
              <input id="exchange-amount" className={styles.input} inputMode="decimal" value={amount} onChange={(event) => setAmount(event.target.value)} disabled={searching || Boolean(funding)} />
            </div>
            <span className={styles.currencyBadge}>{corridor?.source_currency ?? "—"}</span>
          </div>

          <div className={styles.twoColumns}>
            <label className={styles.fieldLabel}>Sending method<select className={styles.select} value={sourceMethod} onChange={(event) => setSourceMethod(event.target.value)}><option value="bank_card">Bank card</option><option value="bank_transfer">Bank transfer</option><option value="p2p">P2P</option></select></label>
            <label className={styles.fieldLabel}>Receiving method<select className={styles.select} value={targetMethod} onChange={(event) => setTargetMethod(event.target.value)}><option value="bank_card">Bank card</option><option value="bank_transfer">Bank account</option><option value="wallet">Wallet</option></select></label>
          </div>

          <label className={styles.fieldLabel}>Recipient<input className={styles.textInput} value={recipient} onChange={(event) => setRecipient(event.target.value)} placeholder="Name or saved recipient ID" /></label>

          {!funding && (
            <button type="button" className={styles.cta} disabled={busy || searching || !corridor} onClick={startSearch} data-testid="start-search">
              {(busy || searching) && <span className={styles.spinner} />}
              {searching ? "Searching routes…" : order ? "Search again" : "Find a route"}
            </button>
          )}

          {selected && !funding && (
            <div className={styles.selectionBox} data-testid="selected-route">
              <strong>Selected option</strong>
              <span>Recipient receives {money(selected.target_amount_minor ?? 0, selected.target_currency ?? corridor?.target_currency ?? "")}</span>
              <span>Fee {money(selected.fee_minor ?? 0, corridor?.source_currency ?? "")}, ETA {selected.eta_minutes} min</span>
              <button type="button" className={styles.cta} disabled={busy || selected.status !== "complete"} onClick={lockRoute}>Confirm selected route</button>
            </div>
          )}

          {funding && order?.status !== "done" && (
            <div className={styles.fundingBox} data-testid="funding-instruction">
              <strong>Payment instructions</strong>
              <span>{money(funding.amount_minor, funding.currency)} via {methodLabel(funding.method_type)}</span>
              <span className={styles.destination}>{funding.destination_ref}</span>
              <label className={styles.consentLabel}>
                <input type="checkbox" checked={consent} onChange={(event) => setConsent(event.target.checked)} />
                I confirm the payment instructions and terms version {termsVersion}. The route may use a TOKEN/crypto settlement asset; Pay3Flow does not debit fiat automatically.
              </label>
              <button type="button" className={styles.cta} disabled={!consent || busy} onClick={fundAndFinish} data-testid="confirm-funding">{busy ? "Processing…" : "Confirm payment"}</button>
            </div>
          )}

          {order?.status === "done" && <div className={styles.successBox} data-testid="order-done"><strong>Transfer completed</strong><span>Mock settlement and proof were verified successfully.</span></div>}
          {socketFallback && searching && <div className={styles.warning}>WebSocket unavailable — results are being refreshed by polling.</div>}
          {error && token && <div className={styles.errorBox} role="alert">{error}</div>}

          {history.length > 0 && (
            <details className={styles.history}>
              <summary>Transfer history ({history.length})</summary>
              {history.map((item) => <div key={item.id} className={styles.historyRow}><span>{money(item.source_amount_minor, item.source_currency)} → {item.target_currency}</span><strong>{STATUS_EN[item.status] ?? item.status}</strong></div>)}
            </details>
          )}
        </div>

        <SidePanel active={Boolean(order)} routes={routes} selectedQuoteId={selected?.quote_id ?? null} onSelect={setSelected} />
      </div>

      {!token && (
        <div className={styles.authBackdrop}>
          <div className={styles.authModal} role="dialog" aria-modal="true" aria-labelledby="auth-title">
            <form className={styles.authBox} onSubmit={signIn} data-testid="auth-form">
              <div className={styles.authMark} aria-hidden="true">P3</div>
              <div className={styles.authCopy}>
                <strong id="auth-title">Sign in to Pay3Flow</strong>
                <p>Enter your email and the demo one-time code 1234.</p>
              </div>
              <label className={styles.fieldLabel}>
                Email
                <input className={styles.textInput} type="email" value={authEmail} onChange={(event) => setAuthEmail(event.target.value)} required autoFocus aria-label="Email" />
              </label>
              <label className={styles.fieldLabel}>
                One-time code
                <input className={styles.textInput} value={authCode} onChange={(event) => setAuthCode(event.target.value)} required inputMode="numeric" aria-label="One-time code" />
              </label>
              {error && <div className={styles.errorBox} role="alert">{error}</div>}
              <button className={styles.secondaryButton} disabled={authBusy} type="submit">{authBusy ? "Signing in…" : "Sign in"}</button>
            </form>
          </div>
        </div>
      )}
    </section>
  );
}
