"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  ExchangeCorridor,
  RouteCandidate,
  fetchCorridors,
  fetchP2pRoutes,
} from "@/lib/exchange";
import { CryptoNetwork, FALLBACK_NETWORK, fetchNetworks } from "@/lib/networks";
import {
  CRYPTO_ASSETS,
  DIGITAL_ASSETS,
  PaymentMethod,
  paymentMethodFavicon,
  paymentMethodsFor,
} from "@/lib/payment-methods";

import { PaymentMethodPicker } from "./payment-method-picker";
import { BankLogo } from "./bank-logo";
import { NetworkPicker } from "./network-picker";
import { RouteInstructions } from "./route-instructions";
import { SidePanel } from "./side-panel";
import styles from "./converter.module.css";

type RefreshSeconds = 0 | 5 | 15 | 30 | 60;

const REFRESH_OPTIONS: RefreshSeconds[] = [0, 5, 15, 30, 60];
const P2P_SOURCES = [
  { id: "binance", label: "Binance", iconUrl: "https://binance.com/favicon.ico" },
  { id: "bybit", label: "Bybit", iconUrl: "https://www.bybit.com/favicon.ico" },
  { id: "okx", label: "OKX", iconUrl: "https://www.okx.com/favicon.ico" },
  { id: "bitget", label: "Bitget", iconUrl: "https://www.bitget.com/favicon.ico" },
  { id: "rapira", label: "Rapira", iconUrl: "https://rapira.net/favicon.ico" },
] as const;
type P2pSource = (typeof P2P_SOURCES)[number]["id"];
const DEFAULT_P2P_SOURCES = P2P_SOURCES.map((source) => source.id);
const INTERMEDIARY_ASSETS = CRYPTO_ASSETS.map(([currency]) => currency);
const CRYPTO_ICON_CDN = "https://cdn.jsdelivr.net/gh/spothq/cryptocurrency-icons@0.18.1/128/color";
const INTERMEDIARY_ASSET_ICONS: Record<string, string> = {
  USDT: "https://upload.wikimedia.org/wikipedia/commons/0/01/USDT_Logo.png?utm_source=commons.wikimedia.org&utm_campaign=index&utm_content=original",
  USDC: "https://thumb.wikimedia.org/wikipedia/commons/thumb/4/4a/Circle_USDC_Logo.svg/1280px-Circle_USDC_Logo.svg.png?utm_source=en.wikipedia.org&utm_campaign=index&utm_content=thumbnail",
  BTC: "https://thumb.wikimedia.org/wikipedia/commons/thumb/4/46/Bitcoin.svg/1280px-Bitcoin.svg.png?utm_source=en.wikipedia.org&utm_campaign=index&utm_content=thumbnail",
  ETH: "https://upload.wikimedia.org/wikipedia/commons/f/fd/Ethereum_Logo.png?utm_source=commons.wikimedia.org&utm_campaign=index&utm_content=original",
  BNB: "https://assets.streamlinehq.com/image/private/w_300,h_300,ar_1/f_auto/v1/icons/vectors/bnb-2c9adc7qw85po528q8y3b.png/bnb-tss7lyzvhxyjfc9ivae0l.png?_a=DATAiZAAZAA0",
  SOL: "https://upload.wikimedia.org/wikipedia/en/b/b9/Solana_logo.png?utm_source=en.wikipedia.org&utm_campaign=index&utm_content=original",
  TRX: "https://assets.streamlinehq.com/image/private/w_300,h_300,ar_1/f_auto/v1/icons/vectors/trx-q8ocxy7h0nc2c11ucr2m.png/trx-2ynri4p1kxp5a8djpqdzy8.png?_a=DATAiZAAZAA0",
  DOT: "https://i.pinimg.com/originals/e1/bb/20/e1bb208a7252b1e3c3cd58a85d6e06c7.png",
  TON: "https://cdn-icons-png.flaticon.com/512/12114/12114247.png",
  APT: "https://readi.fi/media/Aptos_mark_BLK.png",
  NEAR: "https://cdn-icons-png.flaticon.com/512/14446/14446201.png",
  SUI: "https://s2.coinmarketcap.com/static/img/coins/200x200/20947.png",
  FDUSD: "https://assets.kraken.com/marketing/web/icons-uni-webp/s_fdusd.webp?i=kds",
};

