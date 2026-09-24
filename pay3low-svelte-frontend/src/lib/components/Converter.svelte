<script lang="ts">
  import { afterUpdate, onMount, onDestroy } from "svelte";
  import { fetchCorridors, fetchP2pRoutes, fetchProviders, recordServiceOpen, setServiceVote, streamP2pRoutes, type ExchangeCorridor, type P2pRouteSearchResponse, type ProviderDefinition, type RouteCandidate, type ServiceLink, type ServiceStats, type ServiceVote } from "$lib/exchange";
  import { FALLBACK_NETWORK, fetchNetworks, type CryptoNetwork } from "$lib/networks";
  import { assetIcon, networkIcon, venueIcon } from "$lib/icons";
  import { CRYPTO_ASSETS, DIGITAL_ASSETS, PAYMENT_METHODS, paymentMethodFavicon, type PaymentMethod } from "$lib/payment-methods";
  import { locale, t } from "$lib/i18n";
  import SidePanel from "./SidePanel.svelte";

  type RefreshSeconds = 0 | 5 | 15 | 30 | 60;
  type PickerSide = "source" | "target" | null;
  const REFRESH_OPTIONS: RefreshSeconds[] = [0, 5, 15, 30, 60];
  type P2pSource = string;
  type P2pSourceOption = { id: P2pSource; label: string; iconUrl: string; searchable: boolean };
  const INTERMEDIARY_ASSETS = CRYPTO_ASSETS.map(([currency]) => currency);
  const BANK_METHODS = PAYMENT_METHODS.filter((method) => method.kind === "bank");
  const STORAGE = { amount: "pay3flow.exchange.amount", refresh: "pay3flow.exchange.refresh-seconds", sources: "pay3flow.exchange.p2p-sources", corridor: "pay3flow.exchange.corridor", sourceMethod: "pay3flow.exchange.source-method", targetMethod: "pay3flow.exchange.target-method", direction: "pay3flow.exchange.direction-reversed", assets: "pay3flow.exchange.intermediary-assets", anonymousId: "pay3flow.reputation.anonymous-id" };

  let corridors: ExchangeCorridor[] = [];
  let corridorId = "";
  let amount = "0";
  let routes: RouteCandidate[] = [];
  let routesFound = 0;
  let selected: RouteCandidate | null = null;
  let selectionPinnedByUser = false;
  let instructionsRoute: RouteCandidate | null = null;
  let sourceMethodId = "am-ameriabank";
  let targetMethodId = "ru-sberbank";
  let directionReversed = false;
  let methodPicker: PickerSide = null;
  let networks: CryptoNetwork[] = [FALLBACK_NETWORK];
  let sourceNetworkId = FALLBACK_NETWORK.id;
  let targetNetworkId = FALLBACK_NETWORK.id;
  let networkPicker: PickerSide = null;
  let settingsOpen = false;
  let refreshSeconds: RefreshSeconds = 15;
  let p2pSources: P2pSourceOption[] = [];
  let selectedSources: P2pSource[] = [];
  let selectedIntermediaryAssets: string[] = [];
  let searching = false;
  let awaitingFirstRoute = false;
  let lastUpdatedAt: number | null = null;
  let clock = Date.now();
  let error: string | null = null;
  let anonymousId = "";
  let settingsElement: HTMLDivElement;
  let settingsDialog: HTMLDivElement;
  let settingsDragging = false;
  let settingsDragStartY = 0;
  let settingsDragDistance = 0;
  let settingsWasOpen = false;
  let settingsScrollLocked = false;
  let previousOverflow = "";
  let previousOverscrollBehavior = "";
  let preferencesLoaded = false;
  let urlReady = false;
  let requestId = 0;
  let controller: AbortController | null = null;
  let debounceTimer: number | undefined;
  let refreshTimer: number | undefined;
  let clockTimer: number | undefined;
  let initialSearchTimer: number | undefined;
  let initialSearchReady = false;
  let paymentPickerComponent: typeof import("./PaymentMethodPicker.svelte").default | null = null;
  let networkPickerComponent: typeof import("./NetworkPicker.svelte").default | null = null;
  let routeInstructionsComponent: typeof import("./RouteInstructions.svelte").default | null = null;
  let searchingVenues: P2pSourceOption[] = [];
  let foundVenues: P2pSourceOption[] = [];
  $: activeLocale = $locale;

  function readSharedExchange() {
    const match = window.location.hash.match(/^#\/swap\/([^/?#]+)\/([^/?#]+)(?:\?([^#]*))?$/i);
    if (!match) return null;
    const params = new URLSearchParams(match[3] ?? "");
    const value = params.get("amount");
    return { sourceCurrency: decodeURIComponent(match[1]).toUpperCase(), targetCurrency: decodeURIComponent(match[2]).toUpperCase(), amount: value && /^[0-9.,\s]+$/.test(value) ? value : null };
  }
  function normalizeAmount(value: string) {
    const sanitized = value.normalize("NFKC").replace(/[\u00a0\u200b-\u200d\ufeff]/g, "").replace(/[бБ]/g, ",").replace(/[юЮ٫٬]/g, ".").replace(/[^0-9.,]/g, "");
    const separator = Math.max(sanitized.lastIndexOf("."), sanitized.lastIndexOf(","));
    if (separator < 0) return sanitized.replace(/^0+(?=\d)/, "") || "0";
    const integer = sanitized.slice(0, separator).replace(/[.,]/g, "").replace(/^0+(?=\d)/, "") || "0";
    return `${integer}${sanitized[separator]}${sanitized.slice(separator + 1).replace(/[.,]/g, "")}`;
  }
  const amountNumber = (value: string) => Number(normalizeAmount(value).replace(",", "."));
  const amountFromMinor = (minor?: number) => minor == null ? "0" : (minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2, useGrouping: false });
  const intermediaryIcon = (asset: string) => assetIcon(asset);
  const networkName = (id: string | null | undefined) => !id ? "internal" : networks.find((network) => network.id === id)?.name ?? id;
  const locationLabel = (country: string, currency: string) => { try { return `${new Intl.DisplayNames([activeLocale], { type: "region" }).of(country) ?? country} · ${currency}`; } catch { return `${country} · ${currency}`; } };

  function providerLabel(provider: ProviderDefinition) {
    const label = provider.name.replace(/\s+(buy|sell)$/i, "").trim();
    return label || provider.slug;
  }

  function providerSources(providers: ProviderDefinition[]): P2pSourceOption[] {
    const sources = new Map<string, P2pSourceOption>();
    for (const provider of providers) {
      const existing = sources.get(provider.slug);
      if (existing) {
        existing.searchable ||= provider.searchable;
      } else {
        sources.set(provider.slug, {
          id: provider.slug,
          label: providerLabel(provider),
          iconUrl: venueIcon(provider.slug),
          searchable: provider.searchable,
        });
      }
    }
    return [...sources.values()].sort((left, right) => left.label.localeCompare(right.label));
  }

  function mapRoutes(response: Awaited<ReturnType<typeof fetchP2pRoutes>>): RouteCandidate[] {
    const bestTarget = Number(response.routes[0]?.target_amount ?? 0);
    return response.routes.map((route, index) => {
      const entryOffer = route.entry_offer, exitOffer = route.exit_offer, targetAmount = Number(route.target_amount);
      return {
        route_id: route.route_id ?? (route.market_path ? `spot:${route.market_path.venue}:${route.market_path.source_pair}:${route.market_path.target_pair}` : `live:${entryOffer?.source ?? "direct"}:${entryOffer?.ad_id ?? "none"}:${exitOffer?.source ?? "direct"}:${exitOffer?.ad_id ?? "none"}`),
        status: "complete", source_amount_minor: Math.round(Number(route.source_amount) * 100), source_currency: route.source_fiat,
        source_method_icon_url: sourceMethod?.kind === "bank" ? paymentMethodFavicon(sourceMethod) ?? undefined : undefined,
        entry_asset: route.asset, entry_network: networkName(route.entry_network), source_network: route.source_network ? networkName(route.source_network) : undefined, target_network: route.target_network ? networkName(route.target_network) : undefined,
        target_amount_minor: Math.round(targetAmount * 100), target_currency: route.target_fiat,
        target_method_icon_url: targetMethod?.kind === "bank" ? paymentMethodFavicon(targetMethod) ?? undefined : undefined,
        route_kind: route.route_kind, bridge_currency: route.bridge_currency, market_path: route.market_path,
        spread_bps: bestTarget > 0 && Number.isFinite(targetAmount) ? Math.round((targetAmount / bestTarget - 1) * 10_000) : 0,
        is_current_best: index === 0, is_live_market: true, payment_methods_verified: route.payment_methods_verified,
        entry_offer_url: entryOffer?.source_url, entry_offer_is_exact: entryOffer?.source_url_is_exact, entry_offer_ad_id: entryOffer?.ad_id,
        exit_offer_url: exitOffer?.source_url, exit_offer_is_exact: exitOffer?.source_url_is_exact, exit_offer_ad_id: exitOffer?.ad_id,
        entry_offer_snapshot: entryOffer, exit_offer_snapshot: exitOffer, warnings: route.warnings,
        services: route.services, reputation: route.reputation, service_links: route.service_links,
        legs: route.market_path ? [{ kind: "entry" as const, from: route.source_fiat, to: route.bridge_currency ?? route.target_fiat, provider: route.market_path.venue, status: "found" as const }, ...(route.bridge_currency ? [{ kind: "exit" as const, from: route.bridge_currency, to: route.target_fiat, provider: route.market_path.venue, status: "found" as const }] : [])] : [...(entryOffer ? [{ kind: "entry" as const, from: route.source_fiat, to: route.bridge_currency ?? route.asset, provider: entryOffer.source, status: "found" as const }] : []), ...(exitOffer ? [{ kind: "exit" as const, from: route.bridge_currency ?? route.asset, to: route.target_fiat, provider: exitOffer.source, status: "found" as const }] : [])],
      };
    });
  }

  function anonymousBrowserId() {
    const saved = localStorage.getItem(STORAGE.anonymousId);
    if (saved && /^[0-9a-f-]{36}$/i.test(saved)) return saved;
    const created = crypto.randomUUID();
    localStorage.setItem(STORAGE.anonymousId, created);
    return created;
  }

  function applySearchResponse(response: P2pRouteSearchResponse) {
    const nextRoutes = mapRoutes(response);
    const knownVenues = new Set(foundVenues.map((venue) => venue.id));
    const newlyFound = nextRoutes
      .flatMap((route) => route.legs.map((leg) => leg.provider.toLowerCase()))
      .filter((venue, index, venues) => !knownVenues.has(venue) && venues.indexOf(venue) === index)
      .map((venue) => p2pSources.find((item) => item.id === venue) ?? { id: venue, label: venue.charAt(0).toUpperCase() + venue.slice(1), iconUrl: venueIcon(venue), searchable: true });
    if (newlyFound.length) foundVenues = [...foundVenues, ...newlyFound];
    routesFound = response.routes_found ?? nextRoutes.length;
    routes = nextRoutes;
    const bestRoute = routes.find((route) => route.is_current_best) ?? routes[0] ?? null;
    if (selectionPinnedByUser) {
      const pinnedRoute = routes.find((route) => route.route_id === selected?.route_id);
      if (pinnedRoute) selected = pinnedRoute;
      else { selectionPinnedByUser = false; selected = bestRoute; }
    } else selected = bestRoute;
    if (instructionsRoute) instructionsRoute = routes.find((route) => route.route_id === instructionsRoute?.route_id) ?? instructionsRoute;
  }

  function selectRoute(route: RouteCandidate) {
    selected = route;
    selectionPinnedByUser = true;
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

  $: corridor = corridors.find((item) => item.id === corridorId) ?? corridors[0];
  $: sourceCountry = corridor ? (directionReversed ? corridor.target_country : corridor.source_country) : "";
  $: sourceCurrency = corridor ? (directionReversed ? corridor.target_currency : corridor.source_currency) : "";
  $: targetCountry = corridor ? (directionReversed ? corridor.source_country : corridor.target_country) : "";
  $: targetCurrency = corridor ? (directionReversed ? corridor.source_currency : corridor.target_currency) : "";
  $: sourceMethods = [...BANK_METHODS.filter((method) => method.role === "sender" || method.role === "both"), ...DIGITAL_ASSETS];
  $: targetMethods = [...BANK_METHODS.filter((method) => method.role === "recipient" || method.role === "both"), ...DIGITAL_ASSETS];
  $: sourceMethod = sourceMethods.find((method) => method.id === sourceMethodId) ?? (sourceCountry ? sourceMethods[0] : null);
  $: targetMethod = targetMethods.find((method) => method.id === targetMethodId) ?? (targetCountry ? targetMethods[0] : null);
  $: selectedSourceCurrency = sourceMethod?.currency || sourceCurrency;
  $: selectedTargetCurrency = targetMethod?.currency || targetCurrency;
  $: sourceNetworks = sourceMethod?.kind === "wallet" ? networks.filter((network) => network.currencies.includes(sourceMethod!.currency)) : [];
  $: targetNetworks = targetMethod?.kind === "wallet" ? networks.filter((network) => network.currencies.includes(targetMethod!.currency)) : [];
  $: sourceNetwork = sourceNetworks.find((network) => network.id === sourceNetworkId) ?? sourceNetworks[0];
  $: targetNetwork = targetNetworks.find((network) => network.id === targetNetworkId) ?? targetNetworks[0];
  $: hasAmount = Number.isFinite(amountNumber(amount)) && amountNumber(amount) > 0;
  $: previewRoute = selected ?? routes.find((route) => route.status === "complete" && route.is_current_best) ?? routes.find((route) => route.status === "complete") ?? null;
  $: secondsUntilRefresh = refreshSeconds && lastUpdatedAt ? Math.max(0, refreshSeconds - Math.floor((clock - lastUpdatedAt) / 1000)) : null;
  $: refreshProgress = secondsUntilRefresh !== null && refreshSeconds ? ((refreshSeconds - secondsUntilRefresh) / refreshSeconds) * 100 : 0;
  $: searchingVenues = selectedSources.map((source) => p2pSources.find((item) => item.id === source) ?? { id: source, label: source, iconUrl: venueIcon(source), searchable: true });
  $: searchSignature = `${corridor?.id ?? ""}:${sourceMethod?.id ?? ""}:${sourceNetwork?.id ?? ""}:${targetMethod?.id ?? ""}:${targetNetwork?.id ?? ""}:${amount}:${selectedSources.join(",")}:${selectedIntermediaryAssets.join(",")}:${directionReversed}`;
  $: scheduleAutomaticSearch(searchSignature, preferencesLoaded, urlReady, hasAmount, initialSearchReady);
  $: manageRefresh(refreshSeconds, lastUpdatedAt, hasAmount);
  $: if (preferencesLoaded) persistPreferences(amount, refreshSeconds, selectedSources, selectedIntermediaryAssets, corridorId, sourceMethodId, targetMethodId, directionReversed);
  $: if (urlReady && corridor && selectedSourceCurrency && selectedTargetCurrency) updateHash(selectedSourceCurrency, selectedTargetCurrency, amount);

  function persistPreferences(value: string, refresh: RefreshSeconds, sources: P2pSource[], assets: string[], corridorValue: string, sourceMethodValue: string, targetMethodValue: string, reversed: boolean) {
    try {
      localStorage.setItem(STORAGE.amount, value); localStorage.setItem(STORAGE.refresh, String(refresh)); localStorage.setItem(STORAGE.sources, sources.join(",")); localStorage.setItem(STORAGE.assets, assets.join(",")); localStorage.setItem(STORAGE.corridor, corridorValue); localStorage.setItem(STORAGE.sourceMethod, sourceMethodValue); localStorage.setItem(STORAGE.targetMethod, targetMethodValue); localStorage.setItem(STORAGE.direction, String(reversed));
    } catch {}
  }
  function updateHash(source: string, target: string, value: string) {
    const params = new URLSearchParams(); if (value !== "0") params.set("amount", value);
    const query = params.toString();
    history.replaceState(null, "", `${location.pathname}${location.search}#/swap/${encodeURIComponent(source)}/${encodeURIComponent(target)}${query ? `?${query}` : ""}`);
  }
  function scheduleAutomaticSearch(_signature: string, loaded: boolean, ready: boolean, validAmount: boolean, initialReady: boolean) {
    if (debounceTimer) window.clearTimeout(debounceTimer);
    debounceTimer = undefined;
    if (loaded && ready && initialReady && corridor && sourceMethod && targetMethod && validAmount) debounceTimer = window.setTimeout(startSearch, 650);
  }
  function manageRefresh(seconds: RefreshSeconds, updatedAt: number | null, validAmount: boolean) {
    if (refreshTimer) window.clearInterval(refreshTimer);
    if (seconds && updatedAt && validAmount) refreshTimer = window.setInterval(startSearch, seconds * 1000);
  }
  function resetResults() {
    controller?.abort(); routes = []; routesFound = 0; selected = null; selectionPinnedByUser = false; instructionsRoute = null; lastUpdatedAt = null; searching = false; awaitingFirstRoute = false; foundVenues = []; error = null;
    if (refreshTimer) window.clearInterval(refreshTimer);
  }
  function updateAmount(value: string) { initialSearchReady = true; amount = normalizeAmount(value); resetResults(); }
  function swapDirection() {
    if (!corridor) return;
    initialSearchReady = true;
    const nextSource = targetMethod?.id ?? "", nextTarget = sourceMethod?.id ?? "";
    if (previewRoute?.target_amount_minor != null) amount = amountFromMinor(previewRoute.target_amount_minor);
    directionReversed = !directionReversed; sourceMethodId = nextSource; targetMethodId = nextTarget;
    sourceNetworkId = targetNetwork?.id ?? FALLBACK_NETWORK.id; targetNetworkId = sourceNetwork?.id ?? FALLBACK_NETWORK.id; resetResults();
  }
  function chooseSource(method: PaymentMethod, network?: CryptoNetwork) { initialSearchReady = true; sourceMethodId = method.id; if (network) sourceNetworkId = network.id; methodPicker = null; resetResults(); }
  function chooseTarget(method: PaymentMethod, network?: CryptoNetwork) { initialSearchReady = true; targetMethodId = method.id; if (network) targetNetworkId = network.id; methodPicker = null; resetResults(); }
  function selectNetwork(network: CryptoNetwork) { initialSearchReady = true; if (networkPicker === "source") sourceNetworkId = network.id; else targetNetworkId = network.id; networkPicker = null; resetResults(); }
  async function openMethodPicker(side: Exclude<PickerSide, null>) {
    methodPicker = side;
    paymentPickerComponent ??= (await import("./PaymentMethodPicker.svelte")).default;
  }
  async function openNetworkPicker(side: Exclude<PickerSide, null>) {
    networkPicker = side;
    networkPickerComponent ??= (await import("./NetworkPicker.svelte")).default;
  }
  async function openInstructions(route: RouteCandidate) {
    instructionsRoute = route;
    routeInstructionsComponent ??= (await import("./RouteInstructions.svelte")).default;
  }

  function runPrimaryAction() {
    if (previewRoute) {
      void openInstructions(previewRoute);
      return;
    }
    void startSearch();
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

  async function voteForService(service: ServiceStats, vote: ServiceVote) {
    try {
      replaceServiceStats(await setServiceVote(service.id, anonymousId, vote));
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Could not save your feedback";
    }
  }

  async function startSearch() {
    if (debounceTimer) window.clearTimeout(debounceTimer);
    debounceTimer = undefined;
    if (!corridor || !sourceMethod || !targetMethod) return;
    const value = amountNumber(amount); if (!Number.isFinite(value) || value <= 0) return resetResults();
    controller?.abort(); controller = new AbortController(); const signal = controller.signal; const currentRequest = ++requestId; searching = true; awaitingFirstRoute = true; routesFound = 0; foundVenues = []; error = null;
    try {
      const sourceWallet = sourceMethod.kind === "wallet", targetWallet = targetMethod.kind === "wallet";
      if ((sourceWallet && !sourceNetwork) || (targetWallet && !targetNetwork)) throw new Error("No compatible network is available for the selected cryptocurrency");
      if (sourceWallet && targetWallet && sourceMethod.currency === targetMethod.currency) {
        if (sourceNetwork?.id === targetNetwork?.id) throw new Error("Choose a different cryptocurrency or network for the destination");
        throw new Error(`Cross-network bridge routes are not available yet. No live bridge provider is configured for ${sourceMethod.currency}: ${sourceNetwork?.name} → ${targetNetwork?.name}.`);
      }
      const liveQuery = { sourceFiat: selectedSourceCurrency, targetFiat: selectedTargetCurrency, sourceAmount: value, intermediaryAssets: !sourceWallet && !targetWallet && selectedIntermediaryAssets.length ? selectedIntermediaryAssets : undefined, sourceNetwork: sourceWallet ? sourceNetwork?.id : undefined, targetNetwork: targetWallet ? targetNetwork?.id : undefined, sourcePaymentMethod: sourceWallet ? undefined : sourceMethod.p2pQuery, targetPaymentMethod: targetWallet ? undefined : targetMethod.p2pQuery, sources: selectedSources, allowCrossVenue: true, limit: 40 };
      let response: P2pRouteSearchResponse;
      try {
        response = await streamP2pRoutes(liveQuery, anonymousId, signal, (event) => {
          if (currentRequest !== requestId) return;
          if (event.type === "search_started") routesFound = event.routes_found;
          if (event.type === "routes_updated") {
            applySearchResponse(event);
            if (event.routes.length > 0) awaitingFirstRoute = false;
          }
        });
      } catch (streamError) {
        if (signal.aborted || currentRequest !== requestId) return;
        response = await fetchP2pRoutes({ ...liveQuery, signal, anonymousId });
      }
      if (currentRequest !== requestId) return;
      applySearchResponse(response); awaitingFirstRoute = false; lastUpdatedAt = Date.now(); clock = Date.now();
    } catch (cause) {
      if (signal.aborted || currentRequest !== requestId) return;
      error = cause instanceof Error ? cause.message : "Could not search live P2P markets";
    } finally { if (currentRequest === requestId) { searching = false; awaitingFirstRoute = false; } }
  }
  function toggleSource(source: P2pSource) { initialSearchReady = true; selectedSources = selectedSources.includes(source) ? (selectedSources.length === 1 ? selectedSources : selectedSources.filter((item) => item !== source)) : [...selectedSources, source]; resetResults(); }
  function toggleAsset(asset: string) { initialSearchReady = true; selectedIntermediaryAssets = selectedIntermediaryAssets.includes(asset) ? selectedIntermediaryAssets.filter((item) => item !== asset) : [...selectedIntermediaryAssets, asset]; resetResults(); }
  function onDocumentMouseDown(event: MouseEvent) { if (settingsOpen && settingsElement && !settingsElement.contains(event.target as Node)) settingsOpen = false; }
  function closeSettings() { settingsOpen = false; }
  function onSettingsKeyDown(event: KeyboardEvent) { if (event.key === "Escape") closeSettings(); }
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
    const shared = readSharedExchange();
    let savedSourceIds: string[] = [];
    try {
      anonymousId = anonymousBrowserId();
      amount = shared?.amount ?? localStorage.getItem(STORAGE.amount) ?? "0";
      corridorId = localStorage.getItem(STORAGE.corridor) ?? ""; sourceMethodId = localStorage.getItem(STORAGE.sourceMethod) ?? sourceMethodId; targetMethodId = localStorage.getItem(STORAGE.targetMethod) ?? targetMethodId;
      const savedDirection = localStorage.getItem(STORAGE.direction); if (savedDirection != null) directionReversed = savedDirection === "true";
      savedSourceIds = localStorage.getItem(STORAGE.sources)?.split(",").map((value) => value.trim()).filter(Boolean) ?? [];
      const savedAssets = localStorage.getItem(STORAGE.assets); if (savedAssets != null) selectedIntermediaryAssets = [...new Set(savedAssets.split(",").filter((asset) => INTERMEDIARY_ASSETS.includes(asset as (typeof INTERMEDIARY_ASSETS)[number])))];
      const savedRefresh = Number(localStorage.getItem(STORAGE.refresh)); if (REFRESH_OPTIONS.includes(savedRefresh as RefreshSeconds)) refreshSeconds = savedRefresh as RefreshSeconds;
    } catch {}
    preferencesLoaded = true;
    // Keep a saved route search off the initial critical path. User changes
    // still enable the normal debounced search immediately.
    initialSearchTimer = window.setTimeout(() => initialSearchReady = true, 1500);
    fetchNetworks().then((items) => { if (items.length) networks = items; }).catch(() => {});
    fetchProviders().then((providers) => {
      p2pSources = providerSources(providers);
      const catalog = new Set(p2pSources.map((source) => source.id));
      const live = p2pSources.filter((source) => source.searchable).map((source) => source.id);
      const restored = [...new Set(savedSourceIds.filter((source) => catalog.has(source)))];
      selectedSources = restored.length ? restored : live;
    }).catch((cause: Error) => error ??= cause.message);
    fetchCorridors().then((response) => {
      const savedDirection = localStorage.getItem(STORAGE.direction);
      const sharedCorridor = shared ? response.items.find((item) => (item.source_currency === shared.sourceCurrency && item.target_currency === shared.targetCurrency) || (item.source_currency === shared.targetCurrency && item.target_currency === shared.sourceCurrency)) : null;
      corridors = response.items; corridorId = corridorId || sharedCorridor?.id || response.items[0]?.id || "";
      if (shared && sharedCorridor?.source_currency === shared.targetCurrency && sharedCorridor.target_currency === shared.sourceCurrency) directionReversed = true; else if (savedDirection != null) directionReversed = savedDirection === "true";
      urlReady = true;
    }).catch((cause: Error) => error = cause.message);
    document.addEventListener("mousedown", onDocumentMouseDown);
    clockTimer = window.setInterval(() => clock = Date.now(), 1000);
  });
  afterUpdate(() => {
    if (settingsOpen === settingsWasOpen) return;
    settingsWasOpen = settingsOpen;
    if (settingsOpen) {
      settingsScrollLocked = window.matchMedia("(max-width: 640px)").matches;
      if (settingsScrollLocked) {
        previousOverflow = document.body.style.overflow;
        previousOverscrollBehavior = document.body.style.overscrollBehavior;
        document.body.style.overflow = "hidden";
        document.body.style.overscrollBehavior = "none";
        window.addEventListener("keydown", onSettingsKeyDown);
      }
    } else {
      if (settingsScrollLocked) {
        document.body.style.overflow = previousOverflow;
        document.body.style.overscrollBehavior = previousOverscrollBehavior;
        settingsScrollLocked = false;
      }
      window.removeEventListener("keydown", onSettingsKeyDown);
    }
  });
  onDestroy(() => {
    controller?.abort();
    if (debounceTimer) clearTimeout(debounceTimer);
    if (refreshTimer) clearInterval(refreshTimer);
    if (clockTimer) clearInterval(clockTimer);
    if (initialSearchTimer) clearTimeout(initialSearchTimer);
    if (typeof document !== "undefined") document.removeEventListener("mousedown", onDocumentMouseDown);
    if (typeof document !== "undefined" && settingsScrollLocked) {
      document.body.style.overflow = previousOverflow;
      document.body.style.overscrollBehavior = previousOverscrollBehavior;
    }
    if (typeof window !== "undefined") window.removeEventListener("keydown", onSettingsKeyDown);
  });
