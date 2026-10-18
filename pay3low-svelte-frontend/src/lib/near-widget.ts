import { createElement, useEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
import {
  WidgetConfigProvider, WidgetSwap, useTokenInputPair, useUnsafeSnapshot, useWalletConnection,
  type Chains, type WidgetConfig,
} from "@aurora-is-near/intents-swap-widget-standalone";
import "@aurora-is-near/intents-swap-widget/styles.css";
import type { RouteCandidate } from "./exchange";
import type { TutorialStep } from "./route-tutorial";
import { t } from "./i18n";

const chains: Record<string, Chains> = {
  ethereum: "eth", "bnb-smart-chain": "bsc", "polygon-pos": "pol", "avalanche-c": "avax",
  "arbitrum-one": "arb", optimism: "op", base: "base", gnosis: "gnosis", near: "near",
  solana: "sol", bitcoin: "btc", tron: "tron", ton: "ton", xrpl: "xrp", stellar: "stellar",
  dogecoin: "doge", litecoin: "ltc", "bitcoin-cash": "bch", zcash: "zec", sui: "sui",
  cardano: "cardano", berachain: "bera", monad: "monad", plasma: "plasma", scroll: "scroll",
  xlayer: "xlayer", robinhood: "hood", aurora: "aurora", starknet: "starknet",
};

export interface MountedNearWidget {
  updateTheme(theme: "light" | "dark"): void;
  destroy(): void;
}

export function mountNearWidget(container: HTMLElement, route: RouteCandidate, step: TutorialStep, networks: Record<string, string>, apiKey: string | undefined, theme: "light" | "dark"): MountedNearWidget {
  const chain = (name: string | undefined) => chains[Object.keys(networks).find(id => networks[id] === name) ?? name ?? ""];
  const sourceChain = chain(step.fromAsset?.split("@")[1] ?? step.network);
  const targetChain = chain(step.toAsset?.split("@")[1] ?? step.targetNetwork ?? step.network);
  const input = step.amount?.endsWith(` ${step.from}`) ? step.amount.slice(0, -step.from.length - 1) : step.amount;
  const amount = (step.inputAmount ?? input ?? route.acquired_asset_amount ?? route.source_amount ?? "").replace(/,/g, "");
  // The SDK filters raw catalog symbols before normalizing these display aliases.
  const allowedTokens = (symbol: string) => symbol === "NEAR" ? ["NEAR", "wNEAR"] : symbol === "USDT" ? ["USDT", "USDT0"] : [symbol];
  const config: Partial<WidgetConfig> = {
    apiKey, referral: "Pay3Flow", enableAccountAbstraction: false,
    allowSwapWithExternalWallet: true, lockSwapDirection: true, enableAutoTokensSwitching: false,
    defaultSourceToken: sourceChain ? { symbol: step.from, blockchain: sourceChain } : null,
    defaultTargetToken: targetChain ? { symbol: step.to, blockchain: targetChain } : null,
    allowedSourceChainsList: sourceChain ? [sourceChain] : undefined,
    allowedTargetChainsList: targetChain ? [targetChain] : undefined,
    allowedSourceTokensList: allowedTokens(step.from), allowedTargetTokensList: allowedTokens(step.to),
    chainsFilter: { source: { external: "all", intents: "none" }, target: { external: "all", intents: "none" } },
    themeParentElementSelector: "[data-testid=near-intents-live-widget]",
  };

  function WalletHeader() {
    const { isConnected, walletSignIn, walletSignOut } = useWalletConnection();
    return createElement("div", { className: "flex items-center justify-between w-full gap-sw-md text-sw-gray-50" },
      createElement("strong", null, "NEAR Intents"),
      createElement("button", { type: "button", className: "px-sw-lg py-sw-md rounded-sw-md bg-sw-gray-800 text-sw-gray-50 text-sw-label-md cursor-pointer", onClick: () => { if (isConnected) void walletSignOut?.(); else walletSignIn?.(); } }, t(isConnected ? "Disconnect" : "Connect wallet")));
  }

  function ConfiguredSwap() {
    const seeded = useRef(false);
    const { ctx } = useUnsafeSnapshot();
    const { onChangeAmount } = useTokenInputPair();
    useEffect(() => {
      if (seeded.current || !/^\d+(\.\d+)?$/.test(amount)) return;
      if (ctx.sourceToken?.blockchain !== sourceChain || ctx.targetToken?.blockchain !== targetChain) return;
      if (ctx.sourceToken?.symbol.toUpperCase() !== step.from.toUpperCase() || ctx.targetToken?.symbol.toUpperCase() !== step.to.toUpperCase()) return;
      seeded.current = true;
      onChangeAmount("source", amount);
    }, [ctx.sourceToken, ctx.targetToken, onChangeAmount]);
    return createElement(WidgetSwap, { HeaderComponent: createElement(WalletHeader) });
  }

  const root = createRoot(container);
  function render(colorScheme: "light" | "dark") {
    root.render(createElement(WidgetConfigProvider, {
      config, theme: { colorScheme, accentColor: "#b5f500", backgroundColor: "#191919", showContainer: true },
    }, createElement(ConfiguredSwap)));
  }
  render(theme);
  return { updateTheme: render, destroy: () => root.unmount() };
}
