"use client";

import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  ExchangeCorridor,
  RouteCandidate,
  authenticate,
  fetchCorridors,
  fetchP2pRoutes,
} from "@/lib/exchange";
import { PaymentMethod, paymentMethodsFor } from "@/lib/payment-methods";

import { PaymentMethodPicker } from "./payment-method-picker";
import { SidePanel } from "./side-panel";
import styles from "./converter.module.css";

type RefreshSeconds = 0 | 5 | 15 | 30 | 60;

const REFRESH_OPTIONS: RefreshSeconds[] = [0, 5, 15, 30, 60];

const money = (minor: number, currency: string) =>
  `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency}`;

const amountFromMinor = (minor: number | undefined) =>
  minor == null
    ? "0"
    : (minor / 100).toLocaleString("en-US", {
        maximumFractionDigits: 2,
        useGrouping: false,
      });

const locationKey = (country: string, currency: string) => `${country}:${currency}`;

function locationLabel(country: string, currency: string): string {
  try {
    const region = new Intl.DisplayNames(["en"], { type: "region" }).of(country);
    return `${region ?? country} · ${currency}`;
  } catch {
    return `${country} · ${currency}`;
  }
}

function amountNumber(value: string): number {
  return Number(value.replace(/\s/g, "").replace(",", "."));
}

function mapRoutes(response: Awaited<ReturnType<typeof fetchP2pRoutes>>): RouteCandidate[] {
  const bestTarget = Number(response.routes[0]?.target_amount ?? 0);
  return response.routes.map((route, index) => {
    const targetAmount = Number(route.target_amount);
    const relativeBps =
      bestTarget > 0 && Number.isFinite(targetAmount)
        ? Math.round((targetAmount / bestTarget - 1) * 10_000)
        : 0;
    return {
      route_id: `live:${route.entry_offer.source}:${route.entry_offer.ad_id}:${route.exit_offer.source}:${route.exit_offer.ad_id}`,
      status: "complete",
      source_amount_minor: Math.round(Number(route.source_amount) * 100),
      source_currency: route.source_fiat,
      entry_asset: route.asset,
      entry_network: route.same_venue ? route.entry_offer.source : "cross-venue",
      target_amount_minor: Math.round(targetAmount * 100),
      target_currency: route.target_fiat,
      spread_bps: relativeBps,
      is_current_best: index === 0,
      is_live_market: true,
      payment_methods_verified: route.payment_methods_verified,
      legs: [
        {
          kind: "entry",
          from: route.source_fiat,
          to: route.asset,
          provider: route.entry_offer.source,
          status: "found",
        },
        {
          kind: "exit",
          from: route.asset,
          to: route.target_fiat,
          provider: route.exit_offer.source,
          status: "found",
        },
      ],
    };
  });
}

interface ConverterProps {
  token: string | null;
  email: string;
  onAuthenticated: (token: string, email: string) => void;
  onRequireAuth: () => void;
}