</script>

<section class="shell" id="transfer">
  <div class="hero"><h1>{t("Move money.", {}, activeLocale)} <span>{t("Keep more.", {}, activeLocale)}</span></h1><p>{t("Stop spending hours searching for an exchange.", {}, activeLocale)}</p></div>
  <div class="workspace">
    <div class="card">
      <div class="cardTop">
        <div class="modeTabs" aria-label={t("Exchange mode", {}, activeLocale)}><button type="button" class="modeActive">{t("Bridge", {}, activeLocale)}</button><button type="button" disabled>{t("History", {}, activeLocale)}</button></div>
        <div class="cardActions">
          <button type="button" class="refreshButton" on:click={startSearch} disabled={!hasAmount || searching} aria-label="Refresh routes now"><svg class:refreshSpin={awaitingFirstRoute} width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true"><path d="M16.2 7.1A6.8 6.8 0 1 0 16.7 12" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /><path d="M13.1 3.8h3.6v3.6" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" /></svg></button>
          <div class="settingsWrap" bind:this={settingsElement}>
            <button type="button" class="settingsButton" on:click={() => settingsOpen = !settingsOpen} aria-haspopup="dialog" aria-expanded={settingsOpen} aria-label="Route refresh settings"><svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true"><path d="M10 6.8a3.2 3.2 0 1 0 0 6.4 3.2 3.2 0 0 0 0-6.4Z" stroke="currentColor" stroke-width="1.6" /><path d="M16.2 11.3a6.5 6.5 0 0 0 0-2.6l1.5-1.1-1.8-3.1-1.8.8a6.7 6.7 0 0 0-2.2-1.3L11.7 2H8.3L8 4a6.7 6.7 0 0 0-2.2 1.3L4 4.5 2.2 7.6l1.5 1.1a6.5 6.5 0 0 0 0 2.6l-1.5 1.1L4 15.5l1.8-.8A6.7 6.7 0 0 0 8 16l.3 2h3.4l.3-2a6.7 6.7 0 0 0 2.2-1.3l1.8.8 1.8-3.1-1.6-1.1Z" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" /></svg></button>
            {#if settingsOpen}
              <div class="settingsBackdrop" on:mousedown={closeSettings} role="presentation">
                <div class:settingsDragging class="settingsMenu" bind:this={settingsDialog} role="dialog" aria-modal="true" aria-label="Refresh settings" tabindex="-1" on:mousedown|stopPropagation>
                <div class="settingsModalHeader"><span class="settingsSheetHandle" aria-hidden="true" on:pointerdown={startSettingsDrag} on:pointermove={moveSettingsDrag} on:pointerup={endSettingsDrag} on:pointercancel={endSettingsDrag}></span><button type="button" class="settingsClose" on:click={closeSettings} aria-label="Close route settings"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg></button></div>
                <div class="settingsHead"><div><strong>Auto-refresh</strong><span>Keep market routes current</span></div><span class={refreshSeconds ? "onBadge" : "offBadge"}>{refreshSeconds ? "On" : "Off"}</span></div>
                <div class="refreshOptions">{#each REFRESH_OPTIONS as seconds}<button type="button" aria-pressed={refreshSeconds === seconds} on:click={() => { refreshSeconds = seconds; settingsOpen = false; }}>{seconds === 0 ? "Off" : `${seconds}s`}</button>{/each}</div>
                <div class="sourceSettings"><span class="sourceSettingsLabel">Search exchanges</span><div class="sourceOptions exchangeOptions" aria-label="Exchanges to search">{#each p2pSources as source}{@const enabled = selectedSources.includes(source.id)}<button type="button" class:sourceOptionActive={enabled} class="sourceOption" aria-pressed={enabled} title={source.searchable ? `Search ${source.label}` : `Select ${source.label}`} on:click={() => toggleSource(source.id)}><span class="sourceOptionIcon" aria-hidden="true"><img src={source.iconUrl} alt="" width="18" height="18" loading="lazy" decoding="async" on:error={(event) => fallbackSourceIcon(event, source.id)} /></span>{source.label}</button>{/each}</div></div>
                <div class="sourceSettings"><div class="intermediarySettingsHead"><span class="sourceSettingsLabel">Cryptocurrency intermediary</span><small>{selectedIntermediaryAssets.length ? `${selectedIntermediaryAssets.length} selected` : "All available"}</small></div><div class="sourceOptions intermediaryOptions" aria-label="Cryptocurrency intermediaries">
                  <button type="button" class:sourceOptionActive={selectedIntermediaryAssets.length === 0} class="sourceOption" aria-pressed={selectedIntermediaryAssets.length === 0} on:click={() => { selectedIntermediaryAssets = []; resetResults(); }}>All available</button>
                  {#each INTERMEDIARY_ASSETS as asset}{@const enabled = selectedIntermediaryAssets.includes(asset)}<button type="button" class:sourceOptionActive={enabled} class="sourceOption" aria-pressed={enabled} on:click={() => toggleAsset(asset)}><span class="intermediaryAssetIcon" aria-hidden="true"><img src={intermediaryIcon(asset)} alt="" width="18" height="18" loading="lazy" decoding="async" on:error={fallbackAssetIcon} /></span>{asset}</button>{/each}
                </div></div>
                <p>Search also runs automatically 650ms after you change the amount, bank or intermediary.</p>
                </div>
              </div>
            {/if}
          </div>
        </div>
      </div>

      <div class="intentLabel"><span>Sell</span></div>
      <div class="moneyPanel moneyPanelSource">
        <div class="panelCopy"><label for="exchange-amount">You send</label><input id="exchange-amount" class="amountInput" type="text" inputmode="decimal" autocomplete="off" spellcheck="false" value={amount} on:focus={(event) => event.currentTarget.select()} on:input={(event) => updateAmount(event.currentTarget.value)} aria-label="Amount to send" /><span class="currencyHint">{selectedSourceCurrency || "AMD"} available via {sourceMethod?.kind === "wallet" ? "digital wallet" : "bank transfer"}</span></div>
        <div class="methodControls"><button type="button" class="methodTrigger" on:click={() => void openMethodPicker("source")} aria-label={`Select sending ${sourceMethod?.kind === "wallet" ? "asset" : "bank"}: ${sourceMethod?.name ?? "none"}`}>
          <span class="methodAvatar" style:background-color={paymentMethodFavicon(sourceMethod) ? "transparent" : sourceMethod?.color ?? "#171a17"} aria-hidden="true">{#if paymentMethodFavicon(sourceMethod)}<img src={paymentMethodFavicon(sourceMethod) ?? ""} alt="" width="48" height="48" loading="lazy" decoding="async" on:error={hideBrokenImage} /><span data-icon-fallback style="display:none">{sourceMethod?.initials ?? corridor?.source_country ?? "—"}</span>{:else}<span>{sourceMethod?.initials ?? corridor?.source_country ?? "—"}</span>{/if}</span>
          <span class="methodText"><strong>{sourceMethod?.name ?? "Select bank"}</strong><small>{sourceMethod?.kind === "wallet" ? `${sourceMethod.currency} · ${sourceNetwork?.name ?? "Loading networks…"}` : sourceMethod ? locationLabel(sourceMethod.country, sourceMethod.currency) : corridor ? locationLabel(sourceCountry, sourceCurrency) : "Unavailable"}</small></span><svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </button>{#if sourceMethod?.kind === "wallet" && sourceNetwork}<div class="networkControl"><button type="button" class="networkButton" on:click={() => void openNetworkPicker("source")} aria-haspopup="dialog"><span class="networkDot" aria-hidden="true"><img src={networkIcon(sourceNetwork.name)} alt="" width="18" height="18" loading="lazy" decoding="async" /></span><span class="networkCopy"><small>Network</small><strong>{sourceNetwork.name}</strong></span><span class="networkChevron" aria-hidden="true">⌄</span></button></div>{/if}</div>
      </div>
      <div class="flowBridge"><span class="bridgeLine" aria-hidden="true"></span><button type="button" class:bridgeIconReversed={directionReversed} class="bridgeIcon" on:click={swapDirection} aria-label="Swap sender and recipient" title="Swap sender and recipient"><svg width="18" height="18" viewBox="0 0 20 20" fill="none"><path d="M10 4v12m0 0-4-4m4 4 4-4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg></button></div>
      <div class="intentLabel intentLabelBuy"><span>Buy</span></div>
      <div class="moneyPanel moneyPanelTarget">
        <div class="panelCopy"><label for="exchange-output">Recipient gets</label><output id="exchange-output" class={previewRoute ? "amountOutput" : "amountOutputEmpty"}>{amountFromMinor(previewRoute?.target_amount_minor)}</output><span class="currencyHint">{targetMethod?.kind === "wallet" ? `${targetMethod.currency} available via digital wallet` : previewRoute ? `Estimated ${previewRoute.target_currency}` : "Live estimate appears here"}</span></div>
        <div class="methodControls"><button type="button" class="methodTrigger" on:click={() => void openMethodPicker("target")} aria-label={`Select recipient ${targetMethod?.kind === "wallet" ? "asset" : "bank"}: ${targetMethod?.name ?? "none"}`}>
          <span class="methodAvatar" style:background-color={paymentMethodFavicon(targetMethod) ? "transparent" : targetMethod?.color ?? "#171a17"} aria-hidden="true">{#if paymentMethodFavicon(targetMethod)}<img src={paymentMethodFavicon(targetMethod) ?? ""} alt="" width="48" height="48" loading="lazy" decoding="async" on:error={hideBrokenImage} /><span data-icon-fallback style="display:none">{targetMethod?.initials ?? corridor?.target_country ?? "—"}</span>{:else}<span>{targetMethod?.initials ?? corridor?.target_country ?? "—"}</span>{/if}</span>
          <span class="methodText"><strong>{targetMethod?.name ?? "Select bank"}</strong><small>{targetMethod?.kind === "wallet" ? `${targetMethod.currency} · ${targetNetwork?.name ?? "Loading networks…"}` : targetMethod ? locationLabel(targetMethod.country, targetMethod.currency) : corridor ? locationLabel(targetCountry, targetCurrency) : "Unavailable"}</small></span><svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </button>{#if targetMethod?.kind === "wallet" && targetNetwork}<div class="networkControl"><button type="button" class="networkButton" on:click={() => void openNetworkPicker("target")} aria-haspopup="dialog"><span class="networkDot" aria-hidden="true"><img src={networkIcon(targetNetwork.name)} alt="" width="18" height="18" loading="lazy" decoding="async" /></span><span class="networkCopy"><small>Network</small><strong>{targetNetwork.name}</strong></span><span class="networkChevron" aria-hidden="true">⌄</span></button></div>{/if}</div>
      </div>
      {#if refreshSeconds > 0}<div class="marketBar"><div class="marketState"><span class="refreshProgress" role="img" aria-label={secondsUntilRefresh === null ? "Auto-refresh is off" : `Refresh in ${secondsUntilRefresh} seconds`}><svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true"><circle class="refreshTrack" cx="9" cy="9" r="7" pathLength="100" /><circle class="refreshFill" cx="9" cy="9" r="7" pathLength="100" style:stroke-dashoffset={`${100 - refreshProgress}`} /></svg></span><div><span>{lastUpdatedAt ? `Updated ${Math.max(0, Math.floor((clock - lastUpdatedAt) / 1000))}s ago` : "Public P2P sources only · no order placement"}</span></div></div>{#if secondsUntilRefresh !== null}<span class="nextRefresh">{secondsUntilRefresh}s</span>{/if}</div>{/if}
      <button type="button" class="cta" disabled={!hasAmount || (!previewRoute && (searching || !corridor))} on:click={runPrimaryAction} data-testid="start-search" aria-label={previewRoute ? "Open swap instructions" : "Find routes"}>{#if previewRoute}Swap <span>↗</span>{:else if searching}<span class="spinner"></span> Finding routes{:else if hasAmount}Find routes <span>↗</span>{:else}Enter an amount to begin{/if}</button>
      {#if error}<div class="errorBox" role="alert">{error}</div>{/if}
    </div>
    <SidePanel {routes} {routesFound} sourceCurrency={selectedSourceCurrency} targetCurrency={selectedTargetCurrency} selectedRouteId={selected?.route_id ?? null} onSelect={selectRoute} onOpenInstructions={openInstructions} onVote={voteForService} {searching} {searchingVenues} {foundVenues} searched={lastUpdatedAt !== null} {hasAmount} />
  </div>
  {#if paymentPickerComponent}<svelte:component this={paymentPickerComponent} open={methodPicker === "source"} title="Choose where you pay from" role="sender" {networks} selected={sourceMethod} selectedNetwork={sourceNetwork} onClose={() => methodPicker = null} onSelect={chooseSource} /><svelte:component this={paymentPickerComponent} open={methodPicker === "target"} title="Choose where the recipient gets paid" role="recipient" {networks} selected={targetMethod} selectedNetwork={targetNetwork} onClose={() => methodPicker = null} onSelect={chooseTarget} />{/if}
  {#if networkPickerComponent}<svelte:component this={networkPickerComponent} open={networkPicker !== null} networks={networkPicker === "source" ? sourceNetworks : targetNetworks} selected={networkPicker === "source" ? sourceNetwork : targetNetwork} onClose={() => networkPicker = null} onSelect={selectNetwork} />{/if}
  {#if routeInstructionsComponent && instructionsRoute}<svelte:component this={routeInstructionsComponent} route={instructionsRoute} onOpenService={openService} onClose={() => instructionsRoute = null} />{/if}
</section>

<style>
.shell {
  position: relative;
  width: 100%;
  padding: 62px 24px 0;
  font-family: var(--font-sans);
}

.hero {
  width: min(900px, 100%);
  margin: 0 auto 42px;
  text-align: center;
  animation: heroIn 0.65s cubic-bezier(0.22, 1, 0.36, 1) both;
}

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
  font-size: clamp(50px, 6.8vw, 84px);
  font-weight: 650;
  letter-spacing: -0.07em;
  line-height: 0.96;
}

@media (min-width: 981px) {
  .hero h1 {
    white-space: nowrap;
  }
}

.hero h1 span {
  position: relative;
  z-index: 0;
  white-space: nowrap;
}

.hero h1 span::after {
  position: absolute;
  right: -0.05em;
  bottom: 0.02em;
  left: -0.04em;
  z-index: -1;
  height: 0.26em;
  border-radius: 3px;
  background: var(--color-accent);
  content: "";
  transform: rotate(-1deg);
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
  animation: workspaceIn 0.72s 0.08s cubic-bezier(0.22, 1, 0.36, 1) both;
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
  box-shadow: var(--shadow-card);
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
  font-size: 11px;
  font-weight: 750;
}

.modeTabs button:disabled {
  cursor: not-allowed;
  opacity: 0.48;
}

.modeTabs .modeActive {
  background: #fff;
  color: var(--color-text);
  box-shadow: 0 3px 10px rgba(20, 23, 19, 0.07);
}

.cardActions {
  gap: 5px;
}

.refreshButton,
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
.settingsButton:hover {
  border-color: var(--color-border);
  background: var(--color-panel);
}

.refreshButton:disabled {
  cursor: not-allowed;
  opacity: 0.36;
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
  box-shadow: var(--shadow-pop);
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

.settingsHead span,
.settingsMenu p {
  color: var(--color-text-faint);
  font-size: 10px;
  line-height: 1.5;
}

.onBadge,
.offBadge {
  padding: 5px 8px;
  border-radius: var(--radius-pill);
  font-size: 9px !important;
  font-weight: 800;
  letter-spacing: 0.07em;
  text-transform: uppercase;
}

.onBadge {
  background: rgba(45, 142, 69, 0.11);
  color: var(--color-good) !important;
}

.offBadge {
  background: var(--color-panel);
}

.refreshOptions {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 5px;
  margin: 17px 0 13px;
}

.refreshOptions button {
  height: 35px;
  border-radius: 10px;
  background: var(--color-panel);
  color: var(--color-text-soft);
  font-size: 10px;
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
  color: var(--color-text-soft);
  font-size: 10px;
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
  font-size: 9px;
  white-space: nowrap;
}

.sourceOptions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
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
  font-size: 10px;
  font-weight: 800;
  transition: border-color 0.15s ease, background 0.15s ease, color 0.15s ease, box-shadow 0.15s ease;
}

.sourceOption:hover {
  border-color: var(--color-accent);
}

.sourceOptionActive {
  border-color: var(--color-accent);
  background: var(--color-accent);
  color: #171717;
  box-shadow: 0 5px 13px rgba(185, 242, 39, 0.2);
}

.intentLabel {
  justify-content: space-between;
  padding: 5px 5px 3px;
  color: var(--color-text-faint);
  font-size: 10px;
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
  box-shadow: 0 10px 26px rgba(25, 28, 24, 0.05);
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
  font-size: 11px;
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
  color: #b7b8b2;
}

.currencyHint {
  color: var(--color-text-faint);
  font-size: 10px;
  font-weight: 650;
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
  box-shadow: 0 5px 15px rgba(21, 24, 20, 0.05);
  /* Avoid repaint-heavy backdrop sampling while the browser is zooming. */
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  transition: border-color 0.15s ease, transform 0.15s ease, box-shadow 0.15s ease;
}

.methodTrigger:hover {
  border-color: rgba(19, 22, 19, 0.2);
  box-shadow: 0 8px 20px rgba(21, 24, 20, 0.08);
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
  box-shadow: 0 5px 13px rgba(20, 23, 19, 0.13);
  font-size: 11px;
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
  flex-direction: column;
  gap: 3px;
}

.methodText strong,
.methodText small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.methodText strong {
  font-size: 12px;
  font-weight: 800;
}

.methodText small {
  color: var(--color-text-faint);
  font-size: 9px;
  font-weight: 650;
}

.flowBridge {
  position: relative;
  z-index: 4;
  display: flex;
  height: 14px;
  align-items: center;
  justify-content: center;
}

.bridgeLine {
  position: absolute;
  top: 50%;
  right: 8%;
  left: 8%;
  height: 1px;
  background: linear-gradient(90deg, transparent, var(--color-border-strong), transparent);
  pointer-events: none;
}

.bridgeIcon {
  position: relative;
  z-index: 1;
  display: grid;
  width: 42px;
  height: 42px;
  place-items: center;
  border: 5px solid rgba(255, 255, 255, 0.95);
  border-radius: 14px;
  background: var(--color-primary);
  color: var(--color-accent);
  cursor: pointer;
  box-shadow: 0 7px 16px rgba(20, 23, 19, 0.18);
  transition: box-shadow 0.2s ease, transform 0.2s ease;
}

.bridgeIcon:hover {
  box-shadow: 0 10px 22px rgba(20, 23, 19, 0.24);
  transform: translateY(-1px) scale(1.04);
}

.bridgeIcon:active {
  transform: scale(0.96);
}

.bridgeIcon svg {
  transition: transform 0.32s cubic-bezier(0.22, 1, 0.36, 1);
}

.bridgeIconReversed svg {
  transform: rotate(180deg);
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
  font-size: 11px;
  font-weight: 800;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.marketState > div > span {
  overflow: hidden;
  color: var(--color-text-faint);
  font-size: 9px;
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
  font-size: 9px;
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
  box-shadow: 0 12px 24px rgba(14, 16, 14, 0.16);
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
  box-shadow: 0 16px 30px rgba(14, 16, 14, 0.2);
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
  font-size: 11px;
  font-weight: 800;
}

.selectionBox small {
  grid-column: 1 / -1;
  color: var(--color-text-faint);
  font-size: 9px;
}

.errorBox {
  padding: 11px 13px;
  border: 1px solid rgba(212, 61, 53, 0.13);
  border-radius: 14px;
  background: rgba(212, 61, 53, 0.06);
  color: var(--color-danger);
  font-size: 10px;
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
  box-shadow: 0 35px 110px rgba(0, 0, 0, 0.3);
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
  font-size: 9px;
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
  font-size: 10px;
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
  font-size: 9px;
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
  font-size: 10px;
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
  box-shadow: 0 0 0 4px var(--color-violet-soft);
}

.demoNote {
  gap: 7px;
  color: var(--color-text-faint);
  font-size: 10px;
}

.demoNote span {
  padding: 4px 7px;
  border-radius: var(--radius-pill);
  background: var(--color-accent-soft);
  color: #536d0f;
  font-size: 8px;
  font-weight: 850;
  text-transform: uppercase;
}

.secondaryButton {
  width: 100%;
  height: 56px;
  border-radius: 16px;
  background: var(--color-primary);
  color: #fff;
  box-shadow: 0 10px 24px rgba(14, 16, 14, 0.18);
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
  font-size: 8px;
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

@media (min-width: 981px) {
  .hero h1 {
    white-space: nowrap;
  }
}

.hero h1 span::after {
  height: 0.2em;
}

.hero > p {
  max-width: 560px;
  margin: 12px auto 0;
  color: var(--color-text-soft);
  font-size: 13px;
  line-height: 1.55;
}

.workspace {
  width: min(1220px, 100%);
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 22px;
}

.card {
  min-height: 0;
  gap: 12px;
  padding: 19px;
  border: 1px solid var(--color-border-strong);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 28px 70px rgba(41, 54, 38, 0.11);
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
  min-height: 27px;
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
  gap: 2px;
}

.refreshButton,
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
  font-size: 10px;
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
  min-height: 0;
  flex-wrap: wrap;
  align-items: flex-start;
  padding: 15px 15px 12px;
  border: 1px solid #d9ded4;
  border-radius: 7px;
  background: #f9faf6;
  box-shadow: none;
}

.moneyPanelSource,
.moneyPanelTarget {
  background: #f9faf6;
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

.amountInput,
.amountOutput,
.amountOutputEmpty {
  font-family: var(--font-mono);
  font-size: clamp(28px, 3.2vw, 38px);
  font-weight: 500;
  letter-spacing: -0.05em;
}

.currencyHint {
  font-family: var(--font-mono);
  color: #667064;
  font-size: 10px;
  font-weight: 500;
  letter-spacing: 0.01em;
  line-height: 1.35;
  opacity: 1;
  -webkit-font-smoothing: auto;
}

.methodTrigger {
  width: min(190px, 48%);
  min-height: 42px;
  gap: 7px;
  padding: 6px 8px 6px 6px;
  border: 1px solid #d7dcd3;
  border-radius: 5px;
  background: #e7ebe2;
  box-shadow: none;
}

.methodTrigger:hover {
  border-color: #a4af9e;
  background: #edf2e8;
  box-shadow: 0 6px 16px rgba(41, 54, 38, 0.08);
  transform: translateY(-1px);
}

.methodAvatar {
  width: 28px;
  height: 28px;
  border: 0;
  border-radius: 50%;
  box-shadow: none;
  font-size: 9px;
}

.methodAvatar img {
  inset: 4px;
  width: calc(100% - 8px);
  height: calc(100% - 8px);
  border-radius: 50%;
}

.methodText {
  gap: 2px;
}

.methodText strong {
  font-size: 11px;
}

.methodText small {
  font-family: var(--font-mono);
  font-size: 8px;
}

.flowBridge {
  height: 31px;
}

.bridgeLine {
  right: 0;
  left: 0;
  background: #d2ddd0;
}

.bridgeIcon {
  width: 27px;
  height: 27px;
  border: 1px solid #cad8c6;
  border-radius: 50%;
  background: #eff5eb;
  color: #689000;
  box-shadow: none;
}

.bridgeIcon svg {
  width: 14px;
  height: 14px;
}

.bridgeIcon:hover {
  box-shadow: 0 4px 12px rgba(53, 84, 43, 0.1);
  transform: translateY(-1px) scale(1.04);
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
  box-shadow: 0 8px 18px rgba(55, 77, 52, 0.11);
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

.networkCopy {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.networkCopy small {
  color: #8a9588;
  font-family: var(--font-mono);
  font-size: 8px;
  letter-spacing: 0.08em;
  line-height: 1;
  text-transform: uppercase;
}

.networkCopy strong {
  color: #344235;
  font-size: 11px;
  font-weight: 700;
  line-height: 1;
  transition: color 0.15s ease;
}

.networkChevron {
  margin-left: 1px;
  color: #7b8878;
  font-size: 14px;
  line-height: 1;
}

.methodControls {
  position: relative;
  z-index: 4;
  display: flex;
  min-width: 0;
  flex: 0 0 auto;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.methodControls .methodTrigger {
  width: auto;
  max-width: 190px;
}

.methodControls .networkControl {
  margin-top: 0;
}

.methodControls .networkButton {
  min-height: 42px;
  padding: 6px 8px;
  border: 1px solid #d7dcd3;
  border-radius: 5px;
  background: #eef2ea;
}

.methodControls .networkButton:hover {
  border-color: #a4af9e;
  background: #f3f7ef;
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
    padding: 25px 12px 0;
  }

  .hero {
    margin-bottom: 20px;
  }

  .hero h1 {
    font-size: 44px;
  }

  .workspace {
    gap: 14px;
    animation: none;
    transform: none;
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
    min-height: 178px;
  }

  .methodTrigger {
    width: 100%;
  }

  .methodControls {
    width: 100%;
  }

  .methodControls .methodTrigger {
    width: auto;
    flex: 1 1 auto;
  }

  .methodControls:not(:has(.networkControl)) .methodTrigger {
    width: 100%;
    max-width: none;
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
  background: rgba(25, 25, 25, 0.96);
  box-shadow: var(--shadow-card);
}

:global(html[data-theme="dark"]) .refreshButton:hover:not(:disabled),
:global(html[data-theme="dark"]) .settingsButton:hover {
  background: #292929;
}

:global(html[data-theme="dark"]) .settingsMenu,
:global(html[data-theme="dark"]) .authBox {
  border-color: var(--color-border-strong);
  background: rgba(25, 25, 25, 0.98);
  box-shadow: var(--shadow-pop);
}

:global(html[data-theme="dark"]) .intentLabel,
:global(html[data-theme="dark"]) .currencyHint {
  color: var(--color-text-soft);
}

:global(html[data-theme="dark"]) .moneyPanel,
:global(html[data-theme="dark"]) .moneyPanelSource,
:global(html[data-theme="dark"]) .moneyPanelTarget {
  border-color: #383838;
  background: #202020;
}

:global(html[data-theme="dark"]) .methodTrigger,
:global(html[data-theme="dark"]) .methodControls .networkButton {
  border-color: #3b3b3b;
  background: #2a2a2a;
}

:global(html[data-theme="dark"]) .methodTrigger:hover,
:global(html[data-theme="dark"]) .methodControls .networkButton:hover {
  border-color: #626262;
  background: #323232;
}

:global(html[data-theme="dark"]) .bridgeLine {
  background: #404040;
}

:global(html[data-theme="dark"]) .bridgeIcon {
  border-color: #505050;
  background: #272727;
  color: var(--color-accent);
}

:global(html[data-theme="dark"]) .networkButton,
:global(html[data-theme="dark"]) .networkCopy strong {
  color: var(--color-text);
}

:global(html[data-theme="dark"]) .networkCopy small,
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

</style>
