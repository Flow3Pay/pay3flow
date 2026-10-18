import { createElement, useEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
import { env } from "$env/dynamic/public";
import {
  WidgetConfigProvider, useTokenInputPair, useUnsafeSnapshot, useWalletConnection,
  useStoreSideEffects, useMakeTransfer, useSwitchChain, useOneClickExecutionStatus,
  fireEvent, useConfig,
} from "pay3flow-near-sdk";
import type { Chains, WidgetConfig } from "@aurora-is-near/intents-swap-widget-standalone";
import { atomicToDecimal } from "./wallet-amount";
import type { PreparedSwap, SwapInput, SwapService, SwapServiceState } from "./swap-service";

const chains: Record<string, Chains> = {
  ethereum: "eth", "bnb-smart-chain": "bsc", "polygon-pos": "pol", "avalanche-c": "avax",
  "arbitrum-one": "arb", optimism: "op", base: "base", gnosis: "gnosis", near: "near",
  solana: "sol", bitcoin: "btc", tron: "tron", ton: "ton", xrpl: "xrp", stellar: "stellar",
  dogecoin: "doge", litecoin: "ltc", "bitcoin-cash": "bch", zcash: "zec", sui: "sui",
  cardano: "cardano", berachain: "bera", monad: "monad", plasma: "plasma", scroll: "scroll",
  xlayer: "xlayer", robinhood: "hood", aurora: "aurora", starknet: "starknet",
};

/** React is a service host only: this component never renders a swap form. */
export function createNearSwapService(input: SwapInput, onState: (state: SwapServiceState) => void): SwapService {
  const [fromSymbol, fromNetwork] = input.fromAsset.split("@");
  const [toSymbol, toNetwork] = input.toAsset.split("@");
  const sourceChain = chains[fromNetwork], targetChain = chains[toNetwork];
  if (!sourceChain || !targetChain) throw new Error("Unsupported NEAR Intents network");
  const allowedTokens = (symbol: string) => symbol === "NEAR" ? ["NEAR", "wNEAR"] : symbol === "USDT" ? ["USDT", "USDT0"] : [symbol];
  const matchesSymbol = (symbol: string, expected: string) => allowedTokens(expected).some(value => value.toUpperCase() === symbol.toUpperCase());
  const config: Partial<WidgetConfig> = {
    apiKey: env.PUBLIC_AURORA_INTENTS_API_KEY, referral: "Pay3Flow", enableAccountAbstraction: false,
    allowSwapWithExternalWallet: true, lockSwapDirection: true, enableAutoTokensSwitching: false,
    defaultSourceToken: { symbol: fromSymbol, blockchain: sourceChain },
    defaultTargetToken: { symbol: toSymbol, blockchain: targetChain },
    allowedSourceChainsList: [sourceChain], allowedTargetChainsList: [targetChain],
    allowedSourceTokensList: allowedTokens(fromSymbol), allowedTargetTokensList: allowedTokens(toSymbol),
    chainsFilter: { source: { external: "all", intents: "none" }, target: { external: "all", intents: "none" } },
  };
  let actions: { connect(): void; prepare(): Promise<PreparedSwap> } | undefined;
  let disposed = false;
  let currentIdentity = "";
  let currentQuote: unknown;
  let lastStatus = "";
  const storageKey = `pay3flow.near-swap.${input.routeId}`;
  type Saved = { input: SwapInput; reference: string; recipient: string; output: string; expiresAt: string; depositAddress: string; depositMemo?: string; externalDeposit: boolean };
  let saved: Saved | undefined;
  try {
    const value = JSON.parse(localStorage.getItem(storageKey) ?? "null") as Saved | null;
    if (value && JSON.stringify(value.input) === JSON.stringify(input)) saved = value;
  } catch { /* Optional recovery. */ }
  function ServiceHost() {
    const seeded = useRef(false);
    const { ctx } = useUnsafeSnapshot();
    const { onChangeAmount } = useTokenInputPair();
    const { walletSignIn } = useWalletConnection();
    const { providers, plugins, alchemyApiKey } = useConfig();
    const { make: transfer } = useMakeTransfer({ providers, plugins });
    const { isSwitchingChainRequired, switchChain } = useSwitchChain({ providers });
    const status = useOneClickExecutionStatus({
      depositAddress: saved?.depositAddress ?? ctx.quote?.depositAddress,
      depositMemo: saved?.depositMemo ?? (ctx.quote && "depositMemo" in ctx.quote ? ctx.quote.depositMemo : undefined),
      disabled: !saved?.depositAddress && !ctx.quote?.depositAddress,
    });
    useStoreSideEffects({ listenTo: [
      "syncConfig", "updateBalances", "checkWalletConnection", "setSourceTokenBalance",
      ["setDefaultSelectedTokens", { skipIntents: false }],
      ["makeQuote", { type: "exact_in", refetchQuoteInterval: ctx.isDepositFromExternalWallet ? 0 : 15_000 }],
      ["setBalancesUsingAlchemyExt", { alchemyApiKey }],
    ] });
    useEffect(() => { fireEvent("reset", { clearWalletAddress: true }); }, []);
    useEffect(() => {
      if (seeded.current || ctx.sourceToken?.blockchain !== sourceChain || ctx.targetToken?.blockchain !== targetChain) return;
      if (!matchesSymbol(ctx.sourceToken.symbol, fromSymbol) || !matchesSymbol(ctx.targetToken.symbol, toSymbol)) return;
      seeded.current = true;
      onChangeAmount("source", input.amount);
    }, [ctx.sourceToken, ctx.targetToken, onChangeAmount]);
    useEffect(() => {
      if (disposed) return;
      const output = ctx.quote && "amountOutFormatted" in ctx.quote ? ctx.quote.amountOutFormatted
        : ctx.targetToken && ctx.targetTokenAmount ? atomicToDecimal(ctx.targetTokenAmount, ctx.targetToken.decimals) : undefined;
      currentIdentity = `${ctx.walletAddress ?? ""}:${ctx.sendAddress ?? ""}:${ctx.refundToAddress ?? ""}`;
      currentQuote = ctx.quote;
      lastStatus = status.data?.status ?? "";
      if (saved) { onState({ output: saved.output, status: lastStatus }); return; }
      const ready = seeded.current && !ctx.error && !ctx.areInputsValidating && ctx.quoteStatus === "success" && !!ctx.quote && !ctx.quote.dry;
      onState({
        address: ctx.walletAddress, output, ready, loading: ctx.quoteStatus === "pending" || ctx.areInputsValidating,
        error: ctx.error?.code.replaceAll("_", " ").toLowerCase(),
        depositAddress: ctx.isDepositFromExternalWallet ? ctx.quote?.depositAddress : undefined,
        depositMemo: ctx.isDepositFromExternalWallet && ctx.quote && "depositMemo" in ctx.quote ? ctx.quote.depositMemo : undefined,
        status: lastStatus,
      });
      actions = {
        connect: () => walletSignIn?.(),
        prepare: async () => {
          if (!ready || !ctx.quote || !output) throw new Error("Wait for a valid swap quote");
          const quote = ctx.quote;
          const identity = currentIdentity;
          const expiresAt = quote.deadline ?? new Date(Date.now() + 30_000).toISOString();
          let submittedReference = "";
          return {
            output, expiresAt,
            submit: async () => {
              if (submittedReference) return { reference: submittedReference };
              if (disposed || currentIdentity !== identity || currentQuote !== quote) throw new Error("The swap changed. Review a fresh quote.");
              if (Date.parse(expiresAt) <= Date.now() + 5_000) throw new Error("The quote expired. Review a fresh quote.");
              if (ctx.isDepositFromExternalWallet) submittedReference = quote.depositAddress!;
              else {
                if (isSwitchingChainRequired && !await switchChain()) throw new Error("Switch the wallet to the source network");
                const result = await transfer();
                if (!result) throw new Error("The wallet did not submit the transfer");
                submittedReference = result.hash;
              }
              saved = { input, reference: submittedReference, recipient: ctx.sendAddress ?? ctx.walletAddress ?? "", output, expiresAt, depositAddress: quote.depositAddress!, depositMemo: "depositMemo" in quote ? quote.depositMemo : undefined, externalDeposit: ctx.isDepositFromExternalWallet };
              try { localStorage.setItem(storageKey, JSON.stringify(saved)); } catch { /* Optional recovery. */ }
              return { reference: submittedReference };
            },
            status: async () => ({ SUCCESS: "completed", FAILED: "failed", REFUNDED: "refunded" })[lastStatus] ?? "submitted",
          };
        },
      };
    }, [ctx, walletSignIn, transfer, switchChain, isSwitchingChainRequired, status.data]);
    return null;
  }
  const container = document.createElement("div");
  const root = createRoot(container);
  root.render(createElement(WidgetConfigProvider, { config }, createElement(ServiceHost)));
  return {
    async connect() { if (!actions) throw new Error("Swap service is loading"); actions.connect(); },
    setRecipient(address) { fireEvent("addressSet", address || null); },
    setExternalDeposit(refundAddress) {
      fireEvent("depositTypeSet", { isExternal: true });
      fireEvent("refundToAddressSet", refundAddress || null);
    },
    async prepare() { if (!actions) throw new Error("Swap service is loading"); return actions.prepare(); },
    async recover() {
      if (!saved) return null;
      const value = saved;
      return { reference: value.reference, recipient: value.recipient, depositAddress: value.externalDeposit ? value.depositAddress : undefined, depositMemo: value.depositMemo, prepared: {
        output: value.output, expiresAt: value.expiresAt,
        async submit() { return { reference: value.reference }; },
        async status() { return ({ SUCCESS: "completed", FAILED: "failed", REFUNDED: "refunded" } as Record<string, string>)[lastStatus] ?? "submitted"; },
      } };
    },
    destroy() { disposed = true; actions = undefined; root.unmount(); },
  };
}
