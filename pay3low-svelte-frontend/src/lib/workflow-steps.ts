import type { RouteCandidate } from "./exchange";

export const FIAT_CURRENCIES = new Set(["AMD", "BYN", "KZT", "RUB", "UAH", "USD"]);
export type WorkflowStep = { currency: string; network?: string; provider?: string; iconUrl?: string; paymentMethod?: string; paymentMethodIcon?: string };

function pathStepProvider(route: RouteCandidate, index: number, lastIndex: number) {
  if (index === 0) return undefined;
  if (route.cycle_legs?.length) return route.cycle_legs.find((leg) => leg.to_asset === route.route_path?.[index])?.provider;
  if (route.route_kind === "fiat_to_fiat" || route.route_kind === "fiat_cycle" || route.route_kind === "crypto_cycle") {
    if (index === 1 && route.entry_offer_snapshot) return route.entry_offer_snapshot.source;
    if (index === lastIndex && route.exit_offer_snapshot) return route.exit_offer_snapshot.source;
    return route.route_provider ?? undefined;
  }
  if (route.route_kind === "fiat_to_crypto") {
    if (index === 1 && route.entry_offer_snapshot) return route.entry_offer_snapshot.source;
    return route.route_provider ?? route.market_path?.venue ?? undefined;
  }
  if (route.route_kind === "crypto_to_fiat") {
    if (index === lastIndex && route.exit_offer_snapshot) return route.exit_offer_snapshot.source;
    return route.route_provider ?? undefined;
  }
  return route.route_provider ?? undefined;
}

function currencySteps(route: RouteCandidate): WorkflowStep[] {
  const entry = route.legs.find((leg) => leg.kind === "entry");
  const exit = route.legs.find((leg) => leg.kind === "exit");
  const source = route.source_currency ?? "—";
  const target = route.target_currency ?? "—";
  if (route.route_path?.length) {
    return route.route_path.map((qualified, index) => {
      const [currency, network] = qualified.split("@", 2);
      return { currency, network, provider: pathStepProvider(route, index, route.route_path!.length - 1) };
    });
  }
  if (!entry && exit) return [{ currency: source, network: route.source_network, provider: exit.provider, iconUrl: route.source_method_icon_url }, { currency: target, iconUrl: route.target_method_icon_url }];
  if (entry && !exit) return [{ currency: source, network: route.source_network, iconUrl: route.source_method_icon_url }, { currency: target, network: route.target_network, provider: entry.provider, iconUrl: route.target_method_icon_url }];
  if (route.bridge_currency) return [{ currency: source, network: route.source_network, provider: entry?.provider, iconUrl: route.source_method_icon_url }, { currency: route.bridge_currency }, { currency: target, network: route.target_network, provider: exit?.provider, iconUrl: route.target_method_icon_url }];
  return [{ currency: source, iconUrl: route.source_method_icon_url }, { currency: route.entry_asset ?? "—", network: route.entry_network !== "internal" ? route.entry_network : undefined, provider: entry?.provider }, { currency: target, provider: exit?.provider, iconUrl: route.target_method_icon_url }];
}

export function routeWorkflowSteps(route: RouteCandidate): WorkflowStep[] {
  const steps = currencySteps(route);
  return steps.map((step, index) => {
    if (!FIAT_CURRENCIES.has(step.currency.toUpperCase())) return step;
    if (index === 0) return { ...step, paymentMethod: route.source_payment_method, paymentMethodIcon: route.source_method_icon_url };
    if (index === steps.length - 1) return { ...step, paymentMethod: route.target_payment_method, paymentMethodIcon: route.target_method_icon_url };
    return step;
  });
}