function intermediaryIconUrl(asset: string): string {
  return INTERMEDIARY_ASSET_ICONS[asset] ?? `${CRYPTO_ICON_CDN}/${asset.toLowerCase()}.png`;
}
const AMOUNT_STORAGE_KEY = "pay3flow.exchange.amount";
const REFRESH_STORAGE_KEY = "pay3flow.exchange.refresh-seconds";
const SOURCES_STORAGE_KEY = "pay3flow.exchange.p2p-sources";
const CORRIDOR_STORAGE_KEY = "pay3flow.exchange.corridor";
const SOURCE_METHOD_STORAGE_KEY = "pay3flow.exchange.source-method";
const TARGET_METHOD_STORAGE_KEY = "pay3flow.exchange.target-method";
const DIRECTION_STORAGE_KEY = "pay3flow.exchange.direction-reversed";
const INTERMEDIARY_ASSETS_STORAGE_KEY = "pay3flow.exchange.intermediary-assets";

interface NetworkControlProps {
  network: CryptoNetwork;
  onOpen: () => void;
}

function NetworkControl({ network, onOpen }: NetworkControlProps) {
  return (
    <div className={styles.networkControl}>
      <button
        type="button"
        className={styles.networkButton}
        onClick={onOpen}
        aria-haspopup="dialog"
      >
        <span className={styles.networkDot} aria-hidden="true">♦</span>
        <span className={styles.networkCopy}>
          <small>Network</small>
          <strong>{network.name}</strong>
        </span>
        <span className={styles.networkChevron} aria-hidden="true">⌄</span>
      </button>
    </div>
  );
}

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

function locationLabel(country: string, currency: string): string {
  try {
    const region = new Intl.DisplayNames(["en"], { type: "region" }).of(country);
    return `${region ?? country} · ${currency}`;
  } catch {
    return `${country} · ${currency}`;
  }
}

function normalizeAmountInput(value: string): string {
  const sanitized = value
    .normalize("NFKC")
    .replace(/[\u00a0\u200b-\u200d\ufeff]/g, "")
    .replace(/[٫٬]/g, ".")
    .replace(/[^0-9.,]/g, "");
  const lastSeparator = Math.max(sanitized.lastIndexOf("."), sanitized.lastIndexOf(","));
  if (lastSeparator < 0) {
    return sanitized.replace(/^0+(?=\d)/, "") || "0";
  }

  const integerPart = sanitized.slice(0, lastSeparator).replace(/[.,]/g, "");
  const fractionPart = sanitized.slice(lastSeparator + 1).replace(/[.,]/g, "");
  const integer = integerPart.replace(/^0+(?=\d)/, "") || "0";
  return `${integer}${sanitized[lastSeparator]}${fractionPart}`;
}

function amountNumber(value: string): number {
  return Number(normalizeAmountInput(value).replace(",", "."));
}

