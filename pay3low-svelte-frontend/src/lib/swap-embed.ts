import { fallbackVenue } from "./provider-profile";
import type { RouteCandidate } from "./exchange";
import type { TutorialStep } from "./route-tutorial";

export function swapAppUrl(step: TutorialStep, providerUrl?: string | null): string | null {
  const fallback = fallbackVenue(step.provider.toLowerCase()).url;
  for (const candidate of [step.url, providerUrl, fallback]) {
    if (!candidate) continue;
    try {
      const url = new URL(candidate);
      if (!['https:', 'http:'].includes(url.protocol)) continue;
      if (fallback && /^(api|docs)\./.test(url.hostname)) continue;
      return url.href;
    } catch { /* Try the provider's public application. */ }
  }
  return null;
}

/** Public applications verified to permit embedding; native execution uses its own panel. */
export function canEmbedSwapApp(provider: string): boolean {
  return provider === "symbiosis";
}

interface SymbiosisChain { id: number; name: string }
interface SymbiosisToken { symbol: string; address: string; chainId: number }
const chainAliases: Record<string, string> = {
  "bnb-smart-chain": "BNB", "polygon-pos": "Polygon", "avalanche-c": "Avalanche",
  "manta-pacific": "Manta", sei: "Sei v2", bsquared: "B² Network", xrpl: "XRP Ledger",
};
const normalize = (name: string) => name.toLowerCase().replace(/\([^)]*\)/g, "").replace(/[^a-z0-9²]/g, "");

/** Use the provider's catalog to preselect token contracts, including non-EVM destinations. */
export async function symbiosisAppUrl(route: RouteCandidate, step: TutorialStep, networks: Record<string, string>, signal: AbortSignal): Promise<string> {
  const timeoutSignal = AbortSignal.any([signal, AbortSignal.timeout(10_000)]);
  const responses = await Promise.all(["v1/chains", "v2/tokens"].map(path => fetch(`https://api.symbiosis.finance/crosschain/${path}`, { signal: timeoutSignal })));
  if (responses.some(response => !response.ok)) throw new Error("Could not load the Symbiosis catalog");
  const [chains, tokens] = await Promise.all(responses.map(response => response.json())) as [SymbiosisChain[], SymbiosisToken[]];
  const url = new URL("https://app.symbiosis.finance/swap");
  function select(side: "In" | "Out", symbol: string, network: string | undefined) {
    const id = Object.keys(networks).find(id => networks[id] === network) ?? network ?? "";
    const name = chainAliases[id] ?? id;
    const chain = chains.find(chain => normalize(chain.name) === normalize(name));
    const token = chain && tokens.find(token => token.chainId === chain.id && token.symbol.toUpperCase() === symbol.toUpperCase());
    if (!chain || !token) throw new Error("Could not preselect the Symbiosis pair");
    url.searchParams.set(`chain${side}`, chain.name);
    url.searchParams.set(`token${side}`, token.address || token.symbol);
  }
  select("In", step.from, step.network);
  select("Out", step.to, step.targetNetwork ?? step.network);
  const input = step.amount?.endsWith(` ${step.from}`) ? step.amount.slice(0, -step.from.length - 1) : step.amount;
  const amount = step.inputAmount ?? input?.replace(/,/g, "") ?? route.acquired_asset_amount ?? route.source_amount;
  if (amount && /^\d+(\.\d+)?$/.test(amount)) url.searchParams.set("amountIn", amount);
  return url.href;
}
