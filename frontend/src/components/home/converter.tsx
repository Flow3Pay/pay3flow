"use client";

import { FormEvent, useEffect, useMemo, useState } from "react";

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

const money = (minor: number, currency: string) =>
  `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency}`;

const amountFromMinor = (minor: number | undefined) =>
  minor == null
    ? ""
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

interface ConverterProps {
  token: string | null;
  email: string;
  onAuthenticated: (token: string, email: string) => void;
  onRequireAuth: () => void;
}

export function Converter({ token, email: sessionEmail, onAuthenticated, onRequireAuth }: ConverterProps) {
  const [corridors, setCorridors] = useState<ExchangeCorridor[]>([]);
  const [corridorId, setCorridorId] = useState("");
  const [amount, setAmount] = useState("100000");
  const [authEmail, setAuthEmail] = useState(sessionEmail || "demo@pay3flow.dev");
  const [authCode, setAuthCode] = useState("1234");
  const [authBusy, setAuthBusy] = useState(false);
  const [routes, setRoutes] = useState<RouteCandidate[]>([]);
  const [selected, setSelected] = useState<RouteCandidate | null>(null);
  const [sourceMethodId, setSourceMethodId] = useState("am-ameriabank");
  const [targetMethodId, setTargetMethodId] = useState("ru-sberbank");
  const [methodPicker, setMethodPicker] = useState<"source" | "target" | null>(null);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  const chooseSourceMethod = (method: PaymentMethod) => {
    setSourceMethodId(method.id);
    setMethodPicker(null);
    setRoutes([]);
    setSelected(null);
  };

  const chooseTargetMethod = (method: PaymentMethod) => {
    setTargetMethodId(method.id);
    setMethodPicker(null);
    setRoutes([]);
    setSelected(null);
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

  const startSearch = async () => {
    if (!token) {
      onRequireAuth();
      setError("Sign in before searching live routes.");
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
    setSearching(true);
    setError(null);
    setRoutes([]);
    setSelected(null);
    try {
      const response = await fetchP2pRoutes({
        sourceFiat: corridor.source_currency,
        targetFiat: corridor.target_currency,
        sourceAmount: numericAmount,
        sourcePaymentMethod: sourceMethod?.p2pQuery,
        targetPaymentMethod: targetMethod?.p2pQuery,
        allowCrossVenue: true,
        limit: 40,
      });
      const bestTarget = Number(response.routes[0]?.target_amount ?? 0);
      const liveRoutes: RouteCandidate[] = response.routes.map((route, index) => {
        const targetAmount = Number(route.target_amount);
        const relativeBps =
          bestTarget > 0 && Number.isFinite(targetAmount)
            ? Math.round((targetAmount / bestTarget - 1) * 10_000)
            : 0;
        return {
          route_id: `live:${route.entry_offer.source}:${route.entry_offer.ad_id}:${route.exit_offer.ad_id}`,
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
      setRoutes(liveRoutes);
      if (liveRoutes.length === 0) {
        setError("No compatible live P2P offers are available for this amount right now.");
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Could not search live P2P markets");
    } finally {
      setSearching(false);
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
            <span className={styles.statusPill}>
              {searching ? "Searching live markets" : routes.length > 0 ? `${routes.length} live routes` : "Ready"}
            </span>
          </div>

          <div className={styles.slotTop}>
            <span className={styles.slotLabel}>You send</span>
            <span className={styles.slotHint}>From</span>
          </div>
          <div className={styles.panel}>
            <div className={styles.panelMain}>
              <input
                id="exchange-amount"
                className={styles.input}
                inputMode="decimal"
                placeholder="0"
                value={amount}
                onChange={(event) => setAmount(event.target.value)}
                disabled={searching}
                aria-label="Amount to send"
              />
            </div>
            <button
                type="button"
                className={styles.methodTrigger}
                onClick={() => setMethodPicker("source")}
                disabled={searching}
                aria-label={`Select sending bank: ${sourceMethod?.name ?? "none"}`}
              >
                <span
                  className={styles.methodAvatar}
                  style={{ backgroundColor: sourceMethod?.color ?? "var(--color-text)" }}
                  aria-hidden="true"
                >
                  {sourceMethod?.initials ?? corridor?.source_country ?? "—"}
                </span>
                <span className={styles.methodTriggerCopy}>
                  <strong>{sourceMethod?.name ?? "Select bank"}</strong>
                  <span>{corridor ? locationLabel(corridor.source_country, corridor.source_currency) : "Unavailable"}</span>
                </span>
                <svg className={styles.methodChevron} width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path d="m4 6 4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
            </button>
          </div>

          <div className={styles.separator} aria-hidden="true">
            <span className={styles.routeDirection}>
              <svg width="18" height="18" viewBox="0 0 20 20" fill="none">
                <path d="M10 4v12m0 0-4-4m4 4 4-4" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </span>
          </div>

          <div className={styles.slotTop}>
            <span className={styles.slotLabel}>Recipient gets</span>
            <span className={styles.slotHint}>To</span>
          </div>
          <div className={styles.panel}>
            <div className={styles.panelMain}>
              <input
                className={styles.input}
                placeholder="0"
                value={amountFromMinor(previewRoute?.target_amount_minor)}
                readOnly
                aria-label={`Estimated amount in ${corridor?.target_currency ?? "target currency"}`}
              />
            </div>
            <button
                type="button"
                className={styles.methodTrigger}
                onClick={() => setMethodPicker("target")}
                disabled={searching}
                aria-label={`Select recipient bank: ${targetMethod?.name ?? "none"}`}
              >
                <span
                  className={styles.methodAvatar}
                  style={{ backgroundColor: targetMethod?.color ?? "var(--color-text)" }}
                  aria-hidden="true"
                >
                  {targetMethod?.initials ?? corridor?.target_country ?? "—"}
                </span>
                <span className={styles.methodTriggerCopy}>
                  <strong>{targetMethod?.name ?? "Select bank"}</strong>
                  <span>{corridor ? locationLabel(corridor.target_country, corridor.target_currency) : "Unavailable"}</span>
                </span>
                <svg className={styles.methodChevron} width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path d="m4 6 4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
            </button>
          </div>

          <button type="button" className={styles.cta} disabled={searching || !corridor} onClick={startSearch} data-testid="start-search">
            {searching && <span className={styles.spinner} />}
            {searching ? "Searching live routes…" : routes.length > 0 ? "Refresh live routes" : "Find live routes"}
          </button>

          {selected && (
            <div className={styles.selectionBox} data-testid="selected-route">
              <strong>Selected live route</strong>
              <span>Estimated recipient amount: {money(selected.target_amount_minor ?? 0, selected.target_currency ?? corridor?.target_currency ?? "")}</span>
              <span>
                {selected.payment_methods_verified
                  ? "Both selected banks are listed on the matched offers."
                  : "Confirm both selected banks on the venue before starting the transfer."}
              </span>
              <span>This is a read-only public market estimate. No trade or reservation has been placed.</span>
            </div>
          )}
          {error && token && <div className={styles.errorBox} role="alert">{error}</div>}
        </div>

        <SidePanel active={routes.length > 0} routes={routes} selectedRouteId={selected?.route_id ?? null} onSelect={setSelected} />
      </div>

      <PaymentMethodPicker
        open={methodPicker === "source"}
        title="Select sender bank"
        role="sender"
        locations={sourceLocations}
        selectedLocation={
          corridor
            ? { country: corridor.source_country, currency: corridor.source_currency }
            : null
        }
        selected={sourceMethod}
        onClose={() => setMethodPicker(null)}
        onLocationSelect={(location) =>
          chooseSourceLocation(locationKey(location.country, location.currency))
        }
        onSelect={chooseSourceMethod}
      />

      <PaymentMethodPicker
        open={methodPicker === "target"}
        title="Select recipient bank"
        role="recipient"
        locations={targetLocations}
        selectedLocation={
          corridor
            ? { country: corridor.target_country, currency: corridor.target_currency }
            : null
        }
        selected={targetMethod}
        onClose={() => setMethodPicker(null)}
        onLocationSelect={(location) =>
          chooseTargetLocation(locationKey(location.country, location.currency))
        }
        onSelect={chooseTargetMethod}
      />

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