function mapRoutes(
  response: Awaited<ReturnType<typeof fetchP2pRoutes>>,
  sourceMethod: PaymentMethod | null,
  targetMethod: PaymentMethod | null,
): RouteCandidate[] {
  const bestTarget = Number(response.routes[0]?.target_amount ?? 0);
  return response.routes.map((route, index) => {
    const entryOffer = route.entry_offer;
    const exitOffer = route.exit_offer;
    const targetAmount = Number(route.target_amount);
    const relativeBps =
      bestTarget > 0 && Number.isFinite(targetAmount)
        ? Math.round((targetAmount / bestTarget - 1) * 10_000)
        : 0;
    return {
      route_id: route.market_path
        ? `spot:${route.market_path.venue}:${route.market_path.source_pair}:${route.market_path.target_pair}`
        : `live:${entryOffer?.source ?? "direct"}:${entryOffer?.ad_id ?? "none"}:${exitOffer?.source ?? "direct"}:${exitOffer?.ad_id ?? "none"}`,
      status: "complete",
      source_amount_minor: Math.round(Number(route.source_amount) * 100),
      source_currency: route.source_fiat,
      source_method_icon_url: sourceMethod?.kind === "bank" ? paymentMethodFavicon(sourceMethod) ?? undefined : undefined,
      entry_asset: route.asset,
      entry_network: route.same_venue
        ? entryOffer?.source ?? exitOffer?.source ?? "direct"
        : "cross-venue",
      target_amount_minor: Math.round(targetAmount * 100),
      target_currency: route.target_fiat,
      target_method_icon_url: targetMethod?.kind === "bank" ? paymentMethodFavicon(targetMethod) ?? undefined : undefined,
      route_kind: route.route_kind,
      bridge_currency: route.bridge_currency,
      market_path: route.market_path,
      spread_bps: relativeBps,
      is_current_best: index === 0,
      is_live_market: true,
      payment_methods_verified: route.payment_methods_verified,
      entry_offer_url: entryOffer?.source_url,
      entry_offer_is_exact: entryOffer?.source_url_is_exact,
      entry_offer_ad_id: entryOffer?.ad_id,
      exit_offer_url: exitOffer?.source_url,
      exit_offer_is_exact: exitOffer?.source_url_is_exact,
      exit_offer_ad_id: exitOffer?.ad_id,
      entry_offer_snapshot: entryOffer,
      exit_offer_snapshot: exitOffer,
      legs: route.market_path
        ? [
            {
              kind: "entry" as const,
              from: route.source_fiat,
              to: route.bridge_currency ?? route.target_fiat,
              provider: route.market_path.venue,
              status: "found" as const,
            },
            ...(route.bridge_currency
              ? [{
                  kind: "exit" as const,
                  from: route.bridge_currency,
                  to: route.target_fiat,
                  provider: route.market_path.venue,
                  status: "found" as const,
                }]
              : []),
          ]
        : [
        ...(entryOffer
          ? [{
              kind: "entry" as const,
              from: route.source_fiat,
              to: route.bridge_currency ?? route.asset,
              provider: entryOffer.source,
              status: "found" as const,
            }]
          : []),
        ...(exitOffer
          ? [{
              kind: "exit" as const,
              from: route.bridge_currency ?? route.asset,
              to: route.target_fiat,
              provider: exitOffer.source,
              status: "found" as const,
            }]
          : []),
          ],
    };
  });
}

