import { get } from "svelte/store";
import { env } from "$env/dynamic/public";
import { wallets, connectWallet, restoreWallets } from "./wallet-session";
import { chainIdForNetwork, validateRecipient, type EvmConnection } from "./wallet-execution";
import { atomicToDecimal, decimalToAtomic } from "./wallet-amount";
import type { SwapInput, SwapService, SwapServiceState } from "./swap-service";

interface Token { chainId: number; symbol: string; address: string; decimals: number }
let catalog: Promise<Token[]> | undefined;
async function tokens(): Promise<Token[]> {
  catalog ??= Promise.all([
    fetch("https://files.cow.fi/tokens/CowSwap.json"),
    fetch("https://api.symbiosis.finance/crosschain/v2/tokens"),
  ]).then(async responses => {
    const lists = await Promise.all(responses.map(async response => {
      if (!response.ok) throw new Error("Could not load swap token metadata");
      const data = await response.json();
      return (Array.isArray(data) ? data : data.tokens) as Token[];
    }));
    return lists.flat();
  }).catch(error => { catalog = undefined; throw error; });
  return catalog;
}

export async function createCowSwapService(input: SwapInput, onState: (state: SwapServiceState) => void): Promise<SwapService> {
  const [fromSymbol, sourceNetwork] = input.fromAsset.split("@");
  const [toSymbol, targetNetwork] = input.toAsset.split("@");
  const chainId = chainIdForNetwork(sourceNetwork);
  const targetChainId = chainIdForNetwork(targetNetwork) ?? ({ bitcoin: 1000000000, solana: 1000000001 } as Record<string, number>)[targetNetwork];
  if (!chainId || !targetChainId) throw new Error("Unsupported CoW wallet network");
  const sdkModule = await import("@cowprotocol/cow-sdk");
  const { TradingSdk, OrderKind, ALL_CHAINS_MAP, EVM_NATIVE_CURRENCY_ADDRESS } = sdkModule;
  const { BridgingSdk, NearIntentsBridgeProvider } = await import("@cowprotocol/sdk-bridging");
  let recipient = "";
  let disposed = false;
  const storageKey = `pay3flow.cow-order.${input.routeId}`;
  async function orderStatus(orderId: string, bridge?: InstanceType<typeof BridgingSdk>): Promise<string> {
    if (chainId !== targetChainId) {
      const tracker = bridge ?? new BridgingSdk({ providers: [new NearIntentsBridgeProvider({ apiKey: env.PUBLIC_NEAR_INTENTS_API_KEY })] });
      const order = await tracker.getOrder({ chainId: chainId!, orderId });
      return order ? ({ executed: "completed", expired: "expired", refund: "refunded" } as Record<string, string>)[order.statusResult.status] ?? "submitted" : "submitted";
    }
    const order = await new sdkModule.OrderBookApi({ chainId }).getOrder(orderId);
    return ({ fulfilled: "completed", cancelled: "cancelled", expired: "expired" } as Record<string, string>)[order.status] ?? "submitted";
  }
  const emit = () => { if (!disposed) onState({ address: get(wallets).evm?.address, ready: !!get(wallets).evm && !!recipient }); };
  const unsubscribe = wallets.subscribe(emit);
  void restoreWallets();
  async function token(symbol: string, networkId: number, address?: string): Promise<Token> {
    const chain = ALL_CHAINS_MAP[networkId as keyof typeof ALL_CHAINS_MAP];
    if (chain && symbol.toUpperCase() === chain.nativeCurrency.symbol?.toUpperCase() && (!address || address.toLowerCase() === chain.nativeCurrency.address.toLowerCase())) {
      return { chainId: networkId, symbol, address: chain.nativeCurrency.address, decimals: chain.nativeCurrency.decimals };
    }
    const found = (await tokens()).find(token => token.chainId === networkId && token.symbol.toUpperCase() === symbol.toUpperCase() && (!address || token.address.toLowerCase() === address.toLowerCase()));
    if (!found) throw new Error(`Token metadata is unavailable for ${symbol}`);
    return found;
  }
  return {
    async connect() { await connectWallet(sourceNetwork); emit(); },
    setRecipient(address) { recipient = address.trim(); emit(); },
    async recover() {
      const raw = localStorage.getItem(storageKey);
      if (!raw) return null;
      const saved = JSON.parse(raw) as { input: SwapInput; reference: string; output: string; expiresAt: string; recipient: string };
      if (JSON.stringify(saved.input) !== JSON.stringify(input)) return null;
      return { recipient: saved.recipient, reference: saved.reference, prepared: {
        output: saved.output, expiresAt: saved.expiresAt,
        async submit() { return { reference: saved.reference }; },
        status: orderStatus,
      } };
    },
    async prepare() {
      const wallet = get(wallets).evm;
      if (!wallet) throw new Error("Connect the source wallet");
      // Reconnect also verifies the current chain before preparing the quote.
      const connected = await connectWallet(sourceNetwork) as EvmConnection;
      if (connected.address.toLowerCase() !== wallet.address.toLowerCase()) throw new Error("The wallet account changed. Review the swap again.");
      if (targetNetwork === "bitcoin") {
        const { isValidChainAddress } = await import("pay3flow-near-sdk");
        if (!isValidChainAddress("btc", recipient)) throw new Error("Invalid recipient address");
      } else await validateRecipient(targetNetwork, recipient);
      const [{ getPublicClient, getWalletClient, getAccount, waitForTransactionReceipt }, { ViemAdapter }, sell, buy] = await Promise.all([
        import("@wagmi/core"), import("@cowprotocol/sdk-viem-adapter"), token(fromSymbol, chainId, input.fromTokenAddress), token(toSymbol, targetChainId, input.toTokenAddress),
      ]);
      const config = connected.config as Parameters<typeof getPublicClient>[0];
      const publicClient = getPublicClient(config, { chainId });
      const walletClient = await getWalletClient(config, { chainId });
      if (!publicClient) throw new Error("Wallet client is unavailable");
      const adapter = new ViemAdapter({ provider: publicClient, walletClient });
      const sdk = new TradingSdk({ chainId, appCode: "Pay3Flow" }, {}, adapter);
      const target = recipient;
      const sellAmount = decimalToAtomic(input.amount, sell.decimals).toString();
      const trade = { kind: OrderKind.SELL, sellToken: sell.address, sellTokenDecimals: sell.decimals, buyToken: buy.address, buyTokenDecimals: buy.decimals, amount: sellAmount, receiver: target, slippageBps: 100 };
      const native = sell.address.toLowerCase() === EVM_NATIVE_CURRENCY_ADDRESS.toLowerCase();
      const bridge = new BridgingSdk({ tradingSdk: sdk, providers: [new NearIntentsBridgeProvider({ apiKey: env.PUBLIC_NEAR_INTENTS_API_KEY }, adapter)] }, adapter);
      const quote = chainId !== targetChainId ? await bridge.getQuote({
        kind: OrderKind.SELL, amount: BigInt(sellAmount),
        sellTokenChainId: chainId, sellTokenAddress: sell.address, sellTokenDecimals: sell.decimals,
        buyTokenChainId: targetChainId, buyTokenAddress: buy.address, buyTokenDecimals: buy.decimals,
        account: connected.address, owner: connected.address, bridgeRecipient: target,
        // CoW's EVM leg uses the owner; the bridge receives the final address.
        receiver: connected.address, appCode: "Pay3Flow", swapSlippageBps: 100, bridgeSlippageBps: 100,
      }) : native ? undefined : await sdk.getQuote(trade);
      const nativeQuote = native && chainId === targetChainId ? await sdk.getQuoteResults(trade) : undefined;
      const swap = quote && "swap" in quote ? quote.swap : quote && "quoteResults" in quote ? quote.quoteResults : nativeQuote!.result;
      const buyAmount = quote && "bridge" in quote ? quote.bridge.amountsAndCosts.afterFee.buyAmount : swap.amountsAndCosts.afterNetworkCosts.buyAmount;
      const output = atomicToDecimal(buyAmount.toString(), buy.decimals);
      const expiresAt = new Date(swap.orderToSign.validTo * 1000).toISOString();
      onState({ address: connected.address, output, ready: true });
      let reference = "";
      return {
        output, expiresAt,
        async submit() {
          if (reference) return { reference };
          const account = getAccount(config);
          if (disposed || recipient !== target || account.address?.toLowerCase() !== connected.address.toLowerCase() || account.chainId !== chainId) throw new Error("The wallet or recipient changed. Review a fresh quote.");
          if (Date.parse(expiresAt) <= Date.now() + 5_000) throw new Error("The quote expired. Review a fresh quote.");
          if (!native) {
            const allowance = await sdk.getCowProtocolAllowance({ tokenAddress: sell.address, owner: connected.address, chainId });
            if (allowance < BigInt(sellAmount)) {
              const hash = await sdk.approveCowProtocol({ tokenAddress: sell.address, amount: BigInt(sellAmount), chainId });
              const receipt = await waitForTransactionReceipt(config, { hash: hash as `0x${string}`, chainId });
              if (receipt.status !== "success") throw new Error("Token approval failed");
              return { reference: hash, approval: true };
            }
          }
          const result = nativeQuote ? await sdkModule.postSellNativeCurrencyOrder(
            nativeQuote.orderBookApi, nativeQuote.result.appDataInfo,
            sdkModule.swapParamsToLimitOrderParams(sdkModule.getTradeParametersAfterQuote({ quoteParameters: nativeQuote.result.tradeParameters, sellToken: sell.address }), nativeQuote.result.quoteResponse),
            undefined, nativeQuote.result.signer,
          ) : await quote!.postSwapOrderFromQuote();
          reference = result.orderId;
          try { localStorage.setItem(storageKey, JSON.stringify({ input, reference, output, expiresAt, recipient: target })); } catch { /* Optional recovery. */ }
          return { reference };
        },
        async status(orderId) {
          return orderStatus(orderId, bridge);
        },
      };
    },
    destroy() { disposed = true; unsubscribe(); },
  };
}
