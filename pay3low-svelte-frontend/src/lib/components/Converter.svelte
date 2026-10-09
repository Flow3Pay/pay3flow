<script lang="ts">
  import { browser } from "$app/environment";
  import { afterUpdate, onMount, onDestroy, tick } from "svelte";
  import { quintOut } from "svelte/easing";
  import { fly, slide } from "svelte/transition";
  import { fetchCorridors, fetchMarketPrices, fetchP2pRoutes, fetchProviders, recordInstructionOpen, recordServiceOpen, setRouteVote, streamP2pRoutes, type ExchangeCorridor, type P2pRouteSearchResponse, type ProviderDefinition, type ProviderGuidance, type RouteCandidate, type ServiceLink, type ServiceStats, type ServiceVote, type VenueSearchStatus } from "$lib/exchange";
  import { FALLBACK_NETWORK, fetchNetworks, type CryptoNetwork } from "$lib/networks";
  import { assetIcon, networkIcon, venueIcon } from "$lib/icons";
  import { fetchPaymentMethods, paymentMethodFavicon, type PaymentMethod } from "$lib/payment-methods";
  import { getAnonymousUserId, registerAnonymousUser } from "$lib/anonymous-user";
  import { locale, t, setLocale } from "$lib/i18n";
  import { homeContent } from "$lib/home-content";
  import { SearchResponseMetrics } from "$lib/response-metrics";
  import { fetchRouteSearchActivity, SEARCH_ACTIVITY_PERIODS, type RouteSearchActivityHour, type SearchActivityPeriod } from "$lib/route-activity";
  import { fiatFlagUrl } from "$lib/currency-flags";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import type { ExchangeShareState } from "$lib/exchange-share";
  import { parseExchangeHash, guideHash, guideRoutePath, sharedIdentifiers } from "$lib/guide-link";
  import ExchangeFlowBridge from "./ExchangeFlowBridge.svelte";
  import SidePanel from "./SidePanel.svelte";
  import SearchActivityChart from "./SearchActivityChart.svelte";
  import SearchActivityModal from "./SearchActivityModal.svelte";
  import CurrencyPicker from "./CurrencyPicker.svelte";
  import NetworkPicker from "./NetworkPicker.svelte";

  export let onGuideChange: (active: boolean) => void = () => {};
  export let guideActive = false;
  let guideRequested = browser && Boolean(parseExchangeHash(window.location.hash)?.guide);
  let pendingGuide = browser ? parseExchangeHash(window.location.hash) : null;
  let lastLocationHash = browser ? window.location.hash : "";
  let guideUnavailable = Boolean(pendingGuide?.guide && !pendingGuide.amount);
  $: guideActive = guideRequested;
  $: onGuideChange(guideActive);

  export let onShareStateChange: (state: ExchangeShareState | null) => void = () => {};
  export let onPaymentMethodsLoaded: (items: PaymentMethod[]) => void = () => {};
  export let onProvidersLoaded: (items: ProviderDefinition[]) => void = () => {};
  export let onBelarusP2pWarningChange: (show: boolean) => void = () => {};
  export let onOpenBelarusP2pWarning: () => void = () => {};

  type RefreshSeconds = 0 | 5 | 15 | 30 | 60 | 300;
  type PickerSide = "source" | "target" | null;
  type AmountSide = "source" | "target";
  const REFRESH_OPTIONS: RefreshSeconds[] = [0, 5, 15, 30, 60, 300];
  type P2pSource = string;
  type ExchangeMethod = "p2p" | "exchanger";
  type ProviderSearchMode = "selectable" | "always_on" | "catalog_only";
  type P2pSourceOption = { id: P2pSource; label: string; iconUrl: string; searchable: boolean; searchMode: ProviderSearchMode; feeDescription?: string };
  const INITIAL_ROUTE_BATCH_SIZE = 100;
  const ROUTE_BATCH_SIZE = 100;
  const ROUTE_BATCH_DELAY_MS = 10;
  const INTERNAL_DISCOVERY_SOURCES = new Set(["fmatch", "database_cache", "provider_fallback", "provider"]);
  let INTERMEDIARY_ASSETS: string[] = [];
  const EXCHANGE_METHODS: ExchangeMethod[] = ["p2p", "exchanger"];
  let paymentMethods: PaymentMethod[] = [];
  const STORAGE = { amount: "pay3flow.exchange.amount", refresh: "pay3flow.exchange.refresh-seconds", activityPeriod: "pay3flow.exchange.activity-period", activityVisible: "pay3flow.exchange.activity-visible", routesVisible: "pay3flow.exchange.routes-visible", sources: "pay3flow.exchange.p2p-sources", knownSources: "pay3flow.exchange.known-p2p-sources", methods: "pay3flow.exchange.methods", corridor: "pay3flow.exchange.corridor", sourceMethod: "pay3flow.exchange.source-method", targetMethod: "pay3flow.exchange.target-method", sourceNetwork: "pay3flow.exchange.source-network", targetNetwork: "pay3flow.exchange.target-network", direction: "pay3flow.exchange.direction-reversed", assets: "pay3flow.exchange.intermediary-assets" };

  let corridors: ExchangeCorridor[] = [];
  let corridorId = "";
  let amount = "0";
  let targetAmount = "0";
  let amountSide: AmountSide = "source";
  let targetAmountNeedsRate = false;
  let targetProbeAmount: number | null = null;
  let targetRefinementAttempts = 0;
  let targetQuoteRouteKey: string | null = null;
  let routes: RouteCandidate[] = [];
  let routesFound = 0;
  let selected: RouteCandidate | null = null;
  let selectionPinnedByUser = false;
  let instructionsRoute: RouteCandidate | null = null;
  let sourceMethodId = "am-ameriabank";
  let targetMethodId = "ru-sberbank";
  let directionReversed = false;
  let methodPicker: PickerSide = null;
  let currencyPicker: PickerSide = null;
  let networkPicker: PickerSide = null;
  let networks: CryptoNetwork[] = [FALLBACK_NETWORK];
  let sourceNetworkId = FALLBACK_NETWORK.id;
  let targetNetworkId = FALLBACK_NETWORK.id;
  let settingsOpen = false;
  let exchangesOpen = false;
  let showBelarusP2pWarning = false;
  let refreshSeconds: RefreshSeconds = 15;
  let p2pSources: P2pSourceOption[] = [];
  let venueNames: Record<string, string> = {};
  let venueUrls: Record<string, string> = {};
  let providerGuidance: Record<string, ProviderGuidance> = {};
  let selectedSources: P2pSource[] = [];
  let selectedExchangeMethods: ExchangeMethod[] = [...EXCHANGE_METHODS];
  let selectedIntermediaryAssets: string[] = [];
  let searching = false;
  let awaitingFirstRoute = false;
  let lastUpdatedAt: number | null = null;
  let clock = Date.now();
  let error: string | null = null;
  let anonymousId = "";
  let activityHours: RouteSearchActivityHour[] = [];
  let activityPeriod: SearchActivityPeriod = "1w";
  let activityLoading = false;
  let activityError = false;
  let activityKey = "";
  let activityDisplayKey = "";
  let activityController: AbortController | null = null;
  let activityModalOpen = false;
  let activityExpanded = false;
  let activityRevealElement: HTMLDivElement | undefined;
  let routesRevealElement: HTMLDivElement | undefined;
  let routesExpanded = true;
  // The animation is added after mounting; server content never depends on it.
  let introPlaying = false;
  let introStarted = false;
  let introCompleted = false;
  let introOverlayElement: HTMLDivElement | undefined;
  let heroHeadingElement: HTMLHeadingElement | undefined;
  let cleanupIntro: () => void = () => {};
  let settingsElement: HTMLDivElement;
  let settingsDialog: HTMLDivElement;
  let settingsDragging = false;
  let settingsDragStartY = 0;
  let settingsDragDistance = 0;
  let settingsWasOpen = false;
  let preferencesLoaded = false;
  let urlReady = false;
  let requestId = 0;
  let controller: AbortController | null = null;
  let debounceTimer: number | undefined;
  let refreshTimer: number | undefined;
  let clockTimer: number | undefined;
  let marketRefreshTimer: number | undefined;
  let marketController: AbortController | null = null;
  let marketPrices: Record<string, number> = {};
  let marketPricesUpdatedAt = 0;
  let marketUsd: { source: number | null; target: number | null } = { source: null, target: null };
  let initialSearchTimer: number | undefined;
  let routeRenderTimer: number | undefined;
  let routeRenderFrame: number | undefined;
  let routeRenderVersion = 0;
  let revealedRouteCount = 0;
  let renderingRoutes = false;
  let initialSearchReady = false;
  let converterCard: HTMLDivElement;
  let walkthroughOpen = false;
  let walkthroughComponent: typeof import("./ConverterWalkthrough.svelte").default | null = null;
  async function openWalkthrough() {
    walkthroughComponent ??= (await import("./ConverterWalkthrough.svelte")).default;
    walkthroughOpen = true;
  }
  let paymentPickerComponent: typeof import("./PaymentMethodPicker.svelte").default | null = null;
  let routeInstructionsComponent: typeof import("./RouteInstructions.svelte").default | null = null;
  let searchingVenues: P2pSourceOption[] = [];
  let foundVenues: P2pSourceOption[] = [];
  let venueStats: Record<string, VenueSearchStatus> = {};
  let responseMetrics = new SearchResponseMetrics();
  let foundVenueIds: string[] = [];
  $: activeLocale = $locale;
  $: home = homeContent[activeLocale];
  $: firstHeadline = headlineWords(t("Move money.", {}, activeLocale));
  $: secondHeadline = headlineWords(t("Keep more.", {}, activeLocale));
  $: modalOpen = settingsOpen || exchangesOpen;

  function headlineWords(value: string) {
    const match = value.match(/^(\S+)\s+(.+?)([.!։。؟]+)$/u);
    return match ? { first: match[1], second: match[2], punctuation: match[3] } : { first: value, second: "", punctuation: "" };
  }


  function finishIntro() {
    cleanupIntro();
    introPlaying = false;
    introCompleted = true;
  }

  onMount(() => {
    if (guideRequested || window.matchMedia("(prefers-reduced-motion: reduce)").matches) { introCompleted = true; return; }
    let frame: number | undefined;
    let timer: number | undefined;
    const positionIntro = () => {
      if (!introOverlayElement || !heroHeadingElement) return;
      const heading = heroHeadingElement.getBoundingClientRect();
      introOverlayElement.style.setProperty("--intro-x", `${heading.left + heading.width / 2 - window.innerWidth / 2}px`);
      introOverlayElement.style.setProperty("--intro-y", `${heading.top + heading.height / 2 - window.innerHeight / 2}px`);
    };
    cleanupIntro = () => {
      document.documentElement.classList.remove("introPlaying");
      window.removeEventListener("resize", positionIntro);
      if (frame !== undefined) window.cancelAnimationFrame(frame);
      if (timer !== undefined) window.clearTimeout(timer);
    };
    document.documentElement.classList.add("introPlaying");
    introPlaying = true;
    void tick().then(() => {
      if (!introPlaying) return;
      positionIntro();
      window.addEventListener("resize", positionIntro, { passive: true });
      frame = window.requestAnimationFrame(() => {
        if (!introPlaying) return;
        introStarted = true;
        timer = window.setTimeout(finishIntro, 5000);
      });
    });
    return () => {
      introPlaying = false;
      cleanupIntro();
    };
  });

  async function refreshActivity(source: string, target: string, period: SearchActivityPeriod, refreshedAt: number | null, interval: number) {
    const displayKey = `${source}|${target}|${period}`;
    const key = `${displayKey}|${refreshedAt ?? ""}|${interval}`;
    if (key === activityKey) return;
    activityKey = key;
    activityController?.abort();
    activityController = new AbortController();
    if (activityDisplayKey !== displayKey) activityHours = [];
    activityDisplayKey = displayKey;
    activityLoading = true;
    activityError = false;
    try {
      const response = await fetchRouteSearchActivity(source, target, period, activityController.signal);
      if (activityKey === key) activityHours = response.hours;
    } catch (error) {
      if (activityKey === key && !(error instanceof DOMException && error.name === "AbortError")) activityError = true;
    } finally {
      if (activityKey === key) activityLoading = false;
    }
  }

  $: if (typeof document !== "undefined" && !document.hidden && selectedSourceCurrency && selectedTargetCurrency) void refreshActivity(selectedSourceCurrency, selectedTargetCurrency, activityPeriod, lastUpdatedAt, Math.floor(clock / 30_000));

  function selectActivityPeriod(period: SearchActivityPeriod) {
    activityPeriod = period;
    try { localStorage.setItem(STORAGE.activityPeriod, period); } catch {}
  }

  function setActivityExpanded(expanded: boolean) {
    activityExpanded = expanded;
    try { localStorage.setItem(STORAGE.activityVisible, String(expanded)); } catch {}
  }

  function bringActivityIntoView(behavior: ScrollBehavior = "smooth") {
    void tick().then(() => window.requestAnimationFrame(() => {
      const chart = activityRevealElement;
      if (!chart) return;
      const bottom = chart.getBoundingClientRect().bottom;
      const visibleBottom = window.innerHeight - 16;
      if (bottom > visibleBottom) window.scrollTo({ top: window.scrollY + bottom - visibleBottom, behavior });
    }));
  }

  function toggleActivityGraph() {
    if (window.matchMedia("(max-width: 980px)").matches) { activityExpanded = false; activityModalOpen = true; }
    else { setActivityExpanded(!activityExpanded); if (activityExpanded && !routesExpanded) bringActivityIntoView(); }
  }

  function toggleRoutes() {
    routesExpanded = !routesExpanded;
    if (routesRevealElement) routesRevealElement.inert = !routesExpanded;
    if (routesExpanded && window.matchMedia("(max-width: 980px)").matches) {
      void tick().then(() => window.requestAnimationFrame(() => {
        const panel = routesRevealElement;
        if (!panel) return;
        window.scrollTo({
          top: window.scrollY + panel.getBoundingClientRect().top - 16,
          behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth"
        });
      }));
    }
    if (!routesExpanded && activityExpanded) bringActivityIntoView();
    try { localStorage.setItem(STORAGE.routesVisible, String(routesExpanded)); } catch {}
  }

  function routesRevealTransition(node: HTMLElement) {
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const mobile = window.matchMedia("(max-width: 980px)").matches;
    const duration = reducedMotion ? 0 : 420;
    if (!mobile) return fly(node, { x: 56, duration, easing: quintOut });

    const collapse = slide(node, { duration, easing: quintOut });
    return {
      ...collapse,
      css: (t: number, u: number) => `${collapse.css?.(t, u) ?? ""}; transform: translateY(${u * 24}px); opacity: ${t};`
    };
  }

  function closeInlineActivityOnMobile() {
    if (window.matchMedia("(max-width: 980px)").matches) activityExpanded = false;
  }

  function readSharedExchange() {
    return parseExchangeHash(window.location.hash);
  }
  function sharedMethod(methods: PaymentMethod[], currency: string, role: "sender" | "recipient", currentId: string) {
    const choices = methods.filter((method) => method.kind !== "currency" && method.currency === currency && (method.role === role || method.role === "both"));
    return choices.find((method) => method.id === currentId)
      ?? choices.find((method) => method.kind === "wallet")
      ?? choices.find((method) => method.popular)
      ?? choices[0];
  }
  function normalizeAmount(value: string) {
    const sanitized = value.normalize("NFKC").replace(/[\u00a0\u200b-\u200d\ufeff]/g, "").replace(/[бБ]/g, ",").replace(/[юЮ٫٬]/g, ".").replace(/[^0-9.,]/g, "");
    const separator = Math.max(sanitized.lastIndexOf("."), sanitized.lastIndexOf(","));
    if (separator < 0) return sanitized.replace(/^0+(?=\d)/, "") || "0";
    const integer = sanitized.slice(0, separator).replace(/[.,]/g, "").replace(/^0+(?=\d)/, "") || "0";
    return `${integer}${sanitized[separator]}${sanitized.slice(separator + 1).replace(/[.,]/g, "")}`;
  }
  const amountNumber = (value: string) => Number(normalizeAmount(value).replace(",", "."));
  let CRYPTO_CURRENCIES = new Set<string>();
  const amountFromMinor = (minor?: number) => minor == null ? "0" : (minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2, useGrouping: false });
  const refreshOptionLabel = (seconds: RefreshSeconds) => seconds === 0 ? "Off" : seconds < 60 ? `${seconds}s` : `${seconds / 60}m`;
  const amountFromRoute = (route?: RouteCandidate | null) => {
    if (!route) return "0";
    if (route.target_amount && CRYPTO_CURRENCIES.has(route.target_currency?.toUpperCase() ?? "")) {
      return Number(route.target_amount).toLocaleString("en-US", { maximumFractionDigits: 8, useGrouping: false });
    }
    return amountFromMinor(route.target_amount_minor);
  };
  const formatEditableAmount = (value: number, currency: string) => {
    if (!Number.isFinite(value) || value <= 0) return "0";
    return value.toLocaleString("en-US", {
      maximumFractionDigits: CRYPTO_CURRENCIES.has(currency.toUpperCase()) ? 8 : 2,
      useGrouping: false,
    });
  };
  const intermediaryIcon = (asset: string) => paymentMethods.find((method) => method.kind === "wallet" && method.currency === asset)?.iconUrl ?? assetIcon(asset);
  const networkName = (id: string | null | undefined) => !id ? "internal" : networks.find((network) => network.id === id)?.name ?? id;
  const methodNoun = (method: PaymentMethod | null | undefined) => method?.kind === "wallet" ? "asset" : method?.kind === "cash" ? "payment method" : "bank";
  const methodTitle = (method: PaymentMethod | null | undefined) => method?.kind === "wallet" ? method.currency : method?.name ?? "Select payment method";
  const currencyMark = (currency: string) => currency === "USD" ? "$" : currency === "RUB" ? "₽" : currency === "AMD" ? "֏" : currency === "BYN" ? "Br" : currency === "UAH" ? "₴" : currency === "KZT" ? "₸" : currency.slice(0, 1);

  function providerLabel(provider: ProviderDefinition) {
    const label = provider.name.replace(/\s+(buy|sell)$/i, "").trim();
    return label || provider.slug;
  }

  function routeFamilyKey(route: RouteCandidate) {
    const entry = route.entry_offer_snapshot;
    const exit = route.exit_offer_snapshot;
    return [
      route.route_kind,
      route.source_currency,
      route.target_currency,
      route.source_network,
      route.target_network,
      route.entry_asset,
      route.entry_network,
      route.bridge_currency,
      route.route_provider,
      route.route_path?.join(","),
      entry ? `${entry.source}:${entry.ad_id}:${entry.payment_methods.join(",")}` : "",
      exit ? `${exit.source}:${exit.ad_id}:${exit.payment_methods.join(",")}` : "",
      route.market_path ? `${route.market_path.venue}:${route.market_path.source_pair}:${route.market_path.target_pair}` : "",
    ].join("|");
  }

  function providerSources(providers: ProviderDefinition[]): P2pSourceOption[] {
    const sources = new Map<string, P2pSourceOption>();
    for (const provider of providers) {
      const existing = sources.get(provider.slug);
      const searchMode = provider.search_mode ?? (provider.searchable ? "selectable" : "catalog_only");
      if (existing) {
        existing.searchable ||= provider.searchable;
        if (searchMode === "always_on" || (searchMode === "selectable" && existing.searchMode === "catalog_only")) existing.searchMode = searchMode;
        existing.feeDescription ||= provider.fee_model?.description;
      } else {
        sources.set(provider.slug, {
          id: provider.slug,
          label: providerLabel(provider),
          iconUrl: venueIcon(provider.slug),
          searchable: provider.searchable,
          searchMode,
          feeDescription: provider.fee_model?.description,
        });
      }
    }
    return [...sources.values()].sort((left, right) => left.label.localeCompare(right.label));
  }

  const sourceTitle = (source: P2pSourceOption) => {
    const action = source.searchMode === "always_on" ? `Always compare ${source.label}` : source.searchable ? `Search ${source.label}` : `Select ${source.label} (catalog-only)`;
    return source.feeDescription ? `${action}. Fee model: ${source.feeDescription}` : action;
  };

  function mapRoutes(response: Awaited<ReturnType<typeof fetchP2pRoutes>>): RouteCandidate[] {
    const bestTarget = Number(response.routes[0]?.target_amount ?? 0);
    return response.routes.map((route, index) => {
    const entryOffer = route.entry_offer, exitOffer = route.exit_offer, targetAmount = Number(route.target_amount);
      const legs = route.cycle_legs?.length
        ? route.cycle_legs.map((leg, index) => ({ kind: index === 0 ? "entry" as const : "exit" as const, from: leg.from_asset, to: leg.to_asset, provider: leg.provider, status: "found" as const }))
        : route.route_provider
        ? [
            ...(entryOffer
              ? [{ kind: "entry" as const, from: route.source_fiat, to: entryOffer.asset, provider: entryOffer.source, status: "found" as const }]
              : [{ kind: "entry" as const, from: route.source_fiat, to: route.asset, provider: route.route_provider, status: "found" as const }]),
            ...(exitOffer
              ? [{ kind: "exit" as const, from: exitOffer.asset, to: route.target_fiat, provider: exitOffer.source, status: "found" as const }]
              : entryOffer
                ? [{ kind: "exit" as const, from: entryOffer.asset, to: route.target_fiat, provider: route.route_provider, status: "found" as const }]
                : []),
          ]
        : route.market_path
          ? [{ kind: "entry" as const, from: route.source_fiat, to: route.bridge_currency ?? route.target_fiat, provider: route.market_path.venue, status: "found" as const }, ...(route.bridge_currency ? [{ kind: "exit" as const, from: route.bridge_currency, to: route.target_fiat, provider: route.market_path.venue, status: "found" as const }] : [])]
          : [...(entryOffer ? [{ kind: "entry" as const, from: route.source_fiat, to: route.bridge_currency ?? route.asset, provider: entryOffer.source, status: "found" as const }] : []), ...(exitOffer ? [{ kind: "exit" as const, from: route.bridge_currency ?? route.asset, to: route.target_fiat, provider: exitOffer.source, status: "found" as const }] : [])];
      const fallbackRouteId = route.market_path
        ? `spot:${route.market_path.venue}:${route.market_path.source_pair}:${route.market_path.target_pair}`
        : `live:${entryOffer?.source ?? "direct"}:${entryOffer?.ad_id ?? "none"}:${exitOffer?.source ?? "direct"}:${exitOffer?.ad_id ?? "none"}:${route.route_path?.join(",") ?? ""}`;
      return {
        route_id: route.route_id?.trim() || fallbackRouteId,
        status: "complete", source_amount_minor: Math.round(Number(route.source_amount) * 100), source_amount: route.source_amount, source_currency: route.source_fiat,
        source_payment_method: sourceMethod?.kind !== "wallet" ? sourceMethod?.name : undefined,
        target_payment_method: targetMethod?.kind !== "wallet" ? targetMethod?.name : undefined,
        source_bank_fee_percent: sourceMethod?.kind === "bank" ? sourceMethod.bankFeePercent : undefined,
        target_bank_fee_percent: targetMethod?.kind === "bank" ? targetMethod.bankFeePercent : undefined,
        source_method_icon_url: sourceMethod?.kind !== "wallet" ? paymentMethodFavicon(sourceMethod) ?? undefined : undefined,
        entry_asset: route.asset, entry_network: networkName(route.entry_network), source_network: route.source_network ? networkName(route.source_network) : undefined, target_network: route.target_network ? networkName(route.target_network) : undefined,
        target_amount_minor: Math.round(targetAmount * 100), target_amount: route.target_amount, target_currency: route.target_fiat,
        target_method_icon_url: targetMethod?.kind !== "wallet" ? paymentMethodFavicon(targetMethod) ?? undefined : undefined,
        route_kind: route.route_kind, profitability: route.profitability, cycle_legs: route.cycle_legs, profitability_decimals: route.profitability_decimals, bridge_currency: route.bridge_currency, market_path: route.market_path,
        route_provider: route.route_provider, route_provider_url: route.route_provider_url, route_path: route.route_path, route_fees: route.route_fees, quote_expires_at: route.quote_expires_at, execution: route.execution,
        spread_bps: bestTarget > 0 && Number.isFinite(targetAmount) ? Math.round((targetAmount / bestTarget - 1) * 10_000) : 0,
        is_current_best: index === 0, is_live_market: true, payment_methods_verified: route.payment_methods_verified,
        entry_offer_url: entryOffer?.source_url, entry_offer_is_exact: entryOffer?.source_url_is_exact, entry_offer_ad_id: entryOffer?.ad_id,
        exit_offer_url: exitOffer?.source_url, exit_offer_is_exact: exitOffer?.source_url_is_exact, exit_offer_ad_id: exitOffer?.ad_id,
        entry_offer_snapshot: entryOffer, exit_offer_snapshot: exitOffer, warnings: route.warnings,
        services: route.services, reputation: route.reputation, feedback: route.feedback ?? { likes_total: 0, dislikes_total: 0 }, service_links: route.service_links,
        legs,
      };
    });
  }

  function foundVenueOptions(): P2pSourceOption[] {
    return foundVenueIds.map((id) =>
      p2pSources.find((item) => item.id.toLowerCase() === id)
        ?? { id, label: id.split("-").map((part) => part.charAt(0).toUpperCase() + part.slice(1)).join(" "), iconUrl: venueIcon(id), searchable: true, searchMode: "selectable" as const },
    );
  }

  function cancelRouteRendering() {
    routeRenderVersion += 1;
    renderingRoutes = false;
    if (routeRenderTimer !== undefined) window.clearTimeout(routeRenderTimer);
    if (routeRenderFrame !== undefined) window.cancelAnimationFrame(routeRenderFrame);
    routeRenderTimer = undefined;
    routeRenderFrame = undefined;
  }

  function displayRoutes(nextRoutes: RouteCandidate[]) {
    routes = nextRoutes;
  }

  function renderRoutesProgressively(nextRoutes: RouteCandidate[]) {
    cancelRouteRendering();
    const version = routeRenderVersion;
    const initialCount = Math.min(Math.max(INITIAL_ROUTE_BATCH_SIZE, revealedRouteCount), nextRoutes.length);
    displayRoutes(nextRoutes.slice(0, initialCount));
    revealedRouteCount = initialCount;
    renderingRoutes = initialCount < nextRoutes.length;
    if (!renderingRoutes) return;

    const scheduleNextBatch = (callback: () => void) => {
      routeRenderFrame = window.requestAnimationFrame(() => {
        routeRenderFrame = undefined;
        if (version !== routeRenderVersion) return;
        routeRenderTimer = window.setTimeout(callback, ROUTE_BATCH_DELAY_MS);
      });
    };
    const revealNextBatch = () => {
      if (version !== routeRenderVersion) return;
      routeRenderTimer = undefined;
      const nextCount = Math.min(revealedRouteCount + ROUTE_BATCH_SIZE, nextRoutes.length);
      displayRoutes(nextRoutes.slice(0, nextCount));
      revealedRouteCount = nextCount;
      renderingRoutes = nextCount < nextRoutes.length;
      if (renderingRoutes) scheduleNextBatch(revealNextBatch);
    };
    scheduleNextBatch(revealNextBatch);
  }

  function applySearchResponse(response: P2pRouteSearchResponse) {
    const nextRoutes = mapRoutes(response);
    const routeCounts = new Map<string, number>();
    for (const route of nextRoutes) {
      const providers = new Set([...route.legs.map((leg) => leg.provider), ...(route.route_provider ? [route.route_provider] : [])].map((provider) => provider.toLowerCase()));
      for (const provider of providers) routeCounts.set(provider, (routeCounts.get(provider) ?? 0) + 1);
    }
    const timings = responseMetrics.observe(response);
    const observedVenueStats = new Map<string, VenueSearchStatus>();
    for (const status of [...(response.asset_statuses ?? []).flatMap((asset) => [...asset.entry_sources, ...asset.exit_sources]), ...(response.provider_statuses ?? [])]) {
      const id = status.source.toLowerCase();
      if (INTERNAL_DISCOVERY_SOURCES.has(id)) continue;
      const previous = observedVenueStats.get(id);
      const timing = timings.get(id);
      observedVenueStats.set(id, {
        ...status,
        ok: (previous?.ok ?? true) && status.ok,
        latency_ms: timing?.average_response_ms ?? 0,
        last_response_ms: timing?.last_response_ms ?? null,
        average_response_ms: timing?.average_response_ms ?? null,
        response_samples: timing?.sample_count ?? 0,
        cache_hits: timing?.cache_hits ?? 0,
        offers_found: (previous?.offers_found ?? 0) + status.offers_found,
        routes_found: routeCounts.get(id) ?? 0,
      });
    }
    const nextVenueStats = Object.fromEntries(
      Object.entries(venueStats).map(([id, status]) => [id, { ...status, routes_found: 0 }]),
    ) as Record<string, VenueSearchStatus>;
    for (const [id, status] of observedVenueStats) {
      nextVenueStats[id] = status;
    }
    for (const [id, count] of routeCounts) {
      const previous = nextVenueStats[id];
      nextVenueStats[id] = previous ? { ...previous, routes_found: count } : { source: id, ok: true, cached: false, latency_ms: 0, last_response_ms: null, average_response_ms: null, response_samples: 0, cache_hits: 0, offers_found: 0, routes_found: count };
    }
    venueStats = nextVenueStats;
    const newlyFoundVenueIds = [
      ...nextRoutes.flatMap((route) => [
        ...route.legs.map((leg) => leg.provider),
        ...(route.route_provider ? [route.route_provider] : []),
      ]),
      ...(response.asset_statuses ?? [])
        .flatMap((status) => [...status.entry_sources, ...status.exit_sources])
        .filter((source) => !INTERNAL_DISCOVERY_SOURCES.has(source.source.toLowerCase()))
        .map((source) => source.source),
      ...(response.provider_statuses ?? []).map((source) => source.source),
    ]
      .map((source) => source.toLowerCase())
      .filter((source, index, sources) => source && sources.indexOf(source) === index);
    foundVenueIds = [...new Set([...foundVenueIds, ...newlyFoundVenueIds])];
    foundVenues = foundVenueOptions();
    routesFound = Math.max(routesFound, response.routes_found ?? nextRoutes.length);
    renderRoutesProgressively(nextRoutes);
    const bestRoute = nextRoutes.find((route) => route.is_current_best) ?? nextRoutes[0] ?? null;
    const targetQuoteRoute = amountSide === "target" && targetQuoteRouteKey
      ? nextRoutes.find((route) => routeFamilyKey(route) === targetQuoteRouteKey) ?? null
      : null;
    if (targetQuoteRoute) {
      selected = targetQuoteRoute;
    } else if (selectionPinnedByUser) {
      const pinnedRoute = nextRoutes.find((route) => route.route_id === selected?.route_id);
      if (pinnedRoute) selected = pinnedRoute;
      else { selectionPinnedByUser = false; selected = bestRoute; }
    } else selected = bestRoute;

  }

  function selectRoute(route: RouteCandidate) {
    selected = route;
    selectionPinnedByUser = true;
    if (amountSide === "target") {
      const desiredTarget = amountNumber(targetAmount);
      const quotedSource = Number(route.source_amount);
      const quotedTarget = Number(route.target_amount);
      if (desiredTarget > 0 && quotedSource > 0 && quotedTarget > 0) {
        amount = formatEditableAmount(desiredTarget * quotedSource / quotedTarget, selectedSourceCurrency);
        targetQuoteRouteKey = routeFamilyKey(route);
        targetRefinementAttempts = 0;
        resetResults();
      }
    }
  }

  function replaceServiceStats(service: ServiceStats) {
    routes = routes.map((route) => {
      if (!route.services?.some((item) => item.id === service.id)) return route;
      const services = route.services.map((item) => item.id === service.id ? service : item);
      const count = services.length || 1;
      return {
        ...route,
        services,
        reputation: {
          executions_average: Math.round(services.reduce((sum, item) => sum + item.executions_total, 0) / count),
          likes_average: Math.round(services.reduce((sum, item) => sum + item.likes_total, 0) / count),
          dislikes_average: Math.round(services.reduce((sum, item) => sum + item.dislikes_total, 0) / count),
        },
      };
    });
    selected = routes.find((route) => route.route_id === selected?.route_id) ?? selected;
    instructionsRoute = routes.find((route) => route.route_id === instructionsRoute?.route_id) ?? instructionsRoute;
  }

  function replaceRouteFeedback(routeId: string, feedback: RouteCandidate["feedback"]) {
    routes = routes.map((route) => route.route_id === routeId ? { ...route, feedback } : route);
    selected = routes.find((route) => route.route_id === selected?.route_id) ?? selected;
    instructionsRoute = routes.find((route) => route.route_id === instructionsRoute?.route_id) ?? instructionsRoute;
  }

  $: corridor = corridors.find((item) => item.id === corridorId) ?? corridors[0];
  $: sourceCountry = corridor ? (directionReversed ? corridor.target_country : corridor.source_country) : "";
  $: sourceCurrency = corridor ? (directionReversed ? corridor.target_currency : corridor.source_currency) : "";
  $: targetCountry = corridor ? (directionReversed ? corridor.source_country : corridor.target_country) : "";
  $: targetCurrency = corridor ? (directionReversed ? corridor.source_currency : corridor.target_currency) : "";
  $: INTERMEDIARY_ASSETS = [...new Set(paymentMethods.filter((method) => method.kind === "wallet").map((method) => method.currency))];
  $: CRYPTO_CURRENCIES = new Set(INTERMEDIARY_ASSETS);
  $: currencyCards = paymentMethods.filter((method) => method.kind === "currency");
  $: sourceMethods = paymentMethods.filter((method) => method.kind !== "currency" && (method.role === "sender" || method.role === "both"));
  $: targetMethods = paymentMethods.filter((method) => method.kind !== "currency" && (method.role === "recipient" || method.role === "both"));
  $: sourceMethod = resolveMethod(sourceMethods, sourceMethodId, sourceCountry, sourceCurrency);
  $: targetMethod = resolveMethod(targetMethods, targetMethodId, targetCountry, targetCurrency);
  $: sourceCurrencyChoice = sourceMethod?.currency || sourceCurrency;
  $: targetCurrencyChoice = targetMethod?.currency || targetCurrency;
  $: sourceCurrencyFlag = sourceMethod?.kind === "wallet" ? null : fiatFlagUrl(sourceCurrencyChoice);
  $: targetCurrencyFlag = targetMethod?.kind === "wallet" ? null : fiatFlagUrl(targetCurrencyChoice);
  $: selectedSourceCurrency = sourceMethod?.currency || sourceCurrency;
  $: selectedTargetCurrency = targetMethod?.currency || targetCurrency;
  $: sourceNetworks = sourceMethod?.kind === "wallet" ? networks.filter((network) => network.currencies.includes(sourceMethod!.currency)) : [];
  $: targetNetworks = targetMethod?.kind === "wallet" ? networks.filter((network) => network.currencies.includes(targetMethod!.currency)) : [];
  $: sourceNetwork = sourceNetworks.find((network) => network.id === sourceNetworkId) ?? sourceNetworks[0];
  $: targetNetwork = targetNetworks.find((network) => network.id === targetNetworkId) ?? targetNetworks[0];
  $: sourceCurrencyChoices = currencyChoicesFor(sourceMethod, "sender");
  $: targetCurrencyChoices = currencyChoicesFor(targetMethod, "recipient");
  $: showBelarusP2pWarning = selectedExchangeMethods.includes("p2p") && [sourceMethod, targetMethod].some((method) => method?.kind === "bank" && method.country === "BY" && method.currency === "BYN");
  $: onBelarusP2pWarningChange(showBelarusP2pWarning);
  $: hasAmount = amountSide === "target"
    ? Number.isFinite(amountNumber(targetAmount)) && amountNumber(targetAmount) > 0
    : Number.isFinite(amountNumber(amount)) && amountNumber(amount) > 0;
  $: previewRoute = selected ?? routes.find((route) => route.status === "complete" && route.is_current_best) ?? routes.find((route) => route.status === "complete") ?? null;
  $: displayedSourceAmount = amountSide === "target" && targetAmountNeedsRate && amountNumber(amount) <= 0 ? "" : amount;
  $: displayedTargetAmount = amountSide === "target" && !previewRoute ? targetAmount : amountFromRoute(previewRoute);
  $: onShareStateChange(urlReady && sourceMethod && targetMethod ? {
    source: selectedSourceCurrency, target: selectedTargetCurrency,
    amount: displayedSourceAmount, receive: displayedTargetAmount,
    from: sourceMethod.id, to: targetMethod.id,
    fromName: t(methodTitle(sourceMethod), {}, activeLocale), toName: t(methodTitle(targetMethod), {}, activeLocale),
    fromIcon: sourceMethod.iconUrl, toIcon: targetMethod.iconUrl,
    fromNetworkName: sourceMethod.kind === "wallet" ? sourceNetwork?.name : undefined,
    toNetworkName: targetMethod.kind === "wallet" ? targetNetwork?.name : undefined,
    fromNetwork: sourceNetwork?.id ?? "", toNetwork: targetNetwork?.id ?? "",
    sources: [...selectedSources], methods: [...selectedExchangeMethods], assets: [...selectedIntermediaryAssets],
  } : null);
  $: marketUsd = {
    source: sourceMethod?.kind === "wallet" ? calculateMarketUsd(displayedSourceAmount, marketPrices[selectedSourceCurrency]) : null,
    target: targetMethod?.kind === "wallet" ? calculateMarketUsd(displayedTargetAmount, marketPrices[selectedTargetCurrency]) : null,
  };
  $: converterAmountDigits = Math.min(14, Math.max(10, displayedSourceAmount.replace(/\D/g, "").length, displayedTargetAmount.replace(/\D/g, "").length));
  $: converterWidth = 550 + (converterAmountDigits - 10) * 21;
  $: secondsUntilRefresh = refreshSeconds && lastUpdatedAt ? Math.max(0, refreshSeconds - Math.floor((clock - lastUpdatedAt) / 1000)) : null;
  $: refreshProgress = secondsUntilRefresh !== null && refreshSeconds ? ((refreshSeconds - secondsUntilRefresh) / refreshSeconds) * 100 : 0;
  $: exchangeChoices = p2pSources.filter((source) => source.searchMode !== "always_on");
  $: alwaysOnProviders = p2pSources.filter((source) => source.searchMode === "always_on");
  $: searchingVenues = [...new Set([...selectedSources, ...alwaysOnProviders.map((provider) => provider.id)])].map((source) => p2pSources.find((item) => item.id === source) ?? { id: source, label: source, iconUrl: venueIcon(source), searchable: true, searchMode: "selectable" as const });
  $: exchangeMode = selectedExchangeMethods.length === EXCHANGE_METHODS.length ? "all" as const : selectedExchangeMethods[0];
  $: searchSignature = `${corridor?.id ?? ""}:${sourceMethod?.id ?? ""}:${sourceNetwork?.id ?? ""}:${targetMethod?.id ?? ""}:${targetNetwork?.id ?? ""}:${amountSide}:${amount}:${targetAmount}:${selectedSources.join(",")}:${selectedExchangeMethods.join(",")}:${selectedIntermediaryAssets.join(",")}:${directionReversed}`;
  $: scheduleAutomaticSearch(searchSignature, preferencesLoaded, urlReady, hasAmount, initialSearchReady);
  $: manageRefresh(guideRequested || walkthroughOpen ? 0 : refreshSeconds, lastUpdatedAt, hasAmount);
  $: if (preferencesLoaded) persistPreferences(amount, refreshSeconds, selectedSources, selectedExchangeMethods, selectedIntermediaryAssets, corridorId, sourceMethodId, targetMethodId, sourceNetwork?.id ?? sourceNetworkId, targetNetwork?.id ?? targetNetworkId, directionReversed);
  $: if (urlReady && corridor && selectedSourceCurrency && selectedTargetCurrency) updateHash(selectedSourceCurrency, selectedTargetCurrency, amount);

  function persistPreferences(value: string, refresh: RefreshSeconds, sources: P2pSource[], methods: ExchangeMethod[], assets: string[], corridorValue: string, sourceMethodValue: string, targetMethodValue: string, sourceNetworkValue: string, targetNetworkValue: string, reversed: boolean) {
    try {
      localStorage.setItem(STORAGE.amount, value); localStorage.setItem(STORAGE.refresh, String(refresh)); localStorage.setItem(STORAGE.sources, sources.join(",")); localStorage.setItem(STORAGE.methods, methods.join(",")); localStorage.setItem(STORAGE.assets, assets.join(",")); localStorage.setItem(STORAGE.corridor, corridorValue); localStorage.setItem(STORAGE.sourceMethod, sourceMethodValue); localStorage.setItem(STORAGE.targetMethod, targetMethodValue); localStorage.setItem(STORAGE.sourceNetwork, sourceNetworkValue); localStorage.setItem(STORAGE.targetNetwork, targetNetworkValue); localStorage.setItem(STORAGE.direction, String(reversed));
    } catch {}
  }
  async function refreshMarketPrices() {
    marketController?.abort();
    const controller = new AbortController();
    marketController = controller;
    try {
      const response = await fetchMarketPrices(controller.signal);
      if (controller.signal.aborted) return;
      if (Object.keys(response.prices).length) {
        marketPrices = response.prices;
        marketPricesUpdatedAt = response.updated_at ?? Math.floor(Date.now() / 1000);
      } else if (marketPricesUpdatedAt && Date.now() / 1000 - marketPricesUpdatedAt >= 6 * 60 * 60) {
        marketPrices = {};
      }
    } catch (error) {
      if (!controller.signal.aborted) {
        if (marketPricesUpdatedAt && Date.now() / 1000 - marketPricesUpdatedAt >= 6 * 60 * 60) marketPrices = {};
        console.warn("Market prices unavailable", error);
      }
    }
  }
  function calculateMarketUsd(value: string, price: number | undefined): number | null {
    const quantity = amountNumber(value);
    if (!Number.isFinite(quantity) || quantity <= 0 || !price || !Number.isFinite(price)) return null;
    const total = quantity * price;
    return Number.isFinite(total) ? total : null;
  }
  function formatMarketUsd(value: number) {
    if (value > 0 && value < 0.01) return "<$0.01";
    return `$${value.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
  }
  function updateHash(source: string, target: string, value: string) {
    if (guideRequested || parseExchangeHash(location.hash)?.guide) return;
    const params = new URLSearchParams(); if (value !== "0") params.set("amount", value);
    const query = params.toString();
    history.replaceState(null, "", `${location.pathname}${location.search}#/swap/${encodeURIComponent(source)}/${encodeURIComponent(target)}${query ? `?${query}` : ""}`);
  }
  function scheduleAutomaticSearch(_signature: string, loaded: boolean, ready: boolean, validAmount: boolean, initialReady: boolean) {
    if (debounceTimer) window.clearTimeout(debounceTimer);
    debounceTimer = undefined;
    if ((!guideRequested || (!instructionsRoute && !guideUnavailable)) && loaded && ready && initialReady && corridor && sourceMethod && targetMethod && validAmount) debounceTimer = window.setTimeout(startSearch, 650);
  }
  function manageRefresh(seconds: RefreshSeconds, updatedAt: number | null, validAmount: boolean) {
    if (refreshTimer) window.clearInterval(refreshTimer);
    if (!guideRequested && seconds && updatedAt && validAmount) refreshTimer = window.setInterval(() => { if (!searching) void startSearch(false); }, seconds * 1000);
  }
  function resetResults() {
    controller?.abort(); cancelRouteRendering(); revealedRouteCount = 0; displayRoutes([]); routesFound = 0; selected = null; selectionPinnedByUser = false; instructionsRoute = null; lastUpdatedAt = null; searching = false; awaitingFirstRoute = false; foundVenueIds = []; foundVenues = []; venueStats = {}; error = null;
    if (refreshTimer) window.clearInterval(refreshTimer);
  }
  function updateAmount(value: string) {
    initialSearchReady = true;
    amountSide = "source";
    targetAmount = "0";
    targetAmountNeedsRate = false;
    targetProbeAmount = null;
    targetRefinementAttempts = 0;
    targetQuoteRouteKey = null;
    amount = normalizeAmount(value);
    resetResults();
  }
  function updateTargetAmount(value: string) {
    initialSearchReady = true;
    const nextTargetAmount = normalizeAmount(value);
    const desiredTarget = amountNumber(nextTargetAmount);
    const quotedTarget = previewRoute?.target_amount ? Number(previewRoute.target_amount) : NaN;
    const quotedSource = amountNumber(amount);

    targetAmount = nextTargetAmount;
    amountSide = "target";
    targetRefinementAttempts = 0;
    targetQuoteRouteKey = previewRoute ? routeFamilyKey(previewRoute) : null;
    if (Number.isFinite(desiredTarget) && desiredTarget > 0 && Number.isFinite(quotedTarget) && quotedTarget > 0 && Number.isFinite(quotedSource) && quotedSource > 0) {
      amount = formatEditableAmount(desiredTarget * quotedSource / quotedTarget, selectedSourceCurrency);
      targetAmountNeedsRate = false;
      targetProbeAmount = null;
    } else {
      // The probe is sent to the live API without presenting it as the source
      // amount. The actual source value is shown only after a quote arrives.
      const currentSource = amountNumber(amount);
      targetProbeAmount = currentSource > 0
        ? currentSource
        : CRYPTO_CURRENCIES.has(selectedSourceCurrency.toUpperCase()) ? 1 : desiredTarget;
      targetAmountNeedsRate = Number.isFinite(desiredTarget) && desiredTarget > 0;
    }
    resetResults();
  }
  function swapDirection() {
    if (!corridor) return;
    initialSearchReady = true;
    const nextSource = targetMethod?.id ?? "", nextTarget = sourceMethod?.id ?? "";
    const nextAmount = amountSide === "target" ? targetAmount : amountFromRoute(previewRoute);
    if (amountNumber(nextAmount) > 0) amount = nextAmount;
    amountSide = "source"; targetAmount = "0"; targetAmountNeedsRate = false; targetProbeAmount = null; targetRefinementAttempts = 0; targetQuoteRouteKey = null;
    directionReversed = !directionReversed; sourceMethodId = nextSource; targetMethodId = nextTarget;
    sourceNetworkId = targetNetwork?.id ?? FALLBACK_NETWORK.id; targetNetworkId = sourceNetwork?.id ?? FALLBACK_NETWORK.id; resetResults();
  }
  function sameCurrencyGroup(left: PaymentMethod, right: PaymentMethod) {
    return (left.currencyGroup ?? left.id) === (right.currencyGroup ?? right.id);
  }
  function resolveMethod(methods: PaymentMethod[], selectedId: string, country: string, currency: string) {
    const selected = methods.find((method) => method.id === selectedId);
    if (selected) return selected;
    const matchesCorridor = (method: PaymentMethod) => method.kind === "wallet"
      ? method.currency === currency
      : method.country === country && method.currency === currency;
    // Cash is an explicit choice; default corridors to a bank transfer when one exists.
    const preferred = [...methods.filter((method) => method.kind !== "cash"), ...methods.filter((method) => method.kind === "cash")];
    return preferred.find(matchesCorridor)
      ?? (country ? preferred.find((method) => method.country === country) : undefined)
      ?? methods[0]
      ?? null;
  }
  function currencyChoicesFor(method: PaymentMethod | null, role: "sender" | "recipient") {
    if (!method || method.kind === "wallet") return [];
    const supported = new Set(
      paymentMethods
        .filter((candidate) => candidate.kind === "bank" || candidate.kind === "cash")
        .filter((candidate) => sameCurrencyGroup(candidate, method) && (candidate.role === role || candidate.role === "both"))
        .map((candidate) => candidate.currency),
    );
    return currencyCards
      .filter((choice) => supported.has(choice.currency))
      .map((choice) => ({ id: choice.currency, name: choice.name, mark: choice.initials, color: choice.color }))
      .sort((left, right) => Number(right.id === method.currency) - Number(left.id === method.currency));
  }
  function chooseCurrency(side: Exclude<PickerSide, null>, choice: string) {
    const role = side === "source" ? "sender" : "recipient";
    const current = side === "source" ? sourceMethod : targetMethod;
    const next = current && current.kind !== "wallet"
      ? paymentMethods.find((method) => (method.kind === "bank" || method.kind === "cash") && sameCurrencyGroup(method, current) && method.currency === choice && (method.role === role || method.role === "both"))
      : undefined;
    if (!next) return;
    initialSearchReady = true;
    if (side === "source") {
      sourceMethodId = next.id;
    } else {
      targetMethodId = next.id;
    }
    currencyPicker = null;
    resetResults();
  }
  function chooseSource(method: PaymentMethod, network?: CryptoNetwork) { initialSearchReady = true; sourceMethodId = method.id; if (method.kind === "wallet") sourceNetworkId = network?.id ?? networks.find((item) => item.currencies.includes(method.currency))?.id ?? FALLBACK_NETWORK.id; methodPicker = null; resetResults(); }
  function chooseTarget(method: PaymentMethod, network?: CryptoNetwork) { initialSearchReady = true; targetMethodId = method.id; if (method.kind === "wallet") targetNetworkId = network?.id ?? networks.find((item) => item.currencies.includes(method.currency))?.id ?? FALLBACK_NETWORK.id; methodPicker = null; resetResults(); }
  function selectNetwork(network: CryptoNetwork) { initialSearchReady = true; if (networkPicker === "source") sourceNetworkId = network.id; else targetNetworkId = network.id; networkPicker = null; resetResults(); }
  function openCurrencyPicker(side: Exclude<PickerSide, null>) { currencyPicker = side; }
  async function openMethodPicker(side: Exclude<PickerSide, null>) {
    if (!browser) return;
    methodPicker = side;
    paymentPickerComponent ??= (await import("./PaymentMethodPicker.svelte")).default;
  }
  function openNetworkPicker(side: Exclude<PickerSide, null>) {
    networkPicker = side;
  }
  function closeGuide() {
    guideRequested = false; pendingGuide = null; guideUnavailable = false; instructionsRoute = null;
    history.replaceState(history.state, "", `${location.pathname}${location.search}`);
    updateHash(selectedSourceCurrency, selectedTargetCurrency, amount);
    lastLocationHash = location.hash;
    window.scrollTo({ top: 0 });
  }
  function restoreShared(shared: NonNullable<ReturnType<typeof readSharedExchange>>) {
    const from = sharedMethod(paymentMethods, shared.sourceCurrency, "sender", shared.params.get("from") ?? "");
    const to = sharedMethod(paymentMethods, shared.targetCurrency, "recipient", shared.params.get("to") ?? "");
    guideUnavailable = !shared.amount || !from || !to || Boolean(shared.params.get("from") && shared.params.get("from") !== from?.id) || Boolean(shared.params.get("to") && shared.params.get("to") !== to?.id);
    for (const [method, key] of [[from, "fromNetwork"], [to, "toNetwork"]] as const) {
      const networkId = shared.params.get(key);
      if (method?.kind === "wallet" && networkId && !networks.some(network => network.id === networkId && network.currencies.includes(method.currency))) guideUnavailable = true;
    }
    amountSide = "source"; targetAmount = "0"; targetAmountNeedsRate = false; targetProbeAmount = null; targetRefinementAttempts = 0; targetQuoteRouteKey = null;
    amount = shared.amount ?? "0";
    sourceMethodId = sharedMethod(paymentMethods, shared.sourceCurrency, "sender", shared.params.get("from") ?? "")?.id ?? sourceMethodId;
    targetMethodId = sharedMethod(paymentMethods, shared.targetCurrency, "recipient", shared.params.get("to") ?? "")?.id ?? targetMethodId;
    // A plain swap hash omits networks; retain the saved selections on reload.
    sourceNetworkId = shared.params.get("fromNetwork") ?? sourceNetworkId;
    targetNetworkId = shared.params.get("toNetwork") ?? targetNetworkId;
    const sources = sharedIdentifiers(shared.params, "sources");
    if (sources.length) selectedSources = sources;
    const methods = sharedIdentifiers(shared.params, "methods").filter((item): item is ExchangeMethod => EXCHANGE_METHODS.includes(item as ExchangeMethod));
    if (methods.length) selectedExchangeMethods = methods;
    selectedIntermediaryAssets = sharedIdentifiers(shared.params, "assets");
  }
  function locationChanged(eventOrForce: Event | boolean = false) {
    if (/^#\/otc(?:[/?]|$)/.test(location.hash)) return;
    if (eventOrForce !== true && lastLocationHash === location.hash) return;
    lastLocationHash = location.hash;
    const shared = readSharedExchange();
    if (!shared?.guide) { closeGuide(); return; }
    controller?.abort(); instructionsRoute = null; guideRequested = true; guideUnavailable = false; pendingGuide = shared;
    finishIntro(); restoreShared(shared); initialSearchReady = true;
    if (!guideUnavailable) void tick().then(() => startSearch(false));
  }
  async function openInstructions(route: RouteCandidate, replace = false) {
    if (!browser) return;
    const params = new URLSearchParams({ amount: route.source_amount ?? String(route.source_amount_minor / 100), from: sourceMethodId, to: targetMethodId, fromNetwork: sourceNetwork?.id ?? sourceNetworkId, toNetwork: targetNetwork?.id ?? targetNetworkId, sources: selectedSources.join(","), methods: selectedExchangeMethods.join(","), assets: selectedIntermediaryAssets.join(","), path: guideRoutePath(route), venues: [...new Set([route.route_provider, ...route.legs.map(leg => leg.provider), ...(route.cycle_legs ?? []).map(leg => leg.provider)].filter(Boolean))].join(","), lang: activeLocale });
    const hash = guideHash(route.source_currency, route.target_currency ?? route.entry_asset, params);
    if (replace) history.replaceState(history.state, "", hash); else history.pushState(history.state, "", hash);
    lastLocationHash = location.hash;
    guideRequested = true; pendingGuide = null; guideUnavailable = false; instructionsRoute = route;
    finishIntro();
    routeInstructionsComponent ??= (await import("./RouteInstructions.svelte")).default;
    window.scrollTo({ top: 0 });
    void recordInstructionOpen(anonymousId, (route.service_links ?? []).map((link) => link.tracking_token)).catch(() => {});
  }

  function runPrimaryAction() {
    if (previewRoute) {
      void openInstructions(previewRoute);
      return;
    }
    if (!searching) void startSearch();
  }

  async function openService(link: ServiceLink) {
    const popup = window.open("about:blank", "_blank");
    if (popup) popup.opener = null;
    try {
      const execution = await recordServiceOpen(anonymousId, link.tracking_token);
      replaceServiceStats(execution.service);
      if (popup) popup.location.replace(execution.redirect_url); else window.open(execution.redirect_url, "_blank", "noopener,noreferrer");
    } catch (cause) {
      popup?.close();
      error = cause instanceof Error ? cause.message : "Could not open the service";
    }
  }

  async function voteForRoute(route: RouteCandidate, vote: ServiceVote) {
    try {
      replaceRouteFeedback(route.route_id, await setRouteVote(route.route_id, anonymousId, vote));
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Could not save your route feedback";
    }
  }

  async function startSearch(countActivity = true) {
    if (debounceTimer) window.clearTimeout(debounceTimer);
    debounceTimer = undefined;
    if (!corridor || !sourceMethod || !targetMethod) return;
    const desiredTarget = amountNumber(targetAmount);
    const targetDriven = amountSide === "target" && Number.isFinite(desiredTarget) && desiredTarget > 0;
    const value = targetDriven && targetAmountNeedsRate && targetProbeAmount != null ? targetProbeAmount : amountNumber(amount);
    if (!Number.isFinite(value) || value <= 0) return resetResults();
    const sourceWallet = sourceMethod.kind === "wallet", targetWallet = targetMethod.kind === "wallet";
    if ((sourceWallet && !sourceNetwork) || (targetWallet && !targetNetwork)) {
      resetResults();
      error = "No compatible network is available for the selected cryptocurrency";
      return;
    }
    controller?.abort(); cancelRouteRendering(); revealedRouteCount = 0; controller = new AbortController(); const signal = controller.signal; const currentRequest = ++requestId; searching = true; awaitingFirstRoute = true; routesFound = 0; foundVenueIds = []; foundVenues = []; venueStats = {}; responseMetrics = new SearchResponseMetrics(); error = null;
    let rerunForTargetAmount = false;
    try {
      const liveQuery = { sourceFiat: selectedSourceCurrency, targetFiat: selectedTargetCurrency, sourceAmount: value, intermediaryAssets: !sourceWallet && !targetWallet && selectedIntermediaryAssets.length ? selectedIntermediaryAssets : undefined, sourceNetwork: sourceWallet ? sourceNetwork?.id : undefined, targetNetwork: targetWallet ? targetNetwork?.id : undefined, sourcePaymentMethod: sourceWallet || sourceMethod.kind === "currency" ? undefined : sourceMethod.p2pQuery, targetPaymentMethod: targetWallet || targetMethod.kind === "currency" ? undefined : targetMethod.p2pQuery, sourcePaymentFeePercent: sourceWallet ? 0 : sourceMethod.bankFeePercent, targetPaymentFeePercent: targetWallet ? 0 : targetMethod.bankFeePercent, sources: selectedSources, exchangeMode, allowCrossVenue: true, limit: 40, countActivity };
      let response: P2pRouteSearchResponse;
      try {
        if (!anonymousId) throw new Error("Anonymous ID unavailable");
        response = await streamP2pRoutes(liveQuery, anonymousId, signal, (event) => {
          if (currentRequest !== requestId) return;
          if (event.type === "search_started") routesFound = Math.max(routesFound, event.routes_found);
          if (event.type === "routes_updated" && !(amountSide === "target" && targetAmountNeedsRate)) {
            applySearchResponse(event);
            if (event.routes.length > 0) awaitingFirstRoute = false;
          }
        });
      } catch (streamError) {
        if (signal.aborted || currentRequest !== requestId) return;
        response = await fetchP2pRoutes({ ...liveQuery, signal, anonymousId });
      }
      if (currentRequest !== requestId) return;
      if (targetDriven) {
        const responseRoutes = mapRoutes(response);
        const quotedRoute = responseRoutes.find((route) => routeFamilyKey(route) === targetQuoteRouteKey) ?? responseRoutes[0];
        const quotedSource = Number(quotedRoute?.source_amount);
        const quotedTarget = Number(quotedRoute?.target_amount);
        if (desiredTarget > 0 && quotedSource > 0 && quotedTarget > 0) {
          targetQuoteRouteKey = routeFamilyKey(quotedRoute);
          const calculatedSource = formatEditableAmount(desiredTarget * quotedSource / quotedTarget, selectedSourceCurrency);
          const targetPrecision = CRYPTO_CURRENCIES.has(selectedTargetCurrency.toUpperCase()) ? 8 : 2;
          const closeEnough = Math.abs(quotedTarget - desiredTarget) < 0.5 * 10 ** -targetPrecision;
          targetAmountNeedsRate = false;
          targetProbeAmount = null;
          if (!closeEnough && calculatedSource !== amount && targetRefinementAttempts < 6) {
            amount = calculatedSource;
            targetRefinementAttempts += 1;
            rerunForTargetAmount = true;
            resetResults();
          }
        }
      }
      if (!rerunForTargetAmount) {
        applySearchResponse(response); awaitingFirstRoute = false; lastUpdatedAt = Date.now(); clock = Date.now();
        if (guideRequested && pendingGuide) {
          const path = pendingGuide.params.get("path");
          const fresh = mapRoutes(response).find(item => item.status === "complete" && item.source_currency === pendingGuide!.sourceCurrency && item.target_currency === pendingGuide!.targetCurrency && (!path || guideRoutePath(item) === path));
          if (fresh) await openInstructions(fresh, true); else guideUnavailable = true;
        }
      }
    } catch (cause) {
      if (signal.aborted || currentRequest !== requestId) return;
      error = cause instanceof Error ? cause.message : "Could not search live P2P markets";
    } finally {
      if (currentRequest === requestId) { searching = false; awaitingFirstRoute = false; }
      if (rerunForTargetAmount) window.setTimeout(() => void startSearch(false), 0);
    }
  }
  function toggleSource(source: P2pSource) { initialSearchReady = true; selectedSources = selectedSources.includes(source) ? (selectedSources.length === 1 ? selectedSources : selectedSources.filter((item) => item !== source)) : [...selectedSources, source]; resetResults(); }
  function toggleExchangeMethod(method: ExchangeMethod) { initialSearchReady = true; selectedExchangeMethods = selectedExchangeMethods.includes(method) ? (selectedExchangeMethods.length === 1 ? selectedExchangeMethods : selectedExchangeMethods.filter((item) => item !== method)) : EXCHANGE_METHODS.filter((item) => item === method || selectedExchangeMethods.includes(item)); resetResults(); }
  function toggleAsset(asset: string) { initialSearchReady = true; selectedIntermediaryAssets = selectedIntermediaryAssets.includes(asset) ? selectedIntermediaryAssets.filter((item) => item !== asset) : [...selectedIntermediaryAssets, asset]; resetResults(); }
  function onDocumentMouseDown(event: MouseEvent) { if (modalOpen && settingsElement && !settingsElement.contains(event.target as Node)) closeSettings(); }
  function closeSettings() { settingsOpen = false; exchangesOpen = false; }
  function onSettingsKeyDown(event: KeyboardEvent) { if (event.key === "Escape") closeSettings(); }
  function portalSettingsBackdrop(node: HTMLDivElement) {
    const anchor = document.createComment("settings backdrop");
    node.before(anchor);
    const mobile = window.matchMedia("(max-width: 640px)");
    let unlockPage: (() => void) | undefined;

    const place = () => {
      if (mobile.matches) {
        document.body.appendChild(node);
        unlockPage ??= lockPageScroll();
      } else {
        anchor.after(node);
        unlockPage?.();
        unlockPage = undefined;
      }
    };

    place();
    mobile.addEventListener("change", place);
    return {
      destroy() {
        mobile.removeEventListener("change", place);
        unlockPage?.();
        node.remove();
        anchor.remove();
      }
    };
  }
  function startSettingsDrag(event: PointerEvent) {
    if (!window.matchMedia("(max-width: 640px)").matches) return;
    settingsDragging = true;
    settingsDragStartY = event.clientY;
    settingsDragDistance = 0;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }
  function moveSettingsDrag(event: PointerEvent) {
    if (!settingsDragging) return;
    settingsDragDistance = Math.max(0, event.clientY - settingsDragStartY);
    settingsDialog?.style.setProperty("--settings-sheet-drag", `${settingsDragDistance}px`);
  }
  function endSettingsDrag() {
    if (!settingsDragging) return;
    const shouldClose = settingsDragDistance > 72 || (settingsDialog && settingsDragDistance > settingsDialog.clientHeight * 0.18);
    settingsDragging = false;
    if (shouldClose) {
      closeSettings();
    } else {
      settingsDialog?.style.removeProperty("--settings-sheet-drag");
    }
  }
  function hideBrokenImage(event: Event) {
    const image = event.currentTarget as HTMLImageElement;
    image.style.display = "none";
    const fallback = image.parentElement?.querySelector<HTMLElement>("[data-icon-fallback]");
    if (fallback) fallback.style.display = "inline";
  }
  function fallbackSourceIcon(event: Event, source: P2pSource) {
    const image = event.currentTarget as HTMLImageElement;
    image.onerror = null;
    image.src = venueIcon(source);
  }
  function fallbackAssetIcon(event: Event) {
    const image = event.currentTarget as HTMLImageElement;
    image.onerror = null;
    image.src = assetIcon("generic");
  }

  onMount(() => {
    void refreshMarketPrices();
    marketRefreshTimer = window.setInterval(() => void refreshMarketPrices(), 60 * 60 * 1000);
    const shared = readSharedExchange();
    const sharedLanguage = shared?.params.get("lang");
    if (shared && (sharedLanguage === "en" || sharedLanguage === "ru" || sharedLanguage === "hy")) setLocale(sharedLanguage);
    let methodsReady = false;
    let corridorsReady = false;
    let providersReady = false, networksReady = false;
    const markUrlReady = () => { if (methodsReady && corridorsReady && providersReady && networksReady) { if (shared) { restoreShared(shared); initialSearchReady = true; } urlReady = true; } };
    let savedSourceIds: string[] = [];
    let savedKnownSourceIds: string[] = [];
    try {
      anonymousId = getAnonymousUserId() ?? "";
      amount = shared?.amount ?? localStorage.getItem(STORAGE.amount) ?? "0";
      corridorId = localStorage.getItem(STORAGE.corridor) ?? ""; sourceMethodId = localStorage.getItem(STORAGE.sourceMethod) ?? sourceMethodId; targetMethodId = localStorage.getItem(STORAGE.targetMethod) ?? targetMethodId; sourceNetworkId = localStorage.getItem(STORAGE.sourceNetwork) ?? sourceNetworkId; targetNetworkId = localStorage.getItem(STORAGE.targetNetwork) ?? targetNetworkId;
      const savedDirection = localStorage.getItem(STORAGE.direction); if (savedDirection != null) directionReversed = savedDirection === "true";
      savedSourceIds = localStorage.getItem(STORAGE.sources)?.split(",").map((value) => value.trim()).filter(Boolean) ?? [];
      savedKnownSourceIds = localStorage.getItem(STORAGE.knownSources)?.split(",").map((value) => value.trim()).filter(Boolean) ?? [];
      const savedMethods = localStorage.getItem(STORAGE.methods)?.split(",").filter((method): method is ExchangeMethod => EXCHANGE_METHODS.includes(method as ExchangeMethod)) ?? [];
      if (savedMethods.length) selectedExchangeMethods = EXCHANGE_METHODS.filter((method) => savedMethods.includes(method));
      const savedAssets = localStorage.getItem(STORAGE.assets); if (savedAssets != null) selectedIntermediaryAssets = [...new Set(savedAssets.split(",").map((asset) => asset.trim().toUpperCase()).filter(Boolean))];
      const savedRefresh = Number(localStorage.getItem(STORAGE.refresh)); if (REFRESH_OPTIONS.includes(savedRefresh as RefreshSeconds)) refreshSeconds = savedRefresh as RefreshSeconds;
      const savedActivityPeriod = localStorage.getItem(STORAGE.activityPeriod); if (SEARCH_ACTIVITY_PERIODS.includes(savedActivityPeriod as SearchActivityPeriod)) activityPeriod = savedActivityPeriod as SearchActivityPeriod;
      routesExpanded = localStorage.getItem(STORAGE.routesVisible) !== "false";
      const savedActivityVisible = localStorage.getItem(STORAGE.activityVisible);
      activityExpanded = !window.matchMedia("(max-width: 980px)").matches && savedActivityVisible === "true";
      if (activityExpanded && !routesExpanded) bringActivityIntoView("auto");
    } catch {}
    void registerAnonymousUser(anonymousId).catch(() => {});
    preferencesLoaded = true;
    // Keep a saved route search off the initial critical path. User changes
    // still enable the normal debounced search immediately.
    initialSearchTimer = window.setTimeout(() => initialSearchReady = true, 1500);
    fetchNetworks().then((items) => { if (items.length) networks = items; }).catch(() => {}).finally(() => { networksReady = true; markUrlReady(); });
    fetchPaymentMethods().then((items) => {
      paymentMethods = items;
      onPaymentMethodsLoaded(items);
      if (shared) {
        sourceMethodId = sharedMethod(items, shared.sourceCurrency, "sender", shared.params.get("from") ?? sourceMethodId)?.id ?? sourceMethodId;
        targetMethodId = sharedMethod(items, shared.targetCurrency, "recipient", shared.params.get("to") ?? targetMethodId)?.id ?? targetMethodId;
      }
    }).catch((cause: Error) => error ??= cause.message).finally(() => { methodsReady = true; markUrlReady(); });
    fetchProviders().then((providers) => {
      p2pSources = providerSources(providers);
      onProvidersLoaded(providers);
      foundVenues = foundVenueOptions();
      venueNames = Object.fromEntries(p2pSources.map((provider) => [provider.id.toLowerCase(), provider.label]));
      venueUrls = Object.fromEntries(providers.map((provider) => [provider.slug.toLowerCase(), provider.source_url]));
      providerGuidance = Object.fromEntries(
        providers
          .filter((provider) => provider.guidance)
          .map((provider) => [provider.slug.toLowerCase(), provider.guidance as ProviderGuidance]),
      );
      const catalog = new Set(p2pSources.map((source) => source.id));
      const live = p2pSources.filter((source) => source.searchMode === "selectable").map((source) => source.id);
      const restored = [...new Set(savedSourceIds.filter((source) => catalog.has(source) && p2pSources.find((item) => item.id === source)?.searchMode === "selectable"))];
      const known = new Set(savedKnownSourceIds.length ? savedKnownSourceIds : savedSourceIds);
      const newlyAdded = live.filter((source) => !known.has(source));
      selectedSources = savedSourceIds.length ? [...new Set([...restored, ...newlyAdded])] : live;
      try { localStorage.setItem(STORAGE.knownSources, live.join(",")); } catch {}
    }).catch((cause: Error) => error ??= cause.message).finally(() => { providersReady = true; markUrlReady(); });
    fetchCorridors().then((response) => {
      const savedDirection = localStorage.getItem(STORAGE.direction);
      const sharedCorridor = shared ? response.items.find((item) => (item.source_currency === shared.sourceCurrency && item.target_currency === shared.targetCurrency) || (item.source_currency === shared.targetCurrency && item.target_currency === shared.sourceCurrency)) : null;
      corridors = response.items; corridorId = sharedCorridor?.id || corridorId || response.items[0]?.id || "";
      if (shared && sharedCorridor?.source_currency === shared.targetCurrency && sharedCorridor.target_currency === shared.sourceCurrency) directionReversed = true; else if (shared) directionReversed = false; else if (savedDirection != null) directionReversed = savedDirection === "true";
    }).catch((cause: Error) => error = cause.message).finally(() => { corridorsReady = true; markUrlReady(); });
    window.addEventListener("hashchange", locationChanged);
    window.addEventListener("popstate", locationChanged);
    document.addEventListener("mousedown", onDocumentMouseDown);
    clockTimer = window.setInterval(() => clock = Date.now(), 1000);
  });
  afterUpdate(() => {
    if (modalOpen === settingsWasOpen) return;
    settingsWasOpen = modalOpen;
    if (modalOpen) {
      window.addEventListener("keydown", onSettingsKeyDown);
    } else {
      window.removeEventListener("keydown", onSettingsKeyDown);
    }
  });
  onDestroy(() => {
    controller?.abort();
    activityController?.abort();
    marketController?.abort();
    cancelRouteRendering();
    if (debounceTimer) clearTimeout(debounceTimer);
    if (refreshTimer) clearInterval(refreshTimer);
    if (clockTimer) clearInterval(clockTimer);
    if (marketRefreshTimer) clearInterval(marketRefreshTimer);
    if (initialSearchTimer) clearTimeout(initialSearchTimer);
    if (typeof document !== "undefined") document.removeEventListener("mousedown", onDocumentMouseDown);
    if (typeof window !== "undefined") { window.removeEventListener("keydown", onSettingsKeyDown); window.removeEventListener("hashchange", locationChanged); window.removeEventListener("popstate", locationChanged); }
  });
</script>

<svelte:window on:resize={closeInlineActivityOnMobile} />

<section hidden={guideRequested} class="shell" class:localeLong={activeLocale !== "en"} class:introReady={introCompleted} id="transfer">
  {#if introPlaying}
    <div class="introOverlay" class:introStarted bind:this={introOverlayElement} aria-hidden="true">
      <p class="introTitle" on:animationend={(event) => { if (event.animationName.endsWith("introDock")) finishIntro(); }}><span class="introClip"><span class="introWord introWordMove">{firstHeadline.first}</span></span>{' '}<span class="introSecondWithDot"><span class="introClip"><span class="introWord introWordMoney">{firstHeadline.second}</span></span><span class="introPunctuation introFirstPunctuation">{firstHeadline.punctuation}</span></span>{' '}<span class="introEmphasis"><span class="introClip"><span class="introWord introWordKeep">{secondHeadline.first}</span></span>{' '}<span class="introSecondWithDot"><span class="introClip"><span class="introWord introWordMore">{secondHeadline.second}</span></span><span class="introPunctuation introLastPunctuation">{secondHeadline.punctuation}</span></span><img class="introMarker" src="/icons/ui/marker-down-right.svg" alt="" width="512" height="512" aria-hidden="true" /></span></p>
    </div>
  {/if}
  <div class="hero"><h1 bind:this={heroHeadingElement}>{t("Move money.", {}, activeLocale)} <span>{t("Keep more.", {}, activeLocale)}</span></h1><p>{home.intro}</p></div>
  <noscript><p class="nojsNotice">{home.nojs}</p></noscript>
  <div class="workspace" class:activityExpanded class:routesCollapsed={!routesExpanded} style:--converter-width={`${converterWidth}px`} style:--amount-digits={converterAmountDigits}>
    <div class="converterStack">
    <div class="card" class:modalOpen bind:this={converterCard}>
      <div class="cardTop">
        <div class="modeTabs" aria-label={t("Exchange mode", {}, activeLocale)}><button type="button" class="modeActive">{t("Bridge", {}, activeLocale)}</button><button type="button" disabled>{t("History", {}, activeLocale)}</button></div>
        <div class="cardActions" bind:this={settingsElement}>
          <button type="button" class="helpButton" disabled={!paymentMethods.some(method => method.kind === "bank" && (method.role === "sender" || method.role === "both")) || !paymentMethods.some(method => method.kind === "bank" && (method.role === "recipient" || method.role === "both"))} on:click={() => void openWalkthrough()} aria-label={t("How to find a route", {}, activeLocale)} title={t("How to find a route", {}, activeLocale)} data-testid="start-converter-walkthrough">?</button>
          <button type="button" class="refreshButton" on:click={() => void startSearch()} disabled={!hasAmount || searching} aria-label={t("Refresh routes now", {}, activeLocale)}><img class:refreshSpin={awaitingFirstRoute} src="/icons/ui/route-refresh.png" alt="" width="18" height="18" aria-hidden="true" /></button>
          <div class="settingsWrap">
            <button type="button" class="exchangesButton" on:click={() => { settingsOpen = false; exchangesOpen = !exchangesOpen; }} aria-haspopup="dialog" aria-expanded={exchangesOpen} aria-label={t("Choose exchanges", {}, activeLocale)}><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACn0lEQVR4AbSVy0sVURzHZ9q0KcigiIyKrgUZRBS0SSgXLUKMIjAkEIKICHrgQqFF6tJFFEgXiqJNUemi6AGCyBVU0JWuBFHxgS8QUfAPuH6+xznHGe8493pF+X3O7/x+5/c4c+aecZ+3x38FN8hms/vhPnyCL3CrkL3lbUChCnhDsRH4ASm4B1/xX0QnSmwDEs9APfSSLe6gf0KZ7/uV6NtwGCogUSINKHgFtEvt9jWZk1Dt+34KXsEEtofukYajkCiuAYW1q06iD8ILUNE6iv1jXrS4BlR4AhkKVsEHWMbetZgG7L6MSvpVdDH/A5OQKMRLmkJBylGufgRaM5gGzE6A5CNDNQxASx46WP8MNk45yh2n6Un8RmwDYwTDcY6nFprzUMP6I7BxteTXgCStQdgGx2RAmoQFdFFCrp6qnuQqnuI02tvaYExOQUAjTEMmBvkbFRfDUOCLNAh8xSk2kII0rFDhF0huarBP8E0GWK3L1Mojn4LKGORvJd6jqG70MHP9zFFGVhmfsVZqGlBgCXycR3A+Ru9E6gg+AO+oUSKY3wVd2LOmAYZ2Uo7+Dy9pciPEOXxJYj4fBFyGHHENWOkHvZjz6EyIUZrJxpUr7PgvXv3yrhO3IrD1HtbQY+EG13BMBeiLadFF0hNdYm070ZG0hRYPMW+j+Vy4AT6vgaGBhR4Ltv1yKgkzV4gdhOdQwuoDkOjJ3D2Q4z3DU4J0WZg6mWVmGqILkatBkE4j0iDwbyrOsxlLl89dfextJYhvIuAtG11ERxp8x6GzJm5DsBWM8trx6F9nOTruZss3Q6Di5ymuzwWmF2nQjkcvSy/VYo9rhKQ+1pOkm8UW4krRTtxLZmEVfoP9Oup4dFs78F1QBlqN4m62fA9ZV45CHesAAAD//3Y7g4QAAAAGSURBVAMAao5eF665v54AAAAASUVORK5CYII=" alt="" width="18" height="18" aria-hidden="true" /></button>
            {#if exchangesOpen}
              <div class="settingsBackdrop" use:portalSettingsBackdrop on:mousedown={closeSettings} role="presentation">
                <div class:settingsDragging class="settingsMenu exchangesMenu" bind:this={settingsDialog} role="dialog" aria-label="Exchange settings" tabindex="-1" on:mousedown|stopPropagation>
                  <div class="settingsModalHeader"><span class="settingsSheetHandle" aria-hidden="true" on:pointerdown={startSettingsDrag} on:pointermove={moveSettingsDrag} on:pointerup={endSettingsDrag} on:pointercancel={endSettingsDrag}></span><button type="button" class="settingsClose" on:click={closeSettings} aria-label="Close exchange settings"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg></button></div>
                  <div class="settingsHead"><div><strong>Search exchanges</strong></div></div>
                  <div class="sourceOptions exchangeOptions exchangeModalOptions" aria-label="Exchanges to search">{#each exchangeChoices as source}{@const enabled = selectedSources.includes(source.id)}<button type="button" class:sourceOptionActive={enabled} class="sourceOption" aria-pressed={enabled} title={sourceTitle(source)} on:click={() => toggleSource(source.id)}><span class="sourceOptionIcon" aria-hidden="true"><img src={source.iconUrl} alt="" width="18" height="18" loading="lazy" decoding="async" on:error={(event) => fallbackSourceIcon(event, source.id)} /></span>{source.label}</button>{/each}</div>
                  {#if alwaysOnProviders.length}
                    <div class="alwaysOnProviders" aria-label="Direct quote providers">
                      <div class="alwaysOnHeading"><strong>Direct quote providers</strong><span>Compared automatically</span></div>
                      <div class="sourceOptions exchangeOptions">{#each alwaysOnProviders as provider}<div class="sourceOption alwaysOnProvider" title={sourceTitle(provider)}><span class="sourceOptionIcon" aria-hidden="true"><img src={provider.iconUrl} alt="" width="18" height="18" loading="lazy" decoding="async" on:error={(event) => fallbackSourceIcon(event, provider.id)} /></span><span>{provider.label}</span><small>Always on</small></div>{/each}</div>
                    </div>
                  {/if}
                </div>
              </div>
            {/if}
          </div>
          <div class="settingsWrap">
            <button type="button" class="settingsButton" on:click={() => { exchangesOpen = false; settingsOpen = !settingsOpen; }} aria-haspopup="dialog" aria-expanded={settingsOpen} aria-label={t("Route refresh settings", {}, activeLocale)}><img src="/icons/ui/route-settings.png" alt="" width="18" height="18" aria-hidden="true" /></button>
            {#if settingsOpen}
              <div class="settingsBackdrop" use:portalSettingsBackdrop on:mousedown={closeSettings} role="presentation">
                <div class:settingsDragging class="settingsMenu" bind:this={settingsDialog} role="dialog" aria-label="Refresh settings" tabindex="-1" on:mousedown|stopPropagation>
                <div class="settingsModalHeader"><span class="settingsSheetHandle" aria-hidden="true" on:pointerdown={startSettingsDrag} on:pointermove={moveSettingsDrag} on:pointerup={endSettingsDrag} on:pointercancel={endSettingsDrag}></span><button type="button" class="settingsClose" on:click={closeSettings} aria-label="Close route settings"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg></button></div>
                <div class="settingsHead"><div><strong>Auto-refresh</strong></div></div>
                <div class="refreshOptions">{#each REFRESH_OPTIONS as seconds}<button type="button" aria-pressed={refreshSeconds === seconds} on:click={() => { refreshSeconds = seconds; settingsOpen = false; }}>{refreshOptionLabel(seconds)}</button>{/each}</div>
                <div class="sourceSettings exchangeMethodSettings"><div class="intermediarySettingsHead"><strong class="sourceSettingsLabel">{t("Exchange methods", {}, activeLocale)}</strong></div><div class="sourceOptions exchangeMethodOptions" aria-label={t("Exchange methods", {}, activeLocale)}>
                  {#each EXCHANGE_METHODS as method}{@const enabled = selectedExchangeMethods.includes(method)}<button type="button" class:sourceOptionActive={enabled} class="sourceOption" aria-pressed={enabled} on:click={() => toggleExchangeMethod(method)}>{method === "p2p" ? "P2P" : t("Exchangers", {}, activeLocale)}</button>{/each}
                </div></div>
                <div class="sourceSettings"><div class="intermediarySettingsHead"><strong class="sourceSettingsLabel">Cryptocurrency intermediary</strong></div><div class="sourceOptions intermediaryOptions" aria-label="Cryptocurrency intermediaries">
                  <button type="button" class:sourceOptionActive={selectedIntermediaryAssets.length === 0} class="sourceOption" aria-pressed={selectedIntermediaryAssets.length === 0} on:click={() => { selectedIntermediaryAssets = []; resetResults(); }}>All available</button>
                  {#each INTERMEDIARY_ASSETS as asset}{@const enabled = selectedIntermediaryAssets.includes(asset)}<button type="button" class:sourceOptionActive={enabled} class="sourceOption" aria-pressed={enabled} on:click={() => toggleAsset(asset)}><span class="intermediaryAssetIcon" aria-hidden="true"><img src={intermediaryIcon(asset)} alt="" width="18" height="18" loading="lazy" decoding="async" on:error={fallbackAssetIcon} /></span>{asset}</button>{/each}
                </div></div>
                </div>
              </div>
            {/if}
          </div>
        </div>
      </div>

      <div class="intentLabel"><span>{t("Sell", {}, activeLocale)}</span></div>
      <div class="moneyPanel moneyPanelSource exchangeMoneyPanel">
        <div class="panelCopy"><label for="exchange-amount">{t("You send", {}, activeLocale)}</label><input id="exchange-amount" class="amountInput" type="text" inputmode="decimal" autocomplete="off" spellcheck="false" value={displayedSourceAmount} on:focus={(event) => event.currentTarget.select()} on:input={(event) => updateAmount(event.currentTarget.value)} aria-label={t("Amount to send", {}, activeLocale)} />{#if marketUsd.source !== null}<span class="marketValue" aria-label={t("Approximate USD market value", {}, activeLocale)}>{formatMarketUsd(marketUsd.source)}</span>{/if}</div>
        <div class="methodControls">
          <button type="button" class="methodTrigger" data-tooltip={sourceMethod?.kind === "wallet" ? `${sourceCurrencyChoice} · ${t(sourceMethod.name, {}, activeLocale)}` : t(methodTitle(sourceMethod), {}, activeLocale)} on:click={() => void openMethodPicker("source")} aria-label={`Select sending ${methodNoun(sourceMethod)}: ${sourceMethod ? methodTitle(sourceMethod) : "none"}`}>
            <span class="methodAvatar" style:background-color={paymentMethodFavicon(sourceMethod) ? "transparent" : sourceMethod?.color ?? "#171a17"} aria-hidden="true">{#if paymentMethodFavicon(sourceMethod)}<img src={paymentMethodFavicon(sourceMethod) ?? ""} alt="" width="48" height="48" loading="lazy" decoding="async" on:error={hideBrokenImage} /><span data-icon-fallback style="display:none">{sourceMethod?.initials ?? corridor?.source_country ?? "—"}</span>{:else}<span>{sourceMethod?.initials ?? corridor?.source_country ?? "—"}</span>{/if}</span>
            <span class="methodText" class:methodTextAsset={sourceMethod?.kind === "wallet"}><strong>{t(methodTitle(sourceMethod), {}, activeLocale)}</strong></span><svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </button>
          <div class="networkControl"><button type="button" class="networkButton" on:click={() => sourceMethod?.kind === "wallet" ? openNetworkPicker("source") : openCurrencyPicker("source")} aria-haspopup="dialog" aria-label={sourceMethod?.kind === "wallet" ? `Select sending network: ${sourceNetwork?.name ?? "none"}` : `Select sending currency: ${sourceCurrencyChoice}`} title={sourceMethod?.kind === "wallet" ? sourceNetwork?.name ?? "Select network" : sourceCurrencyChoice}><span class:currencyDot={sourceMethod?.kind !== "wallet"} class:flagDot={Boolean(sourceCurrencyFlag)} class="networkDot" aria-hidden="true">{#if sourceMethod?.kind === "wallet" && sourceNetwork}<img src={networkIcon(sourceNetwork.name)} alt="" width="18" height="18" loading="lazy" decoding="async" />{:else if sourceCurrencyFlag}<img src={sourceCurrencyFlag} alt="" width="18" height="18" decoding="async" />{:else}{currencyMark(sourceCurrencyChoice)}{/if}</span>{#if sourceMethod?.kind !== "wallet"}<span class="networkCopy"><strong>{sourceCurrencyChoice}</strong></span>{/if}<svg class="networkChevron" width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg></button></div>
        </div>
      </div>
      <ExchangeFlowBridge reversed={directionReversed} onSwap={swapDirection} label={t("Swap sender and recipient", {}, activeLocale)}><button type="button" class="routesToggle" class:routesToggleOpen={routesExpanded} on:click={toggleRoutes} aria-label={t(routesExpanded ? "Hide routes" : "Show routes", {}, activeLocale)} aria-expanded={routesExpanded} title={t(routesExpanded ? "Hide routes" : "Show routes", {}, activeLocale)}><span aria-hidden="true">❯</span></button></ExchangeFlowBridge>
      <div class="intentLabel intentLabelBuy"><span>{t("Buy", {}, activeLocale)}</span></div>
      <div class="moneyPanel moneyPanelTarget exchangeMoneyPanel">
        <div class="panelCopy"><label for="exchange-output">{t("Recipient gets", {}, activeLocale)}</label><input id="exchange-output" class:amountOutputEmpty={!previewRoute && amountSide !== "target"} class="amountInput amountOutput" type="text" inputmode="decimal" autocomplete="off" spellcheck="false" value={displayedTargetAmount} on:focus={(event) => event.currentTarget.select()} on:input={(event) => updateTargetAmount(event.currentTarget.value)} aria-label={t("Amount to receive", {}, activeLocale)} />{#if marketUsd.target !== null}<span class="marketValue" aria-label={t("Approximate USD market value", {}, activeLocale)}>{formatMarketUsd(marketUsd.target)}</span>{/if}</div>
        <div class="methodControls">
          <button type="button" class="methodTrigger" data-tooltip={targetMethod?.kind === "wallet" ? `${targetCurrencyChoice} · ${t(targetMethod.name, {}, activeLocale)}` : t(methodTitle(targetMethod), {}, activeLocale)} on:click={() => void openMethodPicker("target")} aria-label={`Select recipient ${methodNoun(targetMethod)}: ${targetMethod ? methodTitle(targetMethod) : "none"}`}>
            <span class="methodAvatar" style:background-color={paymentMethodFavicon(targetMethod) ? "transparent" : targetMethod?.color ?? "#171a17"} aria-hidden="true">{#if paymentMethodFavicon(targetMethod)}<img src={paymentMethodFavicon(targetMethod) ?? ""} alt="" width="48" height="48" loading="lazy" decoding="async" on:error={hideBrokenImage} /><span data-icon-fallback style="display:none">{targetMethod?.initials ?? corridor?.target_country ?? "—"}</span>{:else}<span>{targetMethod?.initials ?? corridor?.target_country ?? "—"}</span>{/if}</span>
            <span class="methodText" class:methodTextAsset={targetMethod?.kind === "wallet"}><strong>{t(methodTitle(targetMethod), {}, activeLocale)}</strong></span><svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </button>
          <div class="networkControl"><button type="button" class="networkButton" on:click={() => targetMethod?.kind === "wallet" ? openNetworkPicker("target") : openCurrencyPicker("target")} aria-haspopup="dialog" aria-label={targetMethod?.kind === "wallet" ? `Select recipient network: ${targetNetwork?.name ?? "none"}` : `Select recipient currency: ${targetCurrencyChoice}`} title={targetMethod?.kind === "wallet" ? targetNetwork?.name ?? "Select network" : targetCurrencyChoice}><span class:currencyDot={targetMethod?.kind !== "wallet"} class:flagDot={Boolean(targetCurrencyFlag)} class="networkDot" aria-hidden="true">{#if targetMethod?.kind === "wallet" && targetNetwork}<img src={networkIcon(targetNetwork.name)} alt="" width="18" height="18" loading="lazy" decoding="async" />{:else if targetCurrencyFlag}<img src={targetCurrencyFlag} alt="" width="18" height="18" decoding="async" />{:else}{currencyMark(targetCurrencyChoice)}{/if}</span>{#if targetMethod?.kind !== "wallet"}<span class="networkCopy"><strong>{targetCurrencyChoice}</strong></span>{/if}<svg class="networkChevron" width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg></button></div>
        </div>
      </div>
      {#if refreshSeconds > 0}<div class="marketBar"><div class="marketState"><span class="refreshProgress" role="img" aria-label={secondsUntilRefresh === null ? "Auto-refresh is off" : `Refresh in ${secondsUntilRefresh} seconds`}><svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true"><circle class="refreshTrack" cx="9" cy="9" r="7" pathLength="100" /><circle class="refreshFill" cx="9" cy="9" r="7" pathLength="100" style:stroke-dashoffset={`${100 - refreshProgress}`} /></svg></span><div><span>{lastUpdatedAt ? `Updated ${Math.max(0, Math.floor((clock - lastUpdatedAt) / 1000))}s ago` : t("Public P2P sources only · no order placement", {}, activeLocale)}</span></div></div>{#if secondsUntilRefresh !== null}<span class="nextRefresh">{secondsUntilRefresh}s</span>{/if}</div>{/if}
      <button type="button" class="cta" disabled={!hasAmount || (!previewRoute && (searching || !corridor))} on:click={runPrimaryAction} data-testid="start-search" aria-label={previewRoute ? t("Open route instructions", {}, activeLocale) : t("Find routes", {}, activeLocale)}>{#if previewRoute}{t("Go", {}, activeLocale)} <span>↗</span>{:else if searching}<span class="spinner"></span> {t("Finding routes", {}, activeLocale)}{:else if hasAmount}{t("Find routes", {}, activeLocale)} <span>↗</span>{:else}{t("Enter an amount to begin", {}, activeLocale)}{/if}</button>
      {#if error}<div class="errorBox" role="alert">{error}</div>{/if}
    </div>
    <div class="panelToggles">
      <button type="button" class="chartToggle" class:chartToggleOpen={activityExpanded} on:click={toggleActivityGraph} aria-label={t(activityExpanded ? "Hide search activity" : "Show search activity", {}, activeLocale)} aria-expanded={activityExpanded || activityModalOpen} title={t(activityExpanded ? "Hide search activity" : "Show search activity", {}, activeLocale)}><span aria-hidden="true">❯</span></button>
      <button type="button" class="mobileRoutesToggle" class:mobileRoutesToggleOpen={routesExpanded} on:click={toggleRoutes} aria-label={t(routesExpanded ? "Hide routes" : "Show routes", {}, activeLocale)} aria-expanded={routesExpanded} title={t(routesExpanded ? "Hide routes" : "Show routes", {}, activeLocale)}><span aria-hidden="true">❯</span></button>
    </div>
    {#if activityExpanded && !guideRequested}
      <div class="activityReveal" bind:this={activityRevealElement} in:fly={{ y: 18, duration: 380, easing: quintOut }}>
        <SearchActivityChart sourceCurrency={selectedSourceCurrency} targetCurrency={selectedTargetCurrency} hours={activityHours} period={activityPeriod} onPeriodChange={selectActivityPeriod} loading={activityLoading} error={activityError} />
      </div>
    {/if}
    </div>
    {#if routesExpanded}
      <div class="routesReveal" bind:this={routesRevealElement} transition:routesRevealTransition inert={!routesExpanded} aria-hidden={!routesExpanded}>
        <SidePanel {routes} {routesFound} sourceCurrency={selectedSourceCurrency} targetCurrency={selectedTargetCurrency} selectedRouteId={selected?.route_id ?? null} onSelect={selectRoute} onOpenInstructions={openInstructions} onVote={voteForRoute} {searching} {renderingRoutes} {searchingVenues} {foundVenues} {venueStats} {venueNames} networkNames={Object.fromEntries(networks.map((network) => [network.id, network.name]))} searched={lastUpdatedAt !== null} {hasAmount} {showBelarusP2pWarning} {onOpenBelarusP2pWarning} onOpenSearchActivity={() => activityModalOpen = true} />
      </div>
    {/if}
  </div>
  {#if activityModalOpen && !guideRequested}<SearchActivityModal sourceCurrency={selectedSourceCurrency} targetCurrency={selectedTargetCurrency} hours={activityHours} period={activityPeriod} onPeriodChange={selectActivityPeriod} loading={activityLoading} error={activityError} onClose={() => activityModalOpen = false} />{/if}
  {#if walkthroughOpen && walkthroughComponent}<svelte:component this={walkthroughComponent} card={converterCard} {paymentMethods} {networks} onClose={() => walkthroughOpen = false} />{/if}
  {#if currencyPicker === "source" || currencyPicker === "target"}<CurrencyPicker open={currencyPicker !== null} selected={currencyPicker === "source" ? sourceCurrencyChoice : targetCurrencyChoice} choices={currencyPicker === "source" ? sourceCurrencyChoices : targetCurrencyChoices} onClose={() => currencyPicker = null} onSelect={(choice) => chooseCurrency(currencyPicker ?? "source", choice)} />{/if}
  {#if paymentPickerComponent}<svelte:component this={paymentPickerComponent} open={methodPicker === "source"} title="Choose where you pay from" role="sender" {networks} {paymentMethods} selected={sourceMethod} selectedNetwork={sourceNetwork} onClose={() => methodPicker = null} onSelect={chooseSource} /><svelte:component this={paymentPickerComponent} open={methodPicker === "target"} title="Choose where the recipient gets paid" role="recipient" {networks} {paymentMethods} selected={targetMethod} selectedNetwork={targetNetwork} onClose={() => methodPicker = null} onSelect={chooseTarget} />{/if}
  {#if networkPicker === "source" || networkPicker === "target"}<NetworkPicker open={networkPicker !== null} networks={networkPicker === "source" ? sourceNetworks : targetNetworks} selected={networkPicker === "source" ? sourceNetwork : targetNetwork} onClose={() => networkPicker = null} onSelect={selectNetwork} />{/if}

</section>
{#if guideRequested}
  {#if routeInstructionsComponent && instructionsRoute}
    {#key instructionsRoute.route_id}
      <svelte:component this={routeInstructionsComponent} route={instructionsRoute} {venueNames} {venueUrls} {providerGuidance} networkNames={Object.fromEntries(networks.map((network) => [network.id, network.name]))} onOpenService={openService} onClose={closeGuide} />
    {/key}
  {:else}
    <section class="guideLoading" aria-live="polite"><span class="loadingMark">↗</span><h1>{t(guideUnavailable ? "This route is no longer available" : error ? "Could not load this guide" : "Preparing your guide", {}, activeLocale)}</h1><p>{t(guideUnavailable ? "Quotes have changed. Choose a current route to continue." : error ? "Try again or choose a current route." : "Checking the selected banks, networks and platforms for a fresh quote.", {}, activeLocale)}</p><button type="button" on:click={() => locationChanged(true)}>{t("Try again", {}, activeLocale)}</button><button type="button" on:click={closeGuide}>← {t("Back to routes", {}, activeLocale)}</button></section>
  {/if}
{/if}

<style>
.shell[hidden] { display: none; }
.guideLoading { min-height: 70vh; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 24px; padding: 32px; text-align: center; }.loadingMark { font-size: 50px; color: var(--color-accent-text); }.guideLoading h1 { font-size: 32px; }.guideLoading p { color: var(--color-text-soft); }.guideLoading button { min-height: 44px; padding: 10px 20px; border: 1px solid var(--color-border); border-radius: 12px; }

.shell {
  position: relative;
  width: 100%;
  padding: 62px var(--page-gutter) 0;
  font-family: var(--font-sans);
}

.hero {
  width: min(900px, 100%);
  margin: 0 auto 42px;
  text-align: center;
}

.shell.introReady .workspace { animation: workspaceIn .72s cubic-bezier(.22, 1, .36, 1) both; }
.shell.introReady .hero > p { animation: heroIn .5s .1s cubic-bezier(.22, 1, .36, 1) both; }
.introOverlay { pointer-events: none; position: fixed; inset: 0; z-index: 1000; overflow: hidden; background: var(--shell-gradient); }
.introTitle { position: absolute; top: 50%; left: 50%; width: min(900px, calc(100vw - 48px)); margin: 0; color: var(--color-text); font-size: clamp(44px, 5.5vw, 72px); font-weight: 650; letter-spacing: -.065em; line-height: .96; text-align: center; transform: translate(-50%, -50%) scale(1.2); }
.introStarted .introTitle { animation: introDock .68s 3.08s cubic-bezier(.22, 1, .36, 1) both; }
.introClip { display: inline-block; clip-path: inset(-.12em -.16em -.18em -.16em); vertical-align: bottom; }
.introSecondWithDot { white-space: nowrap; }
.introWord { display: inline-block; transform: translateY(115%); opacity: 0; }
.introStarted .introWordMove { animation: introRise .48s .2s cubic-bezier(.22, 1, .36, 1) forwards; }
.introStarted .introWordMoney { animation: introRise .48s .7s cubic-bezier(.22, 1, .36, 1) forwards; }
.introStarted .introWordKeep { animation: introRise .48s 1.35s cubic-bezier(.22, 1, .36, 1) forwards; }
.introStarted .introWordMore { animation: introRise .48s 1.85s cubic-bezier(.22, 1, .36, 1) forwards; }
.introPunctuation { opacity: 0; }
.introStarted .introFirstPunctuation { animation: introDot .02s 1.18s linear forwards; }
.introStarted .introLastPunctuation { animation: introDot .02s 2.94s linear forwards; }
.introEmphasis { position: relative; z-index: 0; white-space: nowrap; }
.introEmphasis::after { position: absolute; right: -.05em; bottom: .02em; left: -.04em; z-index: -1; height: .2em; border-radius: 3px; background: var(--color-accent); content: ""; transform: rotate(-1deg) scaleX(0); transform-origin: left center; }
.introStarted .introEmphasis::after { animation: introUnderline .48s 2.42s cubic-bezier(.22, 1, .36, 1) forwards; }
.introMarker { position: absolute; bottom: -.9em; left: -.04em; z-index: 2; width: 1.02em; height: 1.02em; pointer-events: none; opacity: 0; }
.introStarted .introMarker { animation: introMarkerDraw .48s 2.42s cubic-bezier(.22, 1, .36, 1) both; }
:global(html[data-theme="dark"]) .introMarker { filter: brightness(0) invert(1); }
.shell.localeLong .introTitle { font-size: clamp(42px, 5vw, 68px); }
@media (max-width: 640px) { .introTitle { width: calc(100vw - 24px); font-size: 44px; transform: translate(-50%, -50%) scale(1.08); } .shell.localeLong .introTitle { font-size: 44px; transform: translate(-50%, -50%) scale(1); } .shell.localeLong .introEmphasis { white-space: normal; } }
@keyframes introRise { to { opacity: 1; transform: translateY(0); } }
@keyframes introDot { to { opacity: 1; } }
@keyframes introUnderline { to { transform: rotate(-1deg) scaleX(1); } }
@keyframes introMarkerDraw { 0% { left: -.04em; opacity: 0; } 8%, 88% { opacity: 1; } 100% { left: calc(100% + .05em); opacity: 0; } }
@keyframes introDock { to { transform: translate(calc(-50% + var(--intro-x)), calc(-50% + var(--intro-y))) scale(1); } }

.nojsNotice { max-width: 900px; margin: 0 auto 24px; color: var(--color-text-soft); font-size: 16px; line-height: 1.6; }

.modeTabs,
.cardActions,
.intentLabel,
.marketBar,
.marketState,
.nextRefresh,
.selectionBox,
.authBrand,
.demoNote {
  display: flex;
  align-items: center;
}

.hero h1 {
  max-width: 900px;
  margin: 0 auto;
  color: var(--color-text);
  font-size: clamp(44px, 5.5vw, 72px);
  font-weight: 650;
  letter-spacing: -0.065em;
  line-height: 0.96;
}

.hero h1 span {
  position: relative;
  z-index: 0;
  white-space: normal;
}

.hero h1 span::after {
  position: absolute;
  right: -0.05em;
  bottom: 0.02em;
  left: -0.04em;
  z-index: -1;
  height: 0.2em;
  border-radius: 3px;
  background: var(--color-accent);
  content: "";
  transform: rotate(-1deg);
}

@media (min-width: 981px) {
  .hero h1 { white-space: nowrap; }
  .shell.localeLong .hero h1 {
    font-size: clamp(42px, 5vw, 68px);
    white-space: normal;
  }
}




.hero > p {
  max-width: 650px;
  margin: 24px auto 0;
  color: var(--color-text-soft);
  font-size: 16px;
  font-weight: 520;
  line-height: 1.7;
}

.workspace {
  display: grid;
  width: min(1130px, 100%);
  grid-template-columns: minmax(0, 560px) minmax(0, 1fr);
  align-items: start;
  gap: 18px;
  margin: 0 auto;
}

.card {
  position: relative;
  z-index: 2;
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 10px;
  padding: 18px;
  overflow: visible;
  border: 1px solid rgba(255, 255, 255, 0.78);
  border-radius: var(--radius-card);
  background: rgba(255, 255, 252, 0.96);
  box-shadow: none;
  backdrop-filter: blur(28px) saturate(145%);
  -webkit-backdrop-filter: blur(28px) saturate(145%);
  isolation: isolate;
}

.card::before {
  position: absolute;
  inset: 0;
  z-index: -1;
  border-radius: inherit;
  background: linear-gradient(145deg, rgba(255,255,255,0.8), transparent 34%);
  content: "";
}

.cardTop {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 42px;
  padding: 0 2px 5px;
}

.modeTabs {
  gap: 3px;
  padding: 3px;
  border-radius: var(--radius-pill);
  background: var(--color-panel);
}

.modeTabs button {
  min-height: 34px;
  padding: 0 15px;
  border-radius: var(--radius-pill);
  color: var(--color-text-faint);
  font-size: 12px;
  font-weight: 750;
}

.modeTabs button:disabled {
  cursor: not-allowed;
  opacity: 0.48;
}

.modeTabs .modeActive {
  background: #fff;
  color: var(--color-text);
  box-shadow: none;
}

.helpButton { display: grid; width: 39px; height: 39px; place-items: center; border-radius: 8px; color: var(--color-text-faint); font: 750 19px/1 var(--font-sans); transition: color .16s, background .16s; }
.helpButton:hover, .helpButton:focus-visible { color: #132015; background: var(--color-accent); }

.cardActions {
  gap: 5px;
}

.refreshButton,
.exchangesButton,
.settingsButton {
  display: grid;
  width: 39px;
  height: 39px;
  place-items: center;
  border: 1px solid transparent;
  border-radius: 13px;
  color: var(--color-text-soft);
  transition: background 0.15s ease, border-color 0.15s ease, transform 0.15s ease;
}

.refreshButton:hover:not(:disabled),
.exchangesButton:hover,
.settingsButton:hover {
  border-color: var(--color-border);
  border-width: var(--border-highlight-width);
  background: var(--color-panel);
}

.refreshButton:disabled {
  cursor: not-allowed;
  opacity: 0.36;
}

.refreshButton img,
.exchangesButton img,
.settingsButton img {
  display: block;
  width: 18px;
  height: 18px;
  object-fit: contain;
  filter: brightness(0);
}

:global(html[data-theme="dark"]) .refreshButton img,
:global(html[data-theme="dark"]) .exchangesButton img,
:global(html[data-theme="dark"]) .settingsButton img {
  filter: brightness(0) invert(1);
}

.refreshSpin {
  animation: spin 0.75s linear infinite;
}

.settingsWrap {
  position: relative;
}

.settingsBackdrop {
  position: static;
}

.settingsMenu {
  position: absolute;
  top: calc(100% + 9px);
  right: 0;
  z-index: 80;
  width: 310px;
  max-height: min(680px, calc(100vh - 32px));
  overflow-y: auto;
  padding: 17px;
  border: 1px solid var(--color-border);
  border-radius: 20px;
  background: rgba(255, 255, 255, 0.97);
  box-shadow: none;
  backdrop-filter: blur(24px);
  animation: popIn 0.18s ease;
}

.settingsModalHeader {
  display: none;
}

.settingsClose {
  display: none;
}

.settingsSheetHandle {
  display: none;
}

.settingsHead {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.settingsHead > div {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.settingsHead strong {
  font-size: 13px;
  font-weight: 800;
}

.settingsHead span {
  color: var(--color-text-faint);
  font-size: 12px;
  line-height: 1.5;
}

.refreshOptions {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 5px;
  margin: 17px 0 13px;
}

.refreshOptions button {
  height: 35px;
  border-radius: 10px;
  background: var(--color-panel);
  color: var(--color-text-soft);
  font-size: 12px;
  font-weight: 750;
}

.refreshOptions button[aria-pressed="true"] {
  background: var(--color-accent);
  color: #171717;
}

.sourceSettings {
  margin-top: 15px;
  padding-top: 14px;
  border-top: 1px solid var(--color-border);
}

.sourceSettingsLabel {
  display: block;
  margin-bottom: 8px;
  color: var(--color-text);
  font-size: 13px;
  font-weight: 800;
}

.intermediarySettingsHead {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}

.intermediarySettingsHead .sourceSettingsLabel {
  margin-bottom: 8px;
}

.intermediarySettingsHead small {
  color: var(--color-text-faint);
  font-family: var(--font-mono);
  font-size: 12px;
  white-space: nowrap;
}

.sourceOptions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

.exchangeMethodOptions .sourceOption {
  display: flex;
  align-items: center;
  justify-content: center;
}

.exchangeModalOptions {
  grid-template-rows: repeat(9, minmax(32px, auto));
  grid-auto-flow: column;
  grid-auto-columns: minmax(130px, 1fr);
  margin-top: 14px;
}

.exchangesMenu {
  width: max-content;
  max-width: min(640px, calc(100vw - 32px));
}

.alwaysOnProviders {
  margin-top: 16px;
  padding-top: 14px;
  border-top: 1px solid var(--color-border);
}

.alwaysOnHeading {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
}

.alwaysOnHeading strong {
  font-size: 12px;
}

.alwaysOnHeading span {
  color: var(--color-text-faint);
  font-size: 12px;
}

.exchangeOptions .alwaysOnProvider {
  justify-content: flex-start;
  background: rgba(45, 142, 69, 0.07);
  color: var(--color-text);
}

.alwaysOnProvider small {
  margin-left: auto;
  color: var(--color-good);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.intermediaryOptions {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.intermediaryOptions .sourceOption:first-child {
  grid-column: 1 / -1;
}

.intermediaryAssetIcon {
  display: inline-grid;
  width: 18px;
  height: 18px;
  flex: 0 0 18px;
  place-items: center;
}

.intermediaryAssetIcon img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.sourceOptionIcon {
  display: inline-grid;
  width: 18px;
  height: 18px;
  flex: 0 0 18px;
  place-items: center;
}

.sourceOptionIcon img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.exchangeOptions .sourceOption {
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: center;
  gap: 5px;
  line-height: 1;
  text-align: left;
  white-space: nowrap;
}

.intermediaryOptions .sourceOption {
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: center;
  gap: 5px;
  line-height: 1;
  text-align: left;
  white-space: nowrap;
}

.sourceOption {
  min-height: 32px;
  padding: 0 9px;
  border: 1px solid var(--color-border);
  border-radius: 10px;
  background: var(--color-panel);
  color: var(--color-text-soft);
  font-size: 12px;
  font-weight: 800;
  transition: border-color 0.15s ease, background 0.15s ease, color 0.15s ease, box-shadow 0.15s ease;
}

.sourceOption:hover {
  border-color: var(--color-accent);
  border-width: var(--border-highlight-width);
}

.sourceOptionActive {
  border-color: var(--color-accent);
  border-width: var(--border-highlight-width);
  background: var(--color-accent);
  color: #171717;
  box-shadow: none;
}

.intentLabel {
  justify-content: space-between;
  padding: 5px 5px 3px;
  color: var(--color-text-faint);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.moneyPanel {
  position: relative;
  display: flex;
  min-height: 136px;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 22px;
  overflow: hidden;
  border: 1px solid transparent;
  border-radius: 24px;
  transition: border-color 0.18s ease, box-shadow 0.18s ease, transform 0.18s ease;
}

.moneyPanel::after {
  position: absolute;
  top: -40px;
  right: -30px;
  width: 150px;
  height: 150px;
  border-radius: 50%;
  content: "";
  opacity: 0.6;
  filter: blur(8px);
  pointer-events: none;
}

.moneyPanelSource {
  background: linear-gradient(145deg, #f1f1ec 0%, #f7f7f2 100%);
}

.moneyPanelSource::after {
  background: rgba(185, 242, 39, 0.2);
}

.moneyPanelTarget {
  background: linear-gradient(145deg, #f5f2ff 0%, #f8f7fc 100%);
}

.moneyPanelTarget::after {
  background: rgba(117, 88, 246, 0.14);
}

.moneyPanel:hover,
.moneyPanel:focus-within {
  border-color: var(--color-border-strong);
  border-width: var(--border-highlight-width);
  box-shadow: none; outline: var(--focus-ring-width) solid var(--color-focus); outline-offset: 2px;
}

.panelCopy {
  position: relative;
  z-index: 1;
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  gap: 6px;
}

.panelCopy label {
  color: var(--color-text-soft);
  font-size: 12px;
  font-weight: 750;
}

.amountInput,
.amountOutput,
.amountOutputEmpty {
  display: block;
  width: 100%;
  overflow: hidden;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--color-text);
  font-family: var(--font-sans);
  font-size: clamp(30px, 4vw, 42px);
  font-weight: 700;
  letter-spacing: -0.055em;
  line-height: 1.05;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.amountOutputEmpty {
  color: var(--color-text-faint);
}

.methodTrigger {
  position: relative;
  z-index: 1;
  display: flex;
  width: min(220px, 52%);
  min-height: 68px;
  flex: 0 0 auto;
  align-items: center;
  gap: 10px;
  padding: 8px 12px 8px 8px;
  overflow: hidden;
  border: 1px solid rgba(19, 22, 19, 0.09);
  border-radius: 19px;
  background: rgba(255, 255, 255, 0.8);
  color: var(--color-text);
  text-align: left;
  box-shadow: none;
  /* Avoid repaint-heavy backdrop sampling while the browser is zooming. */
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  transition: border-color 0.15s ease, transform 0.15s ease, box-shadow 0.15s ease;
}

.methodTrigger:hover {
  border-color: rgba(19, 22, 19, 0.2);
  border-width: var(--border-highlight-width);
  box-shadow: none;
  transform: translateY(-1px);
}

.methodAvatar {
  position: relative;
  display: grid;
  width: 48px;
  height: 48px;
  flex: 0 0 auto;
  place-items: center;
  border: 2px solid rgba(255, 255, 255, 0.72);
  border-radius: 15px;
  color: #fff;
  box-shadow: none;
  font-size: 12px;
  font-weight: 850;
  letter-spacing: 0.02em;
}

.methodAvatar img {
  position: absolute;
  inset: 5px;
  width: calc(100% - 10px);
  height: calc(100% - 10px);
  border-radius: 10px;
  object-fit: contain;
}

.methodText {
  display: flex;
  min-width: 0;
  flex: 1;
}

.methodText strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.methodText strong {
  font-size: 12px;
  font-weight: 800;
}

.marketBar {
  justify-content: space-between;
  gap: 16px;
  min-height: 28px;
  padding: 0 2px;
}

.marketState {
  min-width: 0;
  gap: 8px;
}

.marketState > div {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.marketState strong {
  overflow: hidden;
  font-size: 12px;
  font-weight: 800;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.marketState > div > span {
  overflow: hidden;
  color: var(--color-text-faint);
  font-size: 12px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.refreshProgress {
  display: inline-flex;
  width: 18px;
  height: 18px;
  flex: 0 0 auto;
}

.refreshProgress svg {
  overflow: visible;
}

.refreshTrack,
.refreshFill {
  fill: none;
  stroke-width: 2;
}

.refreshTrack {
  stroke: #d3d5ce;
}

.refreshFill {
  stroke: var(--color-good);
  stroke-dasharray: 100;
  transform: rotate(-90deg);
  transform-origin: center;
  transition: stroke-dashoffset 0.3s ease;
}

.nextRefresh {
  flex: 0 0 auto;
  color: var(--color-text-soft);
  font-family: var(--font-mono);
  font-size: 12px;
}

.cta {
  display: inline-flex;
  width: 100%;
  height: 66px;
  align-items: center;
  justify-content: center;
  gap: 10px;
  margin-top: 2px;
  border-radius: 19px;
  background: var(--color-primary);
  color: #fff;
  box-shadow: none;
  font-size: 14px;
  font-weight: 800;
  letter-spacing: -0.01em;
  transition: background 0.16s ease, transform 0.16s ease, box-shadow 0.16s ease;
}

.cta span:not(.spinner) {
  color: var(--color-accent);
  font-size: 20px;
}

.cta:hover:not(:disabled) {
  background: #050605;
  box-shadow: none;
  transform: translateY(-1px);
}

.cta:disabled {
  cursor: not-allowed;
  background: #e8e9e3;
  color: #9c9e96;
  box-shadow: none;
}

.spinner {
  width: 17px;
  height: 17px;
  border: 2px solid rgba(255, 255, 255, 0.25);
  border-top-color: var(--color-accent);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

.selectionBox {
  gap: 11px;
  padding: 12px 14px;
  border: 1px solid rgba(45, 142, 69, 0.16);
  border-radius: 17px;
  background: rgba(45, 142, 69, 0.06);
}

.selectionIcon {
  display: grid;
  width: 34px;
  height: 34px;
  flex: 0 0 auto;
  place-items: center;
  border-radius: 11px;
  background: var(--color-good);
  color: #fff;
  font-size: 13px;
  font-weight: 900;
}

.selectionBox > div:last-child {
  display: grid;
  min-width: 0;
  flex: 1;
  grid-template-columns: 1fr auto;
  gap: 2px 10px;
}

.selectionBox strong,
.selectionBox span {
  font-size: 12px;
  font-weight: 800;
}

.selectionBox small {
  grid-column: 1 / -1;
  color: var(--color-text-faint);
  font-size: 12px;
}

.errorBox {
  padding: 11px 13px;
  border: 1px solid rgba(212, 61, 53, 0.13);
  border-radius: 14px;
  background: rgba(212, 61, 53, 0.06);
  color: var(--color-danger);
  font-size: 12px;
  font-weight: 700;
  line-height: 1.45;
}

.authBackdrop {
  position: fixed;
  inset: 0;
  z-index: 1200;
  display: grid;
  place-items: center;
  padding: 20px;
  background: rgba(15, 17, 14, 0.68);
  backdrop-filter: blur(18px) saturate(120%);
  -webkit-backdrop-filter: blur(18px) saturate(120%);
  animation: fadeIn 0.2s ease;
}

.authModal {
  width: min(100%, 455px);
  animation: authIn 0.35s cubic-bezier(0.22, 1, 0.36, 1);
}

.authBox {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 30px;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.72);
  border-radius: 30px;
  background: rgba(250, 250, 246, 0.97);
  box-shadow: none;
}

.authBox::before {
  position: absolute;
  top: -90px;
  right: -80px;
  width: 220px;
  height: 220px;
  border-radius: 50%;
  background: var(--color-accent);
  content: "";
  filter: blur(4px);
  opacity: 0.55;
}

.authBrand {
  position: relative;
  justify-content: space-between;
  color: var(--color-text-soft);
  font-size: 12px;
  font-weight: 850;
  letter-spacing: 0.14em;
}

.authMark {
  display: grid;
  width: 38px;
  height: 38px;
  place-items: center;
  border-radius: 12px;
  background: var(--color-primary);
  color: var(--color-accent);
  font-size: 12px;
  letter-spacing: -0.03em;
}

.authCopy {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 13px 0 4px;
}

.authEyebrow {
  color: var(--color-violet);
  font-size: 12px;
  font-weight: 850;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.authCopy strong {
  max-width: 350px;
  font-size: 29px;
  font-weight: 700;
  letter-spacing: -0.055em;
  line-height: 1.08;
}

.authCopy p {
  max-width: 350px;
  color: var(--color-text-soft);
  font-size: 12px;
  line-height: 1.55;
}

.fieldLabel {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 7px;
  color: var(--color-text-soft);
  font-size: 12px;
  font-weight: 800;
}

.textInput {
  width: 100%;
  height: 53px;
  padding: 0 15px;
  border: 1px solid var(--color-border-strong);
  outline: 0;
  border-radius: 15px;
  background: #fff;
  font-size: 13px;
  font-weight: 650;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}

.textInput:focus {
  border-color: var(--color-violet);
  box-shadow: none; outline: var(--focus-ring-width) solid var(--color-focus); outline-offset: 2px;
}

.demoNote {
  gap: 7px;
  color: var(--color-text-faint);
  font-size: 12px;
}

.demoNote span {
  padding: 4px 7px;
  border-radius: var(--radius-pill);
  background: var(--color-accent-soft);
  color: #536d0f;
  font-size: 12px;
  font-weight: 850;
  text-transform: uppercase;
}

.secondaryButton {
  width: 100%;
  height: 56px;
  border-radius: 16px;
  background: var(--color-primary);
  color: #fff;
  box-shadow: none;
  font-size: 13px;
  font-weight: 800;
  transition: background 0.15s ease, transform 0.15s ease;
}

.secondaryButton:hover:not(:disabled) {
  background: #000;
  transform: translateY(-1px);
}

.secondaryButton:disabled {
  cursor: wait;
  opacity: 0.65;
}

.authLegal {
  color: var(--color-text-faint);
  font-size: 12px;
  line-height: 1.5;
  text-align: center;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@keyframes scanPulse {
  0%, 100% { opacity: 0.55; transform: scale(0.82); }
  50% { opacity: 1; transform: scale(1.12); }
}

@keyframes heroIn {
  from { opacity: 0; transform: translateY(18px); }
  to { opacity: 1; transform: translateY(0); }
}

@keyframes workspaceIn {
  from { opacity: 0; transform: translateY(25px) scale(0.985); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

@keyframes popIn {
  from { opacity: 0; transform: translateY(-5px) scale(0.98); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes authIn {
  from { opacity: 0; transform: translateY(18px) scale(0.97); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

@media (max-width: 980px) {
  .shell {
    padding-top: 44px;
  }

  .workspace {
    width: min(610px, 100%);
    grid-template-columns: 1fr;
    gap: 18px;
  }

  .hero {
    margin-bottom: 32px;
  }
}

@media (max-width: 640px) {
  .shell {
    padding: 35px 12px 0;
  }

  .hero h1 {
    font-size: clamp(43px, 14vw, 64px);
  }

  .hero > p {
    font-size: 13px;
  }

  .card {
    padding: 12px;
    border-radius: 25px;
  }

  .moneyPanel {
    min-height: 184px;
    align-items: stretch;
    flex-direction: column;
    padding: 18px;
  }

  .methodTrigger {
    width: 100%;
  }

  .amountInput,
  .amountOutput,
  .amountOutputEmpty {
    font-size: 36px;
  }

  .marketBar {
    gap: 10px;
  }

  .authBox {
    padding: 24px 20px;
    border-radius: 24px;
  }
}

/* Calm bridge layout: a compact, paper-like workspace with lime route accents. */
.shell {
  padding-top: 34px;
}

.hero {
  width: min(900px, 100%);
  margin: 0 auto 26px;
  text-align: center;
}

.hero h1 {
  margin: 0 auto;
  font-size: clamp(44px, 5.5vw, 72px);
  letter-spacing: -0.065em;
}



.hero > p {
  max-width: 560px;
  margin: 12px auto 0;
  color: var(--color-text-soft);
  font-size: 16px;
  line-height: 1.55;
}

/* Translated slogans can be longer than the compact English slogan. */
.hero h1 {
  max-width: 100%;
  text-wrap: balance;
}



.workspace {
  --workspace-gap: 32px;
  width: min(var(--layout-width), 100%);
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: var(--workspace-gap);
}

@media (min-width: 981px) {
  .workspace:not(.routesCollapsed) {
    width: min(var(--layout-width), 100%);
    grid-template-columns: minmax(0, min(var(--converter-width), 49%)) minmax(0, 1fr);
  }

  .workspace:not(.routesCollapsed) .amountInput {
    font-size: clamp(28px, calc(2.9vw - (var(--amount-digits) - 10) * 1px), 38px);
  }
}

.card.modalOpen {
  z-index: 5;
}

.converterStack { display: flex; min-width: 0; flex-direction: column; transition: transform .38s cubic-bezier(.22, 1, .36, 1); }
.workspace.routesCollapsed .converterStack { transform: translateX(calc(50% + var(--workspace-gap) / 2)); }
.routesReveal { display: flex; min-width: 0; align-self: start; }
.routesReveal :global(.side) { width: 100%; }
.workspace.activityExpanded:not(.routesCollapsed) .converterStack { align-self: stretch; }
.workspace.activityExpanded .routesReveal { min-height: 690px; align-self: stretch; contain: size; }
.workspace.activityExpanded .routesReveal :global(.side) { height: 100%; }
.workspace.activityExpanded .routesReveal :global(.side .panel) { height: 100%; min-height: 0; }
.panelToggles { position: relative; display: flex; height: 42px; flex: 0 0 auto; align-items: center; justify-content: center; padding: 8px 0 4px; }
.chartToggle, .routesToggle, .mobileRoutesToggle { z-index: 3; display: grid; width: 48px; height: 30px; flex: 0 0 auto; place-items: center; padding: 0; border: 0; background: transparent; box-shadow: none; color: var(--color-text-soft); opacity: 1; cursor: pointer; transition: opacity .2s ease; -webkit-tap-highlight-color: transparent; }
.routesToggle { position: absolute; top: 50%; right: calc(-1 * (var(--workspace-gap) + 20px)); width: var(--workspace-gap); transform: translateY(-50%); }
.mobileRoutesToggle { display: none; }
.chartToggle:hover, .routesToggle:hover, .mobileRoutesToggle:hover { opacity: .85; }
.chartToggle:focus-visible, .routesToggle:focus-visible, .mobileRoutesToggle:focus-visible { outline: var(--focus-ring-width) solid var(--color-focus); outline-offset: 2px; }
.chartToggle span, .routesToggle span, .mobileRoutesToggle span { display: block; font-size: 21px; line-height: 1; -webkit-text-stroke: .55px currentColor; transition: transform .26s ease; }
.chartToggle span, .mobileRoutesToggle span { transform: rotate(90deg); }
.chartToggleOpen span, .mobileRoutesToggleOpen span { transform: rotate(-90deg); }
.routesToggleOpen span { transform: rotate(180deg); }
.activityReveal { width: 100%; min-height: 176px; flex: 1 1 auto; overflow: visible; }
.activityReveal :global(.activityCard) { height: 100%; }
@media (max-width: 980px) { .workspace { --workspace-gap: 22px; gap: 0; } .routesReveal { margin-top: 18px; } .workspace.routesCollapsed .converterStack { transform: none; } .activityReveal { display: none; } .routesToggle, .chartToggle { display: none; } .mobileRoutesToggle { display: grid; } }
@media (prefers-reduced-motion: reduce) { .converterStack, .chartToggle span, .routesToggle span, .mobileRoutesToggle span { transition: none; } }

.card {
  min-height: 0;
  gap: 12px;
  padding: 19px;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  background: var(--exchange-card-bg);
  box-shadow: none;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.card::before {
  display: none;
}

.cardTop {
  min-height: 34px;
  padding: 0 1px 5px;
}

.modeTabs {
  padding: 0;
  background: transparent;
}

.modeTabs button {
  min-height: 32px;
  padding: 0;
  color: var(--color-text);
  font-size: 16px;
  font-weight: 600;
  letter-spacing: -0.02em;
}

.modeTabs button + button {
  display: none;
}

.modeTabs .modeActive {
  background: transparent;
  box-shadow: none;
}

.cardActions {
  gap: 8px;
}

.refreshButton,
.exchangesButton,
.settingsButton {
  width: 32px;
  height: 32px;
  border-radius: 9px;
}

.intentLabel {
  justify-content: flex-start;
  gap: 8px;
  margin: 0 3px 9px;
  padding: 0;
  color: #687164;
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 500;
  letter-spacing: 0.07em;
  text-transform: uppercase;
}

.intentLabel > span:first-child::before {
  display: inline-block;
  width: 5px;
  height: 5px;
  margin-right: 8px;
  border-radius: 50%;
  background: #d9534f;
  content: "";
  vertical-align: 1px;
}

.intentLabelBuy > span:first-child::before {
  background: var(--color-accent);
}

.moneyPanel {
  min-height: 96px;
  flex-wrap: wrap;
  align-items: center;
  padding: 15px 15px 12px;
  border: 1px solid var(--exchange-field-border);
  border-radius: 7px;
  background: var(--exchange-field-bg);
  box-shadow: none;
}

.moneyPanelSource,
.moneyPanelTarget {
  background: var(--exchange-field-bg);
}

.moneyPanel::after {
  display: none;
}

.panelCopy {
  gap: 5px;
}

.panelCopy label {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
  white-space: nowrap;
}

.marketValue {
  overflow: hidden;
  color: var(--color-text-soft);
  font-size: 12px;
  font-weight: 600;
  line-height: 1.2;
  font-variant-numeric: tabular-nums;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.amountInput,
.amountOutput,
.amountOutputEmpty {
  font-family: var(--font-mono);
  font-size: clamp(28px, 3.2vw, 38px);
  font-weight: 500;
  letter-spacing: -0.05em;
}

.methodTrigger {
  width: min(190px, 48%);
  min-height: 42px;
  gap: 7px;
  padding: 6px 8px 6px 6px;
  border: 1px solid var(--exchange-asset-border);
  border-radius: 5px;
  background: var(--exchange-asset-bg);
  box-shadow: none;
}

.methodTrigger:hover {
  border-color: #a4af9e;
  border-width: var(--border-highlight-width);
  background: #edf2e8;
  box-shadow: none;
  transform: translateY(-1px);
}

.methodAvatar {
  width: 28px;
  height: 28px;
  border: 0;
  border-radius: 50%;
  box-shadow: none;
  font-size: 12px;
}

.methodAvatar img {
  inset: 4px;
  width: calc(100% - 8px);
  height: calc(100% - 8px);
  border-radius: 50%;
}

.methodText strong {
  font-size: 14px;
}

.methodTextAsset strong {
  font-size: 14px;
  font-weight: 850;
}

.marketBar {
  min-height: 25px;
}

.cta {
  height: 48px;
  margin-top: 3px;
  border: 1px solid #b9e834;
  border-radius: 5px;
  background: var(--color-accent);
  color: #171717;
  box-shadow: none;
  font-size: 12px;
}

.cta span:not(.spinner) {
  color: #2e2e2e;
  font-size: 17px;
}

.cta:hover:not(:disabled) {
  background: #c9f34b;
  box-shadow: none;
  transform: translateY(-1px);
}

.cta:disabled {
  border-color: #d3dfd0;
  background: #f3f6f0;
  color: #9aa79a;
}

.moneyPanel {
  overflow: visible;
}

.networkControl {
  position: relative;
  z-index: 6;
  margin-top: 9px;
}

.networkButton {
  display: inline-flex;
  min-height: 25px;
  align-items: center;
  gap: 7px;
  padding: 0;
  border: 0;
  background: transparent;
  color: #536253;
  text-align: left;
}

.networkButton:hover .networkCopy strong,
.networkButton:focus-visible .networkCopy strong {
  color: #4d7d0e;
}

.networkDot {
  display: grid;
  width: 18px;
  height: 18px;
  flex: 0 0 auto;
  place-items: center;
  overflow: hidden;
  border-radius: 50%;
  background: #eef2ea;
}

.networkDot img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.networkDot.currencyDot {
  background: #536253;
  color: #fff;
  font-size: 12px;
  font-weight: 800;
}

.networkDot.flagDot {
  background: transparent;
}

.networkDot.flagDot img {
  object-fit: cover;
}

.networkCopy strong {
  color: #344235;
  font-size: 12px;
  font-weight: 700;
  line-height: 1;
  transition: color 0.15s ease;
}

.networkChevron {
  color: #7b8878;
  flex: 0 0 auto;
}

.methodControls {
  position: relative;
  z-index: 4;
  display: flex;
  min-width: 0;
  flex: 0 0 auto;
  align-items: center;
  justify-content: flex-end;
  max-width: min(100%, 256px);
  gap: 0;
  overflow: hidden;
  border: 1px solid var(--exchange-asset-border);
  border-radius: 5px;
  background: var(--exchange-asset-bg);
  transition: border-color 0.15s ease;
}

.methodControls:has(> .methodTrigger:is(:hover, :focus-visible)) {
  border-color: #a4af9e;
  border-width: var(--border-highlight-width);
}

.methodControls .methodTrigger {
  width: auto;
  min-width: 0;
  min-height: 44px;
  max-width: none;
  flex: 1 1 auto;
  border: 0;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.methodControls .methodTrigger:hover {
  background: #edf2e8;
  box-shadow: none;
  transform: none;
}

.methodControls .methodTrigger:focus-visible,
.methodControls .networkButton:focus-visible {
  outline: var(--focus-ring-width) solid var(--color-focus);
  outline-offset: calc(-1 * var(--focus-ring-width));
}

.methodControls .networkControl {
  margin-top: 0;
  border-left: 1px solid var(--exchange-asset-border);
}

.methodControls .networkButton {
  min-height: 44px;
  padding: 6px 8px;
  border: 0;
  border-radius: 0;
  background: transparent;
}

.methodControls .networkButton:hover {
  background: #edf2e8;
}

@media (max-width: 980px) {
  .shell {
    padding-top: 30px;
  }

  .workspace {
    grid-template-columns: minmax(0, 1fr);
  }

  .card,
  .side {
    width: 100%;
  }

  .side {
    order: 2;
  }

  .card {
    min-height: 0;
  }
}

@media (max-width: 640px) {
  .shell {
    padding: 25px var(--page-gutter) 0;
  }

  .hero {
    margin-bottom: 20px;
  }

  .hero h1 {
    font-size: 44px;
  }

  .workspace {
    gap: 0;
    transform: none;
  }

  .routesReveal {
    margin-top: 14px;
  }

  .card {
    padding: 16px;
    border-radius: 13px;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }

  .settingsBackdrop {
    position: fixed;
    inset: 0;
    height: 100dvh;
    z-index: 1100;
    display: grid;
    place-items: end center;
    padding: 0;
    background: rgba(8, 11, 8, 0.52);
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
    animation: fadeIn 0.2s ease-out;
    touch-action: none;
  }

  .settingsMenu {
    position: relative;
    top: auto;
    right: auto;
    width: 100%;
    height: auto;
    max-height: calc(100dvh - 16px);
    box-sizing: border-box;
    pointer-events: auto;
    padding: 10px 16px calc(18px + env(safe-area-inset-bottom));
    border-radius: 14px 14px 0 0;
    animation: settingsSheetIn 0.24s cubic-bezier(0.22, 1, 0.36, 1);
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
    overscroll-behavior: contain;
    touch-action: auto;
    transform: translateY(var(--settings-sheet-drag, 0px));
    transition: transform 0.24s ease;
  }

  .exchangesMenu {
    width: 100%;
    max-width: none;
  }

  .exchangeModalOptions {
    grid-auto-flow: row;
    grid-template-rows: none;
    grid-auto-columns: auto;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .settingsMenu.settingsDragging {
    transition: none;
  }

  .settingsModalHeader {
    display: flex;
    min-height: 30px;
    justify-content: space-between;
  }

  .settingsSheetHandle {
    display: flex;
    width: 100%;
    height: 30px;
    align-items: center;
    justify-content: center;
    margin: 0;
    border-radius: 0;
    background: transparent;
    cursor: grab;
    touch-action: none;
    user-select: none;
  }

  .settingsSheetHandle:active {
    cursor: grabbing;
  }

  .settingsSheetHandle::after {
    width: 38px;
    height: 5px;
    border-radius: var(--radius-pill);
    background: var(--color-border-strong);
    content: "";
  }

  .settingsClose {
    display: none;
  }

  .moneyPanel {
    min-height: 116px;
    justify-content: flex-start;
    gap: 8px;
    padding-top: 13px;
    padding-bottom: 10px;
  }

  .panelCopy {
    flex: none;
    width: 100%;
  }

  .methodTrigger {
    width: 100%;
  }

  .methodControls {
    width: 100%;
    max-width: none;
  }

  .methodControls .methodTrigger {
    width: auto;
    flex: 1 1 auto;
  }

  .methodControls .networkButton {
    flex: 0 0 auto;
  }
}

@keyframes settingsSheetIn {
  from { opacity: 0; transform: translateY(24px); }
  to { opacity: 1; transform: translateY(0); }
}

/* Dark theme: graphite surfaces with the existing lime route accents. */
:global(html[data-theme="dark"]) .card {
  border-color: var(--color-border-strong);
  background: var(--exchange-card-bg);
  box-shadow: none;
}

:global(html[data-theme="dark"]) .refreshButton:hover:not(:disabled),
:global(html[data-theme="dark"]) .exchangesButton:hover,
:global(html[data-theme="dark"]) .settingsButton:hover {
  background: #292929;
}

:global(html[data-theme="dark"]) .settingsMenu,
:global(html[data-theme="dark"]) .authBox {
  border-color: var(--color-border-strong);
  background: rgba(25, 25, 25, 0.98);
  box-shadow: none;
}

:global(html[data-theme="dark"]) .intentLabel {
  color: var(--color-text-soft);
}

:global(html[data-theme="dark"]) .moneyPanel,
:global(html[data-theme="dark"]) .moneyPanelSource,
:global(html[data-theme="dark"]) .moneyPanelTarget {
  border-color: #383838;
  background: #202020;
}

:global(html[data-theme="dark"]) .methodControls {
  border-color: #3b3b3b;
  background: #2a2a2a;
}

:global(html[data-theme="dark"]) .methodControls .networkControl {
  border-color: #3b3b3b;
}

:global(html[data-theme="dark"]) .methodControls:has(> .methodTrigger:is(:hover, :focus-visible)) {
  border-color: #626262;
  border-width: var(--border-highlight-width);
}

:global(html[data-theme="dark"]) .methodControls .methodTrigger:hover,
:global(html[data-theme="dark"]) .methodControls .networkButton:hover {
  background: #323232;
}

:global(html[data-theme="dark"]) .networkButton,
:global(html[data-theme="dark"]) .networkCopy strong {
  color: var(--color-text);
}

:global(html[data-theme="dark"]) .networkChevron {
  color: var(--color-text-faint);
}

:global(html[data-theme="dark"]) .cta:disabled {
  border-color: #383838;
  background: #262626;
  color: #707070;
}

:global(html[data-theme="dark"]) .textInput {
  border-color: var(--color-border-strong);
  background: #151515;
}


@media (max-width: 980px), (pointer: coarse) {
  .modeTabs button { min-height: 44px; }
  .refreshButton, .exchangesButton, .settingsButton { width: 44px; height: 44px; }
  .panelToggles { height: 56px; }
  .flowBridge { height: 44px; margin: 10px 0 4px; }
  .chartToggle, .mobileRoutesToggle { height: 44px; }
  .refreshOptions { grid-template-columns: repeat(3, minmax(44px, 1fr)); gap: 8px; }
  .sourceOptions { gap: 8px; }
  .sourceOption { min-height: 44px; }
  .searchBox input, .textInput { font-size: 16px; }
}
</style>