export function Converter() {
  const [corridors, setCorridors] = useState<ExchangeCorridor[]>([]);
  const [corridorId, setCorridorId] = useState("");
  const [amount, setAmount] = useState("0");
  const [routes, setRoutes] = useState<RouteCandidate[]>([]);
  const [selected, setSelected] = useState<RouteCandidate | null>(null);
  const [instructionsRoute, setInstructionsRoute] = useState<RouteCandidate | null>(null);
  const [sourceMethodId, setSourceMethodId] = useState("am-ameriabank");
  const [targetMethodId, setTargetMethodId] = useState("ru-sberbank");
  const [directionReversed, setDirectionReversed] = useState(false);
  const [methodPicker, setMethodPicker] = useState<"source" | "target" | null>(null);
  const [networks, setNetworks] = useState<CryptoNetwork[]>([FALLBACK_NETWORK]);
  const [sourceNetworkId, setSourceNetworkId] = useState(FALLBACK_NETWORK.id);
  const [targetNetworkId, setTargetNetworkId] = useState(FALLBACK_NETWORK.id);
  const [networkPicker, setNetworkPicker] = useState<"source" | "target" | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [refreshSeconds, setRefreshSeconds] = useState<RefreshSeconds>(15);
  const [selectedSources, setSelectedSources] = useState<P2pSource[]>(DEFAULT_P2P_SOURCES);
  // An empty selection means “all available” and lets the backend use its full catalog.
  const [selectedIntermediaryAssets, setSelectedIntermediaryAssets] = useState<string[]>([]);
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
        const savedCorridorId = window.localStorage.getItem(CORRIDOR_STORAGE_KEY);
        const savedSourceMethodId = window.localStorage.getItem(SOURCE_METHOD_STORAGE_KEY);
        const savedTargetMethodId = window.localStorage.getItem(TARGET_METHOD_STORAGE_KEY);
        const savedDirection = window.localStorage.getItem(DIRECTION_STORAGE_KEY);
        const savedSources = window.localStorage.getItem(SOURCES_STORAGE_KEY);
        const savedIntermediaryAssets = window.localStorage.getItem(INTERMEDIARY_ASSETS_STORAGE_KEY);
        if (sharedExchange?.amount) setAmount(sharedExchange.amount);
        else if (savedAmount) setAmount(savedAmount);
        if (savedCorridorId) setCorridorId(savedCorridorId);
        if (savedSourceMethodId) setSourceMethodId(savedSourceMethodId);
        if (savedTargetMethodId) setTargetMethodId(savedTargetMethodId);
        if (savedDirection != null) setDirectionReversed(savedDirection === "true");
        if (savedSources) {
          const parsedSources = savedSources.split(",").filter(
            (source): source is P2pSource =>
              P2P_SOURCES.some((available) => available.id === source),
          );
          if (parsedSources.length > 0) setSelectedSources([...new Set(parsedSources)]);
        }
        if (savedIntermediaryAssets != null) {
          const parsedAssets = savedIntermediaryAssets
            .split(",")
            .filter((asset): asset is (typeof INTERMEDIARY_ASSETS)[number] =>
              INTERMEDIARY_ASSETS.includes(asset as (typeof INTERMEDIARY_ASSETS)[number]),
            );
          setSelectedIntermediaryAssets([...new Set(parsedAssets)]);
        }

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
    fetchNetworks()
      .then((items) => {
        if (items.length === 0) return;
        setNetworks(items);
      })
      .catch(() => {
        // Keep the Ethereum fallback visible while the backend is unavailable.
      });
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

  useEffect(() => {
    if (!preferencesLoadedRef.current) return;
    try {
      window.localStorage.setItem(SOURCES_STORAGE_KEY, selectedSources.join(","));
    } catch {
      // Local storage can be unavailable when the browser blocks site data.
    }
  }, [selectedSources]);

  useEffect(() => {
    if (!preferencesLoadedRef.current) return;
    try {
      window.localStorage.setItem(INTERMEDIARY_ASSETS_STORAGE_KEY, selectedIntermediaryAssets.join(","));
    } catch {
      // Local storage can be unavailable when the browser blocks site data.
    }
  }, [selectedIntermediaryAssets]);

  useEffect(() => {
    if (!preferencesLoadedRef.current) return;
    try {
      window.localStorage.setItem(CORRIDOR_STORAGE_KEY, corridorId);
    } catch {
      // Local storage can be unavailable when the browser blocks site data.
    }
  }, [corridorId]);

  useEffect(() => {
    if (!preferencesLoadedRef.current) return;
    try {
      window.localStorage.setItem(SOURCE_METHOD_STORAGE_KEY, sourceMethodId);
    } catch {
      // Local storage can be unavailable when the browser blocks site data.
    }
  }, [sourceMethodId]);

  useEffect(() => {
    if (!preferencesLoadedRef.current) return;
    try {
      window.localStorage.setItem(TARGET_METHOD_STORAGE_KEY, targetMethodId);
    } catch {
      // Local storage can be unavailable when the browser blocks site data.
    }
  }, [targetMethodId]);

  useEffect(() => {
    if (!preferencesLoadedRef.current) return;
    try {
      window.localStorage.setItem(DIRECTION_STORAGE_KEY, String(directionReversed));
    } catch {
      // Local storage can be unavailable when the browser blocks site data.
    }
  }, [directionReversed]);

  const corridor = useMemo(
    () => corridors.find((item) => item.id === corridorId) ?? corridors[0],
    [corridorId, corridors],
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
    () => [
      ...(sourceCountry ? paymentMethodsFor(sourceCountry, sourceCurrency, "sender") : []),
      ...DIGITAL_ASSETS,
    ],
    [sourceCountry, sourceCurrency],
  );
  const targetMethods = useMemo(
    () => [
      ...(targetCountry ? paymentMethodsFor(targetCountry, targetCurrency, "recipient") : []),
      ...DIGITAL_ASSETS,
    ],
    [targetCountry, targetCurrency],
  );
  const sourceMethod =
    sourceMethods.find((method) => method.id === sourceMethodId) ??
    (sourceCountry ? sourceMethods[0] : null);
  const targetMethod =
    targetMethods.find((method) => method.id === targetMethodId) ??
    (targetCountry ? targetMethods[0] : null);
  const sourceNetworks = sourceMethod?.kind === "wallet"
    ? networks.filter((network) => network.currencies.includes(sourceMethod.currency))
    : [];
  const targetNetworks = targetMethod?.kind === "wallet"
    ? networks.filter((network) => network.currencies.includes(targetMethod.currency))
    : [];
  const sourceNetwork =
    sourceNetworks.find((network) => network.id === sourceNetworkId) ?? sourceNetworks[0];
  const targetNetwork =
    targetNetworks.find((network) => network.id === targetNetworkId) ?? targetNetworks[0];
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
        const savedCorridorId = window.localStorage.getItem(CORRIDOR_STORAGE_KEY);
        const savedDirection = window.localStorage.getItem(DIRECTION_STORAGE_KEY);
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
        setCorridorId(
          (current) =>
            current || sharedCorridor?.id || savedCorridorId || response.items[0]?.id || "",
        );
        if (
          sharedExchange &&
          sharedCorridor?.source_currency === sharedExchange.targetCurrency &&
          sharedCorridor.target_currency === sharedExchange.sourceCurrency
        ) {
          setDirectionReversed(true);
        } else if (savedDirection != null) {
          setDirectionReversed(savedDirection === "true");
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
    setInstructionsRoute(null);
    setLastUpdatedAt(null);
    setSearching(false);
    setError(null);
  };

  const swapDirection = () => {
    if (!corridor) return;
    const nextSourceMethodId = targetMethod?.id ?? "";
    const nextTargetMethodId = sourceMethod?.id ?? "";
    if (previewRoute?.target_amount_minor != null) {
      setAmount(amountFromMinor(previewRoute.target_amount_minor));
    }
    setDirectionReversed((current) => !current);
    setSourceMethodId(nextSourceMethodId);
    setTargetMethodId(nextTargetMethodId);
    setSourceNetworkId(targetNetwork?.id ?? FALLBACK_NETWORK.id);
    setTargetNetworkId(sourceNetwork?.id ?? FALLBACK_NETWORK.id);
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
      const sourceIsWallet = sourceMethod.kind === "wallet";
      const targetIsWallet = targetMethod.kind === "wallet";
      if ((sourceIsWallet && !sourceNetwork) || (targetIsWallet && !targetNetwork)) {
        throw new Error("No compatible network is available for the selected cryptocurrency");
      }
      const response = await fetchP2pRoutes({
        sourceFiat: sourceIsWallet ? sourceMethod.currency : sourceCurrency,
        targetFiat: targetIsWallet ? targetMethod.currency : targetCurrency,
        sourceAmount: value,
        intermediaryAssets:
          !sourceIsWallet && !targetIsWallet && selectedIntermediaryAssets.length > 0
            ? selectedIntermediaryAssets
            : undefined,
        sourceNetwork: sourceIsWallet ? sourceNetwork?.id : undefined,
        targetNetwork: targetIsWallet ? targetNetwork?.id : undefined,
        sourcePaymentMethod: sourceIsWallet ? undefined : sourceMethod.p2pQuery,
        targetPaymentMethod: targetIsWallet ? undefined : targetMethod.p2pQuery,
        sources: selectedSources,
        allowCrossVenue: true,
        limit: 40,
        signal: controller.signal,
      });
      if (requestId !== requestRef.current) return;
      const liveRoutes = mapRoutes(response, sourceMethod, targetMethod);
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
  }, [amount, corridor, selectedIntermediaryAssets, selectedSources, sourceCurrency, sourceMethod, sourceNetwork, targetCurrency, targetMethod, targetNetwork]);

  // Re-run the read-only market search after the user changes the intent.
  // The old result is cleared immediately by updateAmount/applyOrientation,
  // but no new request was scheduled afterwards, leaving the panel empty (or
  // stuck in its loading state after a refresh). Debouncing also prevents a
  // request for every keystroke while the amount is being entered.
  useEffect(() => {
    if (
      !preferencesLoadedRef.current ||
      !urlReadyRef.current ||
      !corridor ||
      !sourceMethod ||
      !targetMethod ||
      !hasAmount
    ) {
      return;
    }

    const timer = window.setTimeout(() => void startSearch(), 650);
    return () => window.clearTimeout(timer);
  }, [corridor, hasAmount, sourceMethod, startSearch, targetMethod]);

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
    setAmount(normalizeAmountInput(value));
    resetResults();
  };

  return (
    <section className={styles.shell} id="transfer">
      <div className={styles.hero}>
        <h1>Move money. <span>Keep more.</span></h1>
        <p>Stop spending hours searching for an exchange.</p>
      </div>

      <div className={styles.workspace}>
        <div className={styles.card}>
          <div className={styles.cardTop}>
            <div className={styles.modeTabs} aria-label="Exchange mode">
              <button type="button" className={styles.modeActive}>Bridge</button>
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
                    <div className={styles.sourceSettings}>
                      <span className={styles.sourceSettingsLabel}>Search exchanges</span>
                      <div className={`${styles.sourceOptions} ${styles.exchangeOptions}`} aria-label="Exchanges to search">
                        {P2P_SOURCES.map((source) => {
                          const enabled = selectedSources.includes(source.id);
                          return (
                            <button
                              key={source.id}
                              type="button"
                              className={`${styles.sourceOption}${enabled ? ` ${styles.sourceOptionActive}` : ""}`}
                              aria-pressed={enabled}
                              onClick={() => {
                                setSelectedSources((current) => {
                                  if (enabled) {
                                    if (current.length === 1) return current;
                                    return current.filter((item) => item !== source.id);
                                  }
                                  return [...current, source.id];
                                });
                                resetResults();
                              }}
                            >
                              <span className={styles.sourceOptionIcon} aria-hidden="true">
                                <img
                                  src={source.iconUrl}
                                  alt=""
                                  onError={(event) => {
                                    event.currentTarget.onerror = null;
                                    event.currentTarget.src = `https://www.google.com/s2/favicons?domain=${source.id === "rapira" ? "rapira.net" : `${source.id}.com`}&sz=64`;
                                  }}
                                />
                              </span>
                              {source.label}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                    <div className={styles.sourceSettings}>
                      <div className={styles.intermediarySettingsHead}>
                        <span className={styles.sourceSettingsLabel}>Cryptocurrency intermediary</span>
                        <small>{selectedIntermediaryAssets.length ? `${selectedIntermediaryAssets.length} selected` : "All available"}</small>
                      </div>
                      <div className={`${styles.sourceOptions} ${styles.intermediaryOptions}`} aria-label="Cryptocurrency intermediaries">
                        <button
                          type="button"
                          className={`${styles.sourceOption}${selectedIntermediaryAssets.length === 0 ? ` ${styles.sourceOptionActive}` : ""}`}
                          aria-pressed={selectedIntermediaryAssets.length === 0}
                          onClick={() => {
                            setSelectedIntermediaryAssets([]);
                            resetResults();
                          }}
                        >
                          All available
                        </button>
                        {INTERMEDIARY_ASSETS.map((asset) => {
                          const enabled = selectedIntermediaryAssets.includes(asset);
                          return (
                            <button
                              key={asset}
                              type="button"
                              className={`${styles.sourceOption}${enabled ? ` ${styles.sourceOptionActive}` : ""}`}
                              aria-pressed={enabled}
                              onClick={() => {
                                setSelectedIntermediaryAssets((current) =>
                                  enabled ? current.filter((item) => item !== asset) : [...current, asset],
                                );
                                resetResults();
                              }}
                            >
                              <span className={styles.intermediaryAssetIcon} aria-hidden="true">
                                <img
                                  src={intermediaryIconUrl(asset)}
                                  alt=""
                                  onError={(event) => {
                                    event.currentTarget.onerror = null;
                                    event.currentTarget.src = `${CRYPTO_ICON_CDN}/generic.png`;
                                  }}
                                />
                              </span>
                              {asset}
                            </button>
                          );
                        })}
                      </div>
                    </div>
                    <p>Search also runs automatically 650ms after you change the amount, bank or intermediary.</p>
                  </div>
                )}
              </div>
            </div>
          </div>

          <div className={styles.intentLabel}>
            <span>Sell</span>
          </div>

          <div className={`${styles.moneyPanel} ${styles.moneyPanelSource}`}>
            <div className={styles.panelCopy}>
              <label htmlFor="exchange-amount">You send</label>
              <input
                id="exchange-amount"
                className={styles.amountInput}
                type="text"
                inputMode="decimal"
                autoComplete="off"
                spellCheck={false}
                value={amount}
                onFocus={(event) => event.currentTarget.select()}
                onKeyDown={(event) => {
                  const isSeparator = ["Comma", "Period", "NumpadDecimal", "Decimal"].includes(event.code)
                    || event.key === ","
                    || event.key === ".";
                  if (!isSeparator) return;

                  event.preventDefault();
                  const input = event.currentTarget;
                  const start = input.selectionStart ?? input.value.length;
                  const end = input.selectionEnd ?? start;
                  const separator = event.key === "," || event.code === "Comma" ? "," : ".";
                  const nextValue = normalizeAmountInput(
                    `${input.value.slice(0, start)}${separator}${input.value.slice(end)}`,
                  );
                  updateAmount(nextValue);
                  window.requestAnimationFrame(() => {
                    const cursor = nextValue.length;
                    input.setSelectionRange(cursor, cursor);
                  });
                }}
                onChange={(event) => updateAmount(event.target.value)}
                aria-label="Amount to send"
              />
              <span className={styles.currencyHint}>
                {sourceMethod?.currency || sourceCurrency || "AMD"} available via {sourceMethod?.kind === "wallet" ? "digital wallet" : "bank transfer"}
              </span>
            </div>
            <div className={styles.methodControls}>
              <button
                type="button"
                className={styles.methodTrigger}
                onClick={() => setMethodPicker("source")}
                aria-label={`Select sending ${sourceMethod?.kind === "wallet" ? "asset" : "bank"}: ${sourceMethod?.name ?? "none"}`}
              >
                <BankLogo
                  className={styles.methodAvatar}
                  method={sourceMethod}
                  fallback={corridor?.source_country ?? "—"}
                />
                <span className={styles.methodText}>
                  <strong>{sourceMethod?.name ?? "Select bank"}</strong>
                  <small>
                    {sourceMethod?.kind === "wallet"
                      ? `${sourceMethod.currency} · ${sourceNetwork?.name ?? "Loading networks…"}`
                      : corridor
                        ? locationLabel(sourceCountry, sourceCurrency)
                        : "Unavailable"}
                  </small>
                </span>
                <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path d="m4 6 4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </button>
              {sourceMethod?.kind === "wallet" && sourceNetwork && (
                <NetworkControl
                  network={sourceNetwork}
                  onOpen={() => setNetworkPicker("source")}
                />
              )}
            </div>
            <div className={styles.walletSupport}>
              <span>Supported wallets</span>
              <div>
                <b>◈</b><b>◉</b><b>W</b><small>+ more</small>
              </div>
            </div>
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
          </div>

          <div className={`${styles.intentLabel} ${styles.intentLabelBuy}`}>
            <span>Buy</span>
          </div>

          <div className={`${styles.moneyPanel} ${styles.moneyPanelTarget}`}>
            <div className={styles.panelCopy}>
              <label>Recipient gets</label>
              <output className={previewRoute ? styles.amountOutput : styles.amountOutputEmpty}>
                {amountFromMinor(previewRoute?.target_amount_minor)}
              </output>
              <span className={styles.currencyHint}>
                {targetMethod?.kind === "wallet"
                  ? `${targetMethod.currency} available via digital wallet`
                  : previewRoute
                    ? `Estimated ${previewRoute.target_currency}`
                    : "Live estimate appears here"}
              </span>
            </div>
            <div className={styles.methodControls}>
              <button
                type="button"
                className={styles.methodTrigger}
                onClick={() => setMethodPicker("target")}
                aria-label={`Select recipient ${targetMethod?.kind === "wallet" ? "asset" : "bank"}: ${targetMethod?.name ?? "none"}`}
              >
                <BankLogo
                  className={styles.methodAvatar}
                  method={targetMethod}
                  fallback={corridor?.target_country ?? "—"}
                />
                <span className={styles.methodText}>
                  <strong>{targetMethod?.name ?? "Select bank"}</strong>
                  <small>
                    {targetMethod?.kind === "wallet"
                      ? `${targetMethod.currency} · ${targetNetwork?.name ?? "Loading networks…"}`
                      : corridor
                        ? locationLabel(targetCountry, targetCurrency)
                        : "Unavailable"}
                  </small>
                </span>
                <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path d="m4 6 4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </button>
              {targetMethod?.kind === "wallet" && targetNetwork && (
                <NetworkControl
                  network={targetNetwork}
                  onOpen={() => setNetworkPicker("target")}
                />
              )}
            </div>
            <div className={styles.walletSupport}>
              <span>Supported wallets</span>
              <div>
                <b>◈</b><b>◉</b><b>W</b><small>+ more</small>
              </div>
            </div>
          </div>

          {refreshSeconds > 0 && (
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
                  <span>
                    {lastUpdatedAt
                      ? `Updated ${Math.max(0, Math.floor((clock - lastUpdatedAt) / 1_000))}s ago`
                    : "Public P2P sources only · no order placement"}
                  </span>
                </div>
              </div>
              {secondsUntilRefresh !== null && (
                <span className={styles.nextRefresh}>{secondsUntilRefresh}s</span>
              )}
            </div>
          )}

          <button
            type="button"
            className={styles.cta}
            disabled={!hasAmount || searching || !corridor}
            onClick={() => void startSearch()}
            data-testid="start-search"
            aria-label="Search routes"
          >
            {searching ? (
              <><span className={styles.spinner} /> Searching every path</>
            ) : hasAmount ? (
              <>Search routes <span>↗</span></>
            ) : (
              "Enter an amount to begin"
            )}
          </button>

          {error && <div className={styles.errorBox} role="alert">{error}</div>}
        </div>

        <SidePanel
          active
          routes={routes}
          selectedRouteId={selected?.route_id ?? null}
          onSelect={setSelected}
          onOpenInstructions={setInstructionsRoute}
          searching={searching}
          searched={lastUpdatedAt !== null}
          hasAmount={hasAmount}
        />
      </div>

      <PaymentMethodPicker
        open={methodPicker === "source"}
        title="Choose where you pay from"
        role="sender"
        selectedLocation={corridor ? { country: sourceCountry, currency: sourceCurrency } : null}
        selected={sourceMethod}
        onClose={() => setMethodPicker(null)}
        onSelect={chooseSourceMethod}
      />

      <PaymentMethodPicker
        open={methodPicker === "target"}
        title="Choose where the recipient gets paid"
        role="recipient"
        selectedLocation={corridor ? { country: targetCountry, currency: targetCurrency } : null}
        selected={targetMethod}
        onClose={() => setMethodPicker(null)}
        onSelect={chooseTargetMethod}
      />

      <NetworkPicker
        open={networkPicker !== null}
        networks={networkPicker === "source" ? sourceNetworks : targetNetworks}
        selected={networkPicker === "source" ? sourceNetwork : targetNetwork}
        onClose={() => setNetworkPicker(null)}
        onSelect={(network) => {
          if (networkPicker === "source") setSourceNetworkId(network.id);
          else setTargetNetworkId(network.id);
          setNetworkPicker(null);
          resetResults();
        }}
      />

      {instructionsRoute && (
        <RouteInstructions
          route={instructionsRoute}
          onClose={() => setInstructionsRoute(null)}
        />
      )}
    </section>
  );
}