export function Converter({ token, email: sessionEmail, onAuthenticated }: ConverterProps) {
  const [corridors, setCorridors] = useState<ExchangeCorridor[]>([]);
  const [corridorId, setCorridorId] = useState("");
  const [amount, setAmount] = useState("0");
  const [authEmail, setAuthEmail] = useState(sessionEmail || "demo@pay3flow.dev");
  const [authCode, setAuthCode] = useState("1234");
  const [authBusy, setAuthBusy] = useState(false);
  const [routes, setRoutes] = useState<RouteCandidate[]>([]);
  const [selected, setSelected] = useState<RouteCandidate | null>(null);
  const [sourceMethodId, setSourceMethodId] = useState("am-ameriabank");
  const [targetMethodId, setTargetMethodId] = useState("ru-sberbank");
  const [methodPicker, setMethodPicker] = useState<"source" | "target" | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [refreshSeconds, setRefreshSeconds] = useState<RefreshSeconds>(15);
  const [searching, setSearching] = useState(false);
  const [lastUpdatedAt, setLastUpdatedAt] = useState<number | null>(null);
  const [clock, setClock] = useState(() => Date.now());
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);
  const abortRef = useRef<AbortController | null>(null);
  const settingsRef = useRef<HTMLDivElement>(null);

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

  const sourceMethods = useMemo(
    () =>
      corridor
        ? paymentMethodsFor(corridor.source_country, corridor.source_currency, "sender")
        : [],
    [corridor],
  );
  const targetMethods = useMemo(
    () =>
      corridor
        ? paymentMethodsFor(corridor.target_country, corridor.target_currency, "recipient")
        : [],
    [corridor],
  );
  const sourceMethod =
    sourceMethods.find((method) => method.id === sourceMethodId) ?? sourceMethods[0] ?? null;
  const targetMethod =
    targetMethods.find((method) => method.id === targetMethodId) ?? targetMethods[0] ?? null;
  const numericAmount = amountNumber(amount);
  const hasAmount = Number.isFinite(numericAmount) && numericAmount > 0;

  const previewRoute = useMemo(
    () =>
      selected ??
      routes.find((route) => route.status === "complete" && route.is_current_best) ??
      routes.find((route) => route.status === "complete") ??
      null,
    [routes, selected],
  );

  useEffect(() => {
    fetchCorridors()
      .then((response) => {
        setCorridors(response.items);
        setCorridorId((current) => current || response.items[0]?.id || "");
      })
      .catch((cause: Error) => setError(cause.message));
  }, []);

  useEffect(() => {
    if (token) return;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previousOverflow;
    };
  }, [token]);

  useEffect(() => {
    if (!settingsOpen) return;
    const onClickOutside = (event: MouseEvent) => {
      if (settingsRef.current && !settingsRef.current.contains(event.target as Node)) {
        setSettingsOpen(false);
      }
    };
    document.addEventListener("mousedown", onClickOutside);
    return () => document.removeEventListener("mousedown", onClickOutside);
  }, [settingsOpen]);

  useEffect(() => {
    if (!lastUpdatedAt) return;
    const timer = window.setInterval(() => setClock(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, [lastUpdatedAt]);

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

  const resetResults = () => {
    abortRef.current?.abort();
    setRoutes([]);
    setSelected(null);
    setLastUpdatedAt(null);
    setSearching(false);
    setError(null);
  };

  const chooseSourceMethod = (method: PaymentMethod) => {
    setSourceMethodId(method.id);
    setMethodPicker(null);
    resetResults();
  };

  const chooseTargetMethod = (method: PaymentMethod) => {
    setTargetMethodId(method.id);
    setMethodPicker(null);
    resetResults();
  };

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

  const startSearch = useCallback(async () => {
    if (!corridor || !sourceMethod || !targetMethod) return;
    const value = amountNumber(amount);
    if (!Number.isFinite(value) || value <= 0) {
      resetResults();
      return;
    }

    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;
    const requestId = ++requestRef.current;
    setSearching(true);
    setError(null);
    try {
      const response = await fetchP2pRoutes({
        sourceFiat: corridor.source_currency,
        targetFiat: corridor.target_currency,
        sourceAmount: value,
        sourcePaymentMethod: sourceMethod.p2pQuery,
        targetPaymentMethod: targetMethod.p2pQuery,
        allowCrossVenue: true,
        limit: 40,
        signal: controller.signal,
      });
      if (requestId !== requestRef.current) return;
      const liveRoutes = mapRoutes(response);
      setRoutes(liveRoutes);
      setSelected((current) =>
        current ? liveRoutes.find((route) => route.route_id === current.route_id) ?? null : null,
      );
      setLastUpdatedAt(Date.now());
      setClock(Date.now());
      if (liveRoutes.length === 0) {
        setError("No compatible live offers are available for this amount right now.");
      }
    } catch (cause) {
      if (controller.signal.aborted || requestId !== requestRef.current) return;
      setError(cause instanceof Error ? cause.message : "Could not search live P2P markets");
    } finally {
      if (requestId === requestRef.current) setSearching(false);
    }
  }, [amount, corridor, sourceMethod, targetMethod]);

  useEffect(() => {
    if (
      !hasAmount ||
      !corridor ||
      !sourceMethod ||
      !targetMethod ||
      searching ||
      routes.length > 0
    ) return;
    const timer = window.setTimeout(() => void startSearch(), 650);
    return () => window.clearTimeout(timer);
  }, [corridor, hasAmount, routes.length, searching, sourceMethod, startSearch, targetMethod]);

  useEffect(() => {
    if (!refreshSeconds || !lastUpdatedAt || !hasAmount) return;
    const timer = window.setInterval(() => void startSearch(), refreshSeconds * 1_000);
    return () => window.clearInterval(timer);
  }, [hasAmount, lastUpdatedAt, refreshSeconds, startSearch]);

  useEffect(() => () => abortRef.current?.abort(), []);

  const secondsUntilRefresh =
    refreshSeconds && lastUpdatedAt
      ? Math.max(0, refreshSeconds - Math.floor((clock - lastUpdatedAt) / 1_000))
      : null;

  const updateAmount = (value: string) => {
    const sanitized = value.replace(/[^0-9.,\s]/g, "");
    setAmount(sanitized || "0");
    resetResults();
  };

  return (
    <section className={styles.shell} id="transfer">
      <div className={styles.hero}>
        <div className={styles.eyebrow}>
          <span className={styles.eyebrowMark}>P3</span>
          Intelligent payment routing
        </div>
        <h1>Move money. <span>Keep more.</span></h1>
        <p>One intent, every available path. Pay3Flow compares live P2P liquidity and assembles the strongest route from Armenia to Russia.</p>
        <div className={styles.heroSignals} aria-label="Product capabilities">
          <span><i className={styles.signalLive} /> Live public offers</span>
          <span>4 connected venues</span>
          <span>Multi-asset routing</span>
        </div>
      </div>

      <div className={styles.workspace}>
        <div className={styles.card}>
          <div className={styles.cardTop}>
            <div className={styles.modeTabs} aria-label="Transfer mode">
              <button type="button" className={styles.modeActive}>Transfer</button>
              <button type="button" disabled>History</button>
            </div>
            <div className={styles.cardActions}>
              <button
                type="button"
                className={styles.refreshButton}
                onClick={() => void startSearch()}
                disabled={!hasAmount || searching}
                aria-label="Refresh routes now"
              >
                <svg className={searching ? styles.refreshSpin : undefined} width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                  <path d="M16.2 7.1A6.8 6.8 0 1 0 16.7 12" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
                  <path d="M13.1 3.8h3.6v3.6" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </button>
              <div className={styles.settingsWrap} ref={settingsRef}>
                <button
                  type="button"
                  className={styles.settingsButton}
                  onClick={() => setSettingsOpen((value) => !value)}
                  aria-expanded={settingsOpen}
                  aria-label="Route refresh settings"
                >
                  <svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                    <path d="M10 6.8a3.2 3.2 0 1 0 0 6.4 3.2 3.2 0 0 0 0-6.4Z" stroke="currentColor" strokeWidth="1.6" />
                    <path d="M16.2 11.3a6.5 6.5 0 0 0 0-2.6l1.5-1.1-1.8-3.1-1.8.8a6.7 6.7 0 0 0-2.2-1.3L11.7 2H8.3L8 4a6.7 6.7 0 0 0-2.2 1.3L4 4.5 2.2 7.6l1.5 1.1a6.5 6.5 0 0 0 0 2.6l-1.5 1.1L4 15.5l1.8-.8A6.7 6.7 0 0 0 8 16l.3 2h3.4l.3-2a6.7 6.7 0 0 0 2.2-1.3l1.8.8 1.8-3.1-1.6-1.1Z" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                </button>
                {settingsOpen && (
                  <div className={styles.settingsMenu} role="dialog" aria-label="Refresh settings">
                    <div className={styles.settingsHead}>
                      <div>
                        <strong>Auto-refresh</strong>
                        <span>Keep market routes current</span>
                      </div>
                      <span className={refreshSeconds ? styles.onBadge : styles.offBadge}>
                        {refreshSeconds ? "On" : "Off"}
                      </span>
                    </div>
                    <div className={styles.refreshOptions}>
                      {REFRESH_OPTIONS.map((seconds) => (
                        <button
                          key={seconds}
                          type="button"
                          aria-pressed={refreshSeconds === seconds}
                          onClick={() => {
                            setRefreshSeconds(seconds);
                            setSettingsOpen(false);
                          }}
                        >
                          {seconds === 0 ? "Off" : `${seconds}s`}
                        </button>
                      ))}
                    </div>
                    <p>Search also runs automatically 650ms after you change the amount or a bank.</p>
                  </div>
                )}
              </div>
            </div>
          </div>

          <div className={styles.intentLabel}>
            <span>Create transfer intent</span>
            <span className={styles.intentStatus}>{searching ? "Scanning markets" : "Live routing"}</span>
          </div>

          <div className={`${styles.moneyPanel} ${styles.moneyPanelSource}`}>
            <div className={styles.panelCopy}>
              <label htmlFor="exchange-amount">You send</label>
              <input
                id="exchange-amount"
                className={styles.amountInput}
                inputMode="decimal"
                value={amount}
                onFocus={(event) => event.currentTarget.select()}
                onChange={(event) => updateAmount(event.target.value)}
                aria-label="Amount to send"
              />
              <span className={styles.currencyHint}>{corridor?.source_currency ?? "AMD"} available via bank transfer</span>
            </div>
            <button
              type="button"
              className={styles.methodTrigger}
              onClick={() => setMethodPicker("source")}
              aria-label={`Select sending bank: ${sourceMethod?.name ?? "none"}`}
            >
              <span className={styles.methodAvatar} style={{ backgroundColor: sourceMethod?.color ?? "#171a17" }} aria-hidden="true">
                {sourceMethod?.initials ?? corridor?.source_country ?? "—"}
              </span>
              <span className={styles.methodText}>
                <strong>{sourceMethod?.name ?? "Select bank"}</strong>
                <small>{corridor ? locationLabel(corridor.source_country, corridor.source_currency) : "Unavailable"}</small>
              </span>
              <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path d="m4 6 4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
          </div>

          <div className={styles.flowBridge} aria-hidden="true">
            <span className={styles.bridgeLine} />
            <span className={styles.bridgeIcon}>
              <svg width="18" height="18" viewBox="0 0 20 20" fill="none">
                <path d="M10 4v12m0 0-4-4m4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </span>
            <span className={styles.bridgeLabel}>{searching ? "Building route" : "Best path"}</span>
          </div>

          <div className={`${styles.moneyPanel} ${styles.moneyPanelTarget}`}>
            <div className={styles.panelCopy}>
              <label>Recipient gets</label>
              <output className={previewRoute ? styles.amountOutput : styles.amountOutputEmpty}>
                {amountFromMinor(previewRoute?.target_amount_minor)}
              </output>
              <span className={styles.currencyHint}>
                {previewRoute ? `Estimated ${previewRoute.target_currency}` : "Live estimate appears here"}
              </span>
            </div>
            <button
              type="button"
              className={styles.methodTrigger}
              onClick={() => setMethodPicker("target")}
              aria-label={`Select recipient bank: ${targetMethod?.name ?? "none"}`}
            >
              <span className={styles.methodAvatar} style={{ backgroundColor: targetMethod?.color ?? "#171a17" }} aria-hidden="true">
                {targetMethod?.initials ?? corridor?.target_country ?? "—"}
              </span>
              <span className={styles.methodText}>
                <strong>{targetMethod?.name ?? "Select bank"}</strong>
                <small>{corridor ? locationLabel(corridor.target_country, corridor.target_currency) : "Unavailable"}</small>
              </span>
              <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path d="m4 6 4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
          </div>

          <div className={styles.marketBar}>
            <div className={styles.marketState}>
              <span className={searching ? styles.scanningDot : routes.length ? styles.liveDot : styles.idleDot} />
              <div>
                <strong>
                  {searching
                    ? "Comparing live offers"
                    : routes.length
                      ? `${routes.length} executable estimates`
                      : hasAmount
                        ? "Ready to search"
                        : "Enter an amount"}
                </strong>
                <span>
                  {lastUpdatedAt
                    ? `Updated ${Math.max(0, Math.floor((clock - lastUpdatedAt) / 1_000))}s ago`
                    : "Binance · Bybit · OKX · Bitget"}
                </span>
              </div>
            </div>
            {secondsUntilRefresh !== null && (
              <span className={styles.nextRefresh}>Refresh in {secondsUntilRefresh}s</span>
            )}
          </div>

          <button
            type="button"
            className={styles.cta}
            disabled={!hasAmount || searching || !corridor}
            onClick={() => void startSearch()}
            data-testid="start-search"
          >
            {searching ? (
              <><span className={styles.spinner} /> Searching every path</>
            ) : routes.length ? (
              <>Refresh {routes.length} live routes <span>↗</span></>
            ) : hasAmount ? (
              <>Find the best route <span>→</span></>
            ) : (
              "Enter an amount to begin"
            )}
          </button>

          {selected && (
            <div className={styles.selectionBox} data-testid="selected-route">
              <div className={styles.selectionIcon}>✓</div>
              <div>
                <strong>Route selected</strong>
                <span>{money(selected.target_amount_minor ?? 0, selected.target_currency ?? corridor?.target_currency ?? "")}</span>
                <small>
                  {selected.payment_methods_verified
                    ? "Both banks are listed on the matched offers."
                    : "Confirm both banks on the venue before transferring."}
                </small>
              </div>
            </div>
          )}
          {error && token && <div className={styles.errorBox} role="alert">{error}</div>}
        </div>

        <SidePanel
          active
          routes={routes}
          selectedRouteId={selected?.route_id ?? null}
          onSelect={setSelected}
          searching={searching}
          hasAmount={hasAmount}
          sourceBank={sourceMethod?.name ?? "Sender bank"}
          targetBank={targetMethod?.name ?? "Recipient bank"}
        />
      </div>

      <div className={styles.assurance} id="how-it-works">
        <div><strong>01</strong><span>You choose the banks</span></div>
        <div><strong>02</strong><span>We scan every viable asset</span></div>
        <div><strong>03</strong><span>You receive the strongest route</span></div>
      </div>

      <PaymentMethodPicker
        open={methodPicker === "source"}
        title="Choose where you pay from"
        role="sender"
        locations={sourceLocations}
        selectedLocation={corridor ? { country: corridor.source_country, currency: corridor.source_currency } : null}
        selected={sourceMethod}
        onClose={() => setMethodPicker(null)}
        onLocationSelect={(location) => chooseSourceLocation(locationKey(location.country, location.currency))}
        onSelect={chooseSourceMethod}
      />

      <PaymentMethodPicker
        open={methodPicker === "target"}
        title="Choose where the recipient gets paid"
        role="recipient"
        locations={targetLocations}
        selectedLocation={corridor ? { country: corridor.target_country, currency: corridor.target_currency } : null}
        selected={targetMethod}
        onClose={() => setMethodPicker(null)}
        onLocationSelect={(location) => chooseTargetLocation(locationKey(location.country, location.currency))}
        onSelect={chooseTargetMethod}
      />

      {!token && (
        <div className={styles.authBackdrop}>
          <div className={styles.authModal} role="dialog" aria-modal="true" aria-labelledby="auth-title">
            <form className={styles.authBox} onSubmit={signIn} data-testid="auth-form">
              <div className={styles.authBrand}>
                <span className={styles.authMark}>P3</span>
                <span>PAY3FLOW ACCESS</span>
              </div>
              <div className={styles.authCopy}>
                <span className={styles.authEyebrow}>Welcome back</span>
                <strong id="auth-title">Sign in to route money smarter.</strong>
                <p>Use your email and the demo one-time code to enter the live routing workspace.</p>
              </div>
              <label className={styles.fieldLabel}>
                Email address
                <input className={styles.textInput} type="email" value={authEmail} onChange={(event) => setAuthEmail(event.target.value)} required autoFocus aria-label="Email" />
              </label>
              <label className={styles.fieldLabel}>
                One-time code
                <input className={styles.textInput} value={authCode} onChange={(event) => setAuthCode(event.target.value)} required inputMode="numeric" aria-label="One-time code" />
              </label>
              <div className={styles.demoNote}><span>Demo</span> Use code <strong>1234</strong></div>
              {error && <div className={styles.errorBox} role="alert">{error}</div>}
              <button className={styles.secondaryButton} disabled={authBusy} type="submit">
                {authBusy ? "Opening workspace…" : "Enter Pay3Flow"}
              </button>
              <small className={styles.authLegal}>By continuing, you agree to the routing and settlement disclosure.</small>
            </form>
          </div>
        </div>
      )}
    </section>
  );
}
