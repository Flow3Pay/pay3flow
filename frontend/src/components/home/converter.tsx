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
const AMOUNT_STORAGE_KEY = "pay3flow.exchange.amount";
const REFRESH_STORAGE_KEY = "pay3flow.exchange.refresh-seconds";

interface SharedExchange {
  sourceCurrency: string;
  targetCurrency: string;
  amount: string | null;
}

function readSharedExchange(): SharedExchange | null {
  if (typeof window === "undefined") return null;

  const match = window.location.hash.match(/^#\/swap\/([^/?#]+)\/([^/?#]+)(?:\?([^#]*))?$/i);
  if (!match) return null;

  const params = new URLSearchParams(match[3] ?? "");
  const amount = params.get("amount");
  return {
    sourceCurrency: decodeURIComponent(match[1]).toUpperCase(),
    targetCurrency: decodeURIComponent(match[2]).toUpperCase(),
    amount: amount && /^[0-9.,\s]+$/.test(amount) ? amount : null,
  };
}

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
  const [directionReversed, setDirectionReversed] = useState(false);
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
  const preferencesLoadedRef = useRef(false);
  const urlReadyRef = useRef(false);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      try {
        const sharedExchange = readSharedExchange();
        const savedAmount = window.localStorage.getItem(AMOUNT_STORAGE_KEY);
        if (sharedExchange?.amount) setAmount(sharedExchange.amount);
        else if (savedAmount) setAmount(savedAmount);

        const savedRefresh = Number(window.localStorage.getItem(REFRESH_STORAGE_KEY));
        if (REFRESH_OPTIONS.includes(savedRefresh as RefreshSeconds)) {
          setRefreshSeconds(savedRefresh as RefreshSeconds);
        }
      } catch {
        // Local storage can be unavailable when the browser blocks site data.
      }
      preferencesLoadedRef.current = true;
    }, 0);
    return () => window.clearTimeout(timer);
  }, []);

  useEffect(() => {
    if (!preferencesLoadedRef.current) return;
    try {
      window.localStorage.setItem(AMOUNT_STORAGE_KEY, amount);
    } catch {
      // Local storage can be unavailable when the browser blocks site data.
    }
  }, [amount]);

  useEffect(() => {
    if (!preferencesLoadedRef.current) return;
    try {
      window.localStorage.setItem(REFRESH_STORAGE_KEY, String(refreshSeconds));
    } catch {
      // Local storage can be unavailable when the browser blocks site data.
    }
  }, [refreshSeconds]);

  const corridor = useMemo(
    () => corridors.find((item) => item.id === corridorId) ?? corridors[0],
    [corridorId, corridors],
  );

  const locations = useMemo(
    () => [
      ...new Map(
        corridors.flatMap((item) => [
          [
            locationKey(item.source_country, item.source_currency),
            { country: item.source_country, currency: item.source_currency },
          ] as const,
          [
            locationKey(item.target_country, item.target_currency),
            { country: item.target_country, currency: item.target_currency },
          ] as const,
        ]),
      ).values(),
    ],
    [corridors],
  );

  const sourceCountry = corridor
    ? directionReversed
      ? corridor.target_country
      : corridor.source_country
    : "";
  const sourceCurrency = corridor
    ? directionReversed
      ? corridor.target_currency
      : corridor.source_currency
    : "";
  const targetCountry = corridor
    ? directionReversed
      ? corridor.source_country
      : corridor.target_country
    : "";
  const targetCurrency = corridor
    ? directionReversed
      ? corridor.source_currency
      : corridor.target_currency
    : "";

  const sourceMethods = useMemo(
    () => (sourceCountry ? paymentMethodsFor(sourceCountry, sourceCurrency, "sender") : []),
    [sourceCountry, sourceCurrency],
  );
  const targetMethods = useMemo(
    () => (targetCountry ? paymentMethodsFor(targetCountry, targetCurrency, "recipient") : []),
    [targetCountry, targetCurrency],
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
        const sharedExchange = readSharedExchange();
        const sharedCorridor = sharedExchange
          ? response.items.find(
              (item) =>
                (item.source_currency === sharedExchange.sourceCurrency &&
                  item.target_currency === sharedExchange.targetCurrency) ||
                (item.source_currency === sharedExchange.targetCurrency &&
                  item.target_currency === sharedExchange.sourceCurrency),
            )
          : null;

        setCorridors(response.items);
        setCorridorId((current) => current || sharedCorridor?.id || response.items[0]?.id || "");
        if (
          sharedExchange &&
          sharedCorridor?.source_currency === sharedExchange.targetCurrency &&
          sharedCorridor.target_currency === sharedExchange.sourceCurrency
        ) {
          setDirectionReversed(true);
        }
        urlReadyRef.current = true;
      })
      .catch((cause: Error) => setError(cause.message));
  }, []);

  useEffect(() => {
    if (!urlReadyRef.current || !corridor || !sourceCurrency || !targetCurrency) return;

    const params = new URLSearchParams();
    if (amount !== "0") params.set("amount", amount);
    const query = params.toString();
    const hash = `#/swap/${encodeURIComponent(sourceCurrency)}/${encodeURIComponent(targetCurrency)}${query ? `?${query}` : ""}`;
    window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}${hash}`);
  }, [amount, corridor, sourceCurrency, targetCurrency]);

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

  const resetResults = () => {
    abortRef.current?.abort();
    setRoutes([]);
    setSelected(null);
    setLastUpdatedAt(null);
    setSearching(false);
    setError(null);
  };

  const applyOrientation = (next: ExchangeCorridor, reversed: boolean) => {
    const nextSourceCountry = reversed ? next.target_country : next.source_country;
    const nextSourceCurrency = reversed ? next.target_currency : next.source_currency;
    const nextTargetCountry = reversed ? next.source_country : next.target_country;
    const nextTargetCurrency = reversed ? next.source_currency : next.target_currency;
    setCorridorId(next.id);
    setDirectionReversed(reversed);
    setSourceMethodId(
      paymentMethodsFor(nextSourceCountry, nextSourceCurrency, "sender")[0]?.id ?? "",
    );
    setTargetMethodId(
      paymentMethodsFor(nextTargetCountry, nextTargetCurrency, "recipient")[0]?.id ?? "",
    );
    setMethodPicker(null);
    resetResults();
  };

  const chooseSourceLocation = (value: string) => {
    const direct = corridors.find(
      (item) => locationKey(item.source_country, item.source_currency) === value,
    );
    if (direct) {
      applyOrientation(direct, false);
      return;
    }
    const reversed = corridors.find(
      (item) => locationKey(item.target_country, item.target_currency) === value,
    );
    if (reversed) applyOrientation(reversed, true);
  };

  const chooseTargetLocation = (value: string) => {
    const direct = corridors.find(
      (item) => locationKey(item.target_country, item.target_currency) === value,
    );
    if (direct) {
      applyOrientation(direct, false);
      return;
    }
    const reversed = corridors.find(
      (item) => locationKey(item.source_country, item.source_currency) === value,
    );
    if (reversed) applyOrientation(reversed, true);
  };

  const swapDirection = () => {
    if (!corridor) return;
    const nextSourceMethodId = targetMethod?.id ?? "";
    const nextTargetMethodId = sourceMethod?.id ?? "";
    setDirectionReversed((current) => !current);
    setSourceMethodId(nextSourceMethodId);
    setTargetMethodId(nextTargetMethodId);
    resetResults();
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
        sourceFiat: sourceCurrency,
        targetFiat: targetCurrency,
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
        liveRoutes.find((route) => route.route_id === current?.route_id) ??
          liveRoutes.find((route) => route.status === "complete" && route.is_current_best) ??
          liveRoutes.find((route) => route.status === "complete") ??
          null,
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
  }, [amount, corridor, sourceCurrency, sourceMethod, targetCurrency, targetMethod]);

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
  const refreshProgress =
    secondsUntilRefresh !== null && refreshSeconds
      ? ((refreshSeconds - secondsUntilRefresh) / refreshSeconds) * 100
      : 0;

  const updateAmount = (value: string) => {
    const sanitized = value.replace(/[^0-9.,\s]/g, "").replace(/\s/g, "");
    const withoutLeadingZeroes = sanitized.replace(/^0+(?=\d)/, "");
    setAmount(withoutLeadingZeroes || "0");
    resetResults();
  };

  return (
    <section className={styles.shell} id="transfer">
      <div className={styles.hero}>
        <h1>Move money. <span>Keep more.</span></h1>
        <p>One intent, every available path. Pay3Flow compares live P2P liquidity and assembles the strongest cross-border route for you.</p>
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
              <span className={styles.currencyHint}>{sourceCurrency || "AMD"} available via bank transfer</span>
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
                <small>{corridor ? locationLabel(sourceCountry, sourceCurrency) : "Unavailable"}</small>
              </span>
              <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path d="m4 6 4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
          </div>

          <div className={styles.flowBridge}>
            <span className={styles.bridgeLine} aria-hidden="true" />
            <button
              type="button"
              className={`${styles.bridgeIcon}${directionReversed ? ` ${styles.bridgeIconReversed}` : ""}`}
              onClick={swapDirection}
              aria-label="Swap sender and recipient"
              title="Swap sender and recipient"
            >
              <svg width="18" height="18" viewBox="0 0 20 20" fill="none">
                <path d="M10 4v12m0 0-4-4m4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
            <span className={styles.bridgeLabel}>{searching ? "Building route" : "Swap direction"}</span>
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
                <small>{corridor ? locationLabel(targetCountry, targetCurrency) : "Unavailable"}</small>
              </span>
              <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path d="m4 6 4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
          </div>

          <div className={styles.marketBar}>
            <div className={styles.marketState}>
              <span
                className={styles.refreshProgress}
                role="img"
                aria-label={secondsUntilRefresh === null ? "Auto-refresh is off" : `Refresh in ${secondsUntilRefresh} seconds`}
              >
                <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
                  <circle className={styles.refreshTrack} cx="9" cy="9" r="7" pathLength="100" />
                  <circle
                    className={styles.refreshFill}
                    cx="9"
                    cy="9"
                    r="7"
                    pathLength="100"
                    style={{ strokeDashoffset: `${100 - refreshProgress}` }}
                  />
                </svg>
              </span>
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
              <span className={styles.nextRefresh}>{secondsUntilRefresh}s</span>
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
        locations={locations}
        selectedLocation={corridor ? { country: sourceCountry, currency: sourceCurrency } : null}
        selected={sourceMethod}
        onClose={() => setMethodPicker(null)}
        onLocationSelect={(location) => chooseSourceLocation(locationKey(location.country, location.currency))}
        onSelect={chooseSourceMethod}
      />

      <PaymentMethodPicker
        open={methodPicker === "target"}
        title="Choose where the recipient gets paid"
        role="recipient"
        locations={locations}
        selectedLocation={corridor ? { country: targetCountry, currency: targetCurrency } : null}
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
