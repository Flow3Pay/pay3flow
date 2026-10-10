import type { P2pOffer, ProviderGuidance, RouteCandidate, ServiceLink } from "./exchange";
import { buildWhitebirdFrames } from "./guides/whitebird/frames";
import { buildBinanceProfileFrames } from "./guides/binance/frames";
import { buildBestchangeFrames } from "./guides/bestchange/frames";
import { buildVenueSwapFrames, swapGuideVenues } from "./guides/exchangers/frames";
import { buildP2pProfileFrames } from "./guides/p2p/frames";
import { buildSpotFrames, spotGuideLinks } from "./guides/spot/frames";
import { buildCifraFrames, cifraGuideUrl, cifraTerminalUrl } from "./guides/cifra/frames";

export type TutorialKind = "buy" | "sell" | "swap" | "transfer";
export type TutorialFrameKind = "open" | "verify" | "review" | "act" | "receive";
export interface TutorialFrame { kind: TutorialFrameKind; title: string; text: string }
export interface TutorialStep {
  id: string;
  kind: TutorialKind;
  title: string;
  summary: string;
  provider: string;
  venue: string;
  from: string;
  to: string;
  amount?: string;
  output?: string;
  network?: string;
  targetNetwork?: string;
  transfer?: {
    from: { provider: string; venue: string };
    to: { provider: string; venue: string };
  };
  pair?: string;
  rate?: string;
  frames: TutorialFrame[];
  notes: string[];
  checkpoint: string;
  offer?: P2pOffer;
  direct?: boolean;
  guide?: ProviderGuidance;
  guideSteps: string[];
  url?: string | null;
  serviceLink?: ServiceLink;
  execution?: boolean;
}
type Copy = (key: string, params?: Record<string, string | number>) => string;

export function tutorialMoney(minor?: number, currency?: string, exact?: string): string {
  const value = exact ? Number(exact) : minor == null ? null : minor / 100;
  return value == null ? "—" : `${value.toLocaleString("en-US", { maximumFractionDigits: exact ? 8 : 2 })} ${currency ?? ""}`.trim();
}

export function spotTutorialUrl(venue: string, symbol: string, first: string, second: string): string | null {
  const normalized = symbol.replace(/[^a-z0-9]/gi, "").toUpperCase();
  const a = first.toUpperCase(), b = second.toUpperCase();
  const pair = normalized === `${a}${b}` ? [a, b] : normalized === `${b}${a}` ? [b, a] : null;
  const key = venue.toLowerCase();
  if (key === "cifra-broker") return "https://tradernet.by/authentication/signup";
  if (!pair) return ({ binance: "https://www.binance.com/en/trade", bybit: "https://www.bybit.com/trade/spot/", okx: "https://www.okx.com/trade-spot/", bitget: "https://www.bitget.com/spot/", mexc: "https://www.mexc.com/exchange/", whitebird: "https://whitebird.io/spot/BTC/USDT" } as Record<string, string>)[key] ?? null;
  const [base, quote] = pair;
  return ({ binance: `https://www.binance.com/en/trade/${base}_${quote}?type=spot`, bybit: `https://www.bybit.com/trade/spot/${base}/${quote}`, okx: `https://www.okx.com/trade-spot/${base.toLowerCase()}-${quote.toLowerCase()}`, bitget: `https://www.bitget.com/spot/${base}${quote}`, mexc: `https://www.mexc.com/exchange/${base}_${quote}`, whitebird: `https://whitebird.io/spot/${base}/${quote}` } as Record<string, string>)[key] ?? null;
}

