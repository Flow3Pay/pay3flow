import type { P2pRoute, RouteLeg } from "./exchange";

type ProviderRoute = Pick<P2pRoute, "source_fiat" | "target_fiat" | "asset" | "route_provider" | "entry_offer" | "exit_offer">;

export function providerRouteLegs(route: ProviderRoute): RouteLeg[] {
  const entry = route.entry_offer, exit = route.exit_offer, provider = route.route_provider;
  if (!provider) return [];
  return [
    ...(entry
      ? [{ kind: "entry" as const, from: route.source_fiat, to: entry.asset, provider: entry.source, status: "found" as const }]
      : []),
    {
      kind: entry && exit ? "swap" : entry ? "exit" : "entry",
      from: entry?.asset ?? route.source_fiat,
      to: exit?.asset ?? route.target_fiat,
      provider,
      status: "found",
    },
    ...(exit
      ? [{ kind: "exit" as const, from: exit.asset, to: route.target_fiat, provider: exit.source, status: "found" as const }]
      : []),
  ];
}