/** Compose chapters from route operations; rendering never depends on a particular corridor. */
export function buildRouteTutorial(route: RouteCandidate, names: Record<string, string>, guidance: Record<string, ProviderGuidance>, networks: Record<string, string>, copy: Copy): TutorialStep[] {
  const venue = (slug: string) => names[slug.toLowerCase()] ?? slug;
  const network = (value?: string) => value && value.toLowerCase() !== "internal" ? networks[value.toLowerCase()] ?? value : undefined;
  const pathAsset = (value: string) => {
    const [asset, chain] = value.split("@", 2);
    return chain ? `${asset} ${copy("in {network}", { network: network(chain) ?? chain })}` : asset;
  };
  const frame = (kind: TutorialFrameKind, title: string, text: string): TutorialFrame => ({ kind, title: copy(title), text });
  const receive = copy("Wait until the new balance appears before doing the next step.");
  const price = copy("Check the current price, fee, and amount you should receive before pressing the exchange button.");
  const quote = copy("Never send money after the quote expires. Get a new quote first.");
  const steps: TutorialStep[] = [];
  const add = (data: Omit<TutorialStep, "guideSteps" | "guide" | "notes"> & { guide?: ProviderGuidance; guideSteps?: string[]; notes?: string[] }) => {
    const guide = data.guide ?? guidance[data.provider.toLowerCase()];
    const key = data.provider.toLowerCase();
    const spotGuide = data.kind === "swap" && data.pair ? spotGuideLinks[key] : undefined;
    const spotFrames = data.kind === "swap" && data.pair ? buildSpotFrames(data, copy) : null;
    const frames = spotFrames ?? (data.kind === "transfer" ? data.frames
      : key === "cifra-broker" ? buildCifraFrames(data, copy)
      : key === "bestchange" ? buildBestchangeFrames(data.from, data.to, copy)
      : key === "whitebird" ? buildWhitebirdFrames(copy)
      : swapGuideVenues.includes(key) ? buildVenueSwapFrames(data, copy) : data.frames);
    if (key === "bestchange" && data.kind !== "transfer") {
      data.checkpoint = copy("Once you open the exchanger website, this guide step is complete. Continue with the exchanger's instructions there.");
      data.summary = copy("Read the exchanger's BestChange reviews, check your exchange direction and open its website.");
    }
    if (key === "cifra-broker" && data.kind !== "transfer") {
      data.url = cifraTerminalUrl;
      data.summary = copy("Use Cifra Markets in Tradernet: prepare your balance, find the market and place the order for your route.");
      data.checkpoint = copy("Check execution and the balance");
      data.guideSteps = [];
    }
    const spotUrl = data.kind === "swap" && data.pair ? spotTutorialUrl(data.provider, data.pair, data.from, data.to) : null;
    const cifraGuide = key === "cifra-broker" && data.kind !== "transfer" ? { ...guide, description: "", steps: [], links: [{ label: copy("Cifra official video guides"), url: cifraGuideUrl }] } : undefined;
    steps.push({ ...data, url: cifraGuide ? cifraTerminalUrl : spotUrl ?? data.url, frames, notes: data.notes ?? [], guide: cifraGuide ?? (spotGuide ? { ...guide, description: "", steps: [], links: [key === "whitebird" && spotUrl ? { ...spotGuide, url: spotUrl } : spotGuide] } : guide), guideSteps: spotGuide ? [] : data.guideSteps ?? guide?.steps ?? [] });
  };
  const swapFrames = (name: string, from: string, to: string) => [
    frame("open", "Open the exchange", copy("Open {venue}", { venue: name })),
    frame("verify", "Check the exchange", copy("First check that the pair changes {from} into {to}.", { from, to })),
    frame("act", "Confirm on the platform", price),
    frame("receive", "Check your balance", receive),
  ];

  // Cycles are already ordered by the router and must not also render entry/exit snapshots.
  if (route.cycle_legs?.length) {
    route.cycle_legs.forEach((leg, index, legs) => {
      const from = leg.from_asset.split("@")[0], to = leg.to_asset.split("@")[0], name = venue(leg.provider);
      const notes = [quote];
      if (leg.market_pair) {
        if (index === 0) notes.push(copy("Deposit the source asset to this exchange using the selected network. Verify deposit availability, minimum amounts, and fees first."));
        if (index > 0 && legs[index - 1].provider !== leg.provider) notes.push(copy("Transfer the previous step's output to this exchange. Verify a shared withdrawal/deposit network, transfer fees, and the minimum amount before sending."));
        if (index === legs.length - 1) notes.push(copy("After the last trade, withdraw the original asset to your wallet on the selected network. Deduct the withdrawal fee when checking profit."));
      }
      add({ id: `cycle-${index}`, kind: "swap", provider: leg.provider, venue: name, from, to,
        title: copy("Convert {from} to {to}", { from: pathAsset(leg.from_asset), to: pathAsset(leg.to_asset) }),
        summary: copy("Route through {venue}", { venue: leg.description ? `${name} · ${leg.description}` : name }),
        amount: `${leg.input_amount} ${from}`, output: `${leg.output_amount} ${to}`, network: network(leg.from_asset.split("@")[1]), targetNetwork: network(leg.to_asset.split("@")[1]), pair: leg.market_pair ?? undefined,
        frames: swapFrames(name, pathAsset(leg.from_asset), pathAsset(leg.to_asset)), notes, checkpoint: receive,
        url: leg.source_url ?? (leg.market_pair ? spotTutorialUrl(leg.provider, leg.market_pair, from, to) : null), direct: true,
      });
    });
    return steps;
  }

  const crypto = route.route_kind === "crypto_to_crypto";
  const providerSwap = Boolean(route.route_provider && (route.entry_offer_snapshot || route.exit_offer_snapshot));
  const addProvider = () => {
    const provider = route.route_provider!, name = venue(provider);
    const from = route.entry_offer_snapshot?.asset ?? route.source_currency, to = route.exit_offer_snapshot?.asset ?? route.target_currency ?? route.entry_asset;
    add({ id: "provider", kind: "swap", provider, venue: name, from, to,
      title: providerSwap || crypto ? copy("Swap {from} for {to} via {venue}", { from, to, venue: name }) : copy("Route through {venue}", { venue: name }),
      summary: copy("Open the direct exchange on {venue}, check the final amount, and follow the provider's instructions.", { venue: name }),
      amount: !route.entry_offer_snapshot ? tutorialMoney(route.source_amount_minor, route.source_currency, route.source_amount) : undefined,
      output: !route.exit_offer_snapshot ? tutorialMoney(route.target_amount_minor, route.target_currency, route.target_amount) : undefined,
      network: network(route.entry_offer_snapshot?.network ?? route.source_network ?? route.entry_network),
      targetNetwork: network(route.exit_offer_snapshot?.network ?? route.target_network ?? route.entry_network),
      frames: [frame("open", "Open the exchange", copy("Open {venue}", { venue: name })),
        frame("verify", "Check the exchange", copy("Check which asset and network you send, and which asset and network you receive.")),
        frame("act", "Confirm on the platform", copy("Check the current rate, provider fee, quote expiry, and any address, memo, or tag requirement.")),
        frame("receive", "Check your balance", receive)],
      notes: [quote, ...(route.route_path?.length ? [route.route_path.map(pathAsset).join(" → ")] : [])], checkpoint: receive,
      url: route.route_provider_url, direct: true, execution: true,
    });
  };
  if (route.route_provider && !providerSwap) addProvider();

  if (crypto && route.market_path) {
    const market = route.market_path, name = venue(market.venue);
    const addMarket = (id: "market_source" | "market_target", from: string, to: string, pair: string, rate: string) => add({
      id, kind: "swap", provider: market.venue, venue: name, from, to, pair, rate,
      amount: id === "market_source" ? tutorialMoney(route.source_amount_minor, from, route.source_amount) : market.intermediary_amount ? `${market.intermediary_amount} ${from}` : undefined,
      output: id === "market_source" && route.bridge_currency ? market.intermediary_amount ? `${market.intermediary_amount} ${to}` : undefined : tutorialMoney(route.target_amount_minor, to, route.target_amount),
      network: network(route.source_network ?? route.entry_network), targetNetwork: network(route.target_network ?? route.entry_network),
      title: copy("Convert {from} to {to}", { from, to }), summary: copy("This is a normal exchange on {venue}. There is no separate person to message.", { venue: name }),
      frames: swapFrames(name, from, to), checkpoint: receive,
      serviceLink: route.service_links?.find((link) => link.kind === id), url: spotTutorialUrl(market.venue, pair, from, to),
      notes: [copy("Before withdrawing, check the receiving asset and the network one more time.")],
    });
    addMarket("market_source", route.source_currency, route.bridge_currency ?? route.target_currency ?? route.entry_asset, market.source_pair, market.source_rate);
    if (route.bridge_currency) addMarket("market_target", route.bridge_currency, route.target_currency ?? route.entry_asset, market.target_pair, market.target_rate);
  }

  const addOffer = (side: "entry" | "exit", offer: P2pOffer) => {
    const buying = side === "entry" ? !crypto : crypto;
    const provider = route.legs.find((leg) => leg.kind === side)?.provider ?? offer.source, name = venue(provider);
    const direct = offer.advertiser.user_type === "service" || offer.source.toLowerCase() === "whitebird";
    const directFiat = route.route_kind === "fiat_to_fiat" && side === "entry" && direct && !route.exit_offer_snapshot && !route.route_provider;
    const from = side === "entry" ? route.source_currency : crypto ? route.bridge_currency ?? route.entry_asset : offer.asset;
    const to = directFiat ? route.target_currency ?? offer.fiat : side === "entry" ? crypto ? route.bridge_currency ?? route.entry_asset : offer.asset : route.target_currency ?? offer.fiat;
    const checkpoint = direct ? copy(buying || directFiat ? "After the exchange, check that the new balance is available before continuing." : "After the sale, check that the money has arrived in your account before considering the exchange finished.") : buying ? copy("After the exchange, check that the new balance is available before continuing.") : copy("Release the asset only after you personally see the payment in your bank or payment account.");
    const guide = guidance[provider.toLowerCase()];
    const binanceP2p = provider.toLowerCase() === "binance" && !direct;
    const profileP2p = ["bybit", "mexc"].includes(provider.toLowerCase()) && !direct;
    const profileVenue = provider.toLowerCase() === "mexc" ? "MEXC" : "Bybit";
    add({ id: side, kind: buying ? "buy" : "sell", provider, venue: name, from, to, offer, direct, guide,
      title: directFiat ? copy("Transfer {from} to {to} via {venue}", { from: route.source_currency, to: route.target_currency ?? "", venue: name })
        : side === "entry" ? (crypto ? copy("Sell {asset} for {amount}", { asset: route.source_currency, amount: route.bridge_currency ?? route.entry_asset }) : copy("Buy {asset} for {amount}", { asset: offer.asset, amount: tutorialMoney(route.source_amount_minor, route.source_currency, route.source_amount) }))
        : crypto ? copy("Buy {asset} with {bridge}", { asset: route.target_currency ?? "", bridge: route.bridge_currency ?? route.entry_asset }) : copy("Sell {asset} for {amount}", { asset: offer.asset, amount: tutorialMoney(route.target_amount_minor, route.target_currency, route.target_amount) }),
      summary: binanceP2p ? copy("Open the Binance advertiser profile, inspect More details and choose the advertisement for your exchange.") : profileP2p ? copy("Open the advertiser's {venue} profile, read the reviews and choose the advertisement for your exchange.", { venue: profileVenue }) : direct ? copy("Open the direct exchange on {venue}, check the final amount, and follow the provider's instructions.", { venue: name }) : copy("Open the P2P listing on {venue}. Check the offer inside the platform before placing an order.", { venue: name }),
      amount: side === "entry" ? tutorialMoney(route.source_amount_minor, route.source_currency, route.source_amount) : undefined,
      output: side === "exit" || directFiat ? tutorialMoney(route.target_amount_minor, route.target_currency, route.target_amount) : undefined,
      network: network(offer.network ?? (side === "entry" ? route.source_network ?? route.entry_network : route.target_network ?? route.entry_network)),
      frames: binanceP2p ? buildBinanceProfileFrames(buying, offer.asset, offer.fiat, copy) : profileP2p ? buildP2pProfileFrames(profileVenue, buying, offer.asset, offer.fiat, copy) : [frame("open", direct ? "Open the exchange" : "Find the offer", direct ? copy("Check the currencies, amount, current rate, fee, and limits before continuing.") : copy("Before creating the order, compare the nickname and advertisement ID.")),
        frame("verify", "Check the exchange", copy("Check the current rate, order limits, and payment method on {venue}.", { venue: name })),
        frame("act", buying ? "Make the payment" : "Wait for the payment", direct ? copy("Sign in or complete verification on {venue}, if it asks you to, then follow the payment instructions shown there.", { venue: name }) : buying ? copy("Use only the payment details shown inside the order. After sending, mark the order as paid.") : checkpoint),
        frame("receive", "Check your balance", direct && !buying ? copy("After the sale, check that the money has arrived in your account before considering the exchange finished.") : checkpoint)],
      checkpoint, serviceLink: route.service_links?.find((link) => link.kind === side),
      url: offer.advertiser_profile_url ?? offer.source_url,
      guideSteps: [...(guide?.steps ?? []), ...(buying ? guide?.buy_steps ?? [] : guide?.sell_steps ?? [])],
      notes: [(side === "entry" ? route.source_bank_fee_percent : route.target_bank_fee_percent) != null ? copy("This bank fee is only an estimate. Check the final bank fee before sending.") : ""].filter(Boolean),
    });
  };
  if (route.entry_offer_snapshot) addOffer("entry", route.entry_offer_snapshot);
  if (providerSwap) addProvider();

  const entry = route.legs.find((leg) => leg.kind === "entry"), exit = route.legs.find((leg) => leg.kind === "exit");
  if (entry && exit && !route.route_provider && entry.provider !== exit.provider) {
    const chain = network(route.entry_network);
    const endpoint = (provider: string, offer?: P2pOffer) => ({
      provider: provider.toLowerCase() === "bestchange" ? "generic" : provider,
      venue: provider.toLowerCase() === "bestchange" ? offer?.advertiser.nickname ?? copy("Selected exchanger") : venue(provider),
    });
    const transfer = { from: endpoint(entry.provider, route.entry_offer_snapshot), to: endpoint(exit.provider, route.exit_offer_snapshot) };
    const name = transfer.to.venue;
    const checkpoint = copy("Wait until {venue} shows the deposit as received before continuing.", { venue: name });
    add({ id: "transfer", kind: "transfer", provider: exit.provider, venue: name, from: route.entry_asset, to: route.entry_asset, network: chain,
      transfer,
      title: copy("Transfer {asset} to {venue}", { asset: route.entry_asset, venue: name }), summary: copy("Send the purchased asset from {from} to the deposit address on {to} before opening the next order.", { from: transfer.from.venue, to: name }),
      frames: [frame("open", "Get the deposit address", chain ? copy("Copy the deposit address from {venue}. Choose the exact {network} network on both platforms.", { venue: name, network: chain }) : copy("First check that both platforms support the same asset and network. Then copy the deposit address from {venue}.", { venue: name })),
        frame("verify", "Check the network", copy("Check the complete address, memo or tag if required, and the withdrawal fee before confirming.")),
        frame("act", "Confirm the transfer", copy("Before sending, check the asset, the receiving asset, and the exact network.")),
        frame("receive", "Wait for the deposit", checkpoint)], checkpoint,
    });
  }
  if (route.exit_offer_snapshot) addOffer("exit", route.exit_offer_snapshot);
  return steps;
}
