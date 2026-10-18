import { expect, test, type Page } from "@playwright/test";
import { encodeAbiParameters } from "viem";

const TRON = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";
const EVM = "0x0000000000000000000000000000000000000001";
const TX = "a".repeat(64);
const harnessPath = "/tests/fixtures/wallet-harness.ts";

async function wallets(page: Page) {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.addInitScript(({ tron, evm, tx }) => {
    if (window.top !== window) return;
    localStorage.setItem("pay3flow-locale", "en");
    const state = window as any;
    state.walletCalls = [];
    state.approvals = 0;
    state.transfers = 0;
    state.signatures = 0;
    state.connections = 0;
    state.rejectWallet = false;
    let allowance = "0";
    const listeners = new Map<string, Array<(value: unknown) => void>>();
    const web = {
      ready: JSON.parse(localStorage.getItem("pay3flow.wallet-families") ?? "[]").includes("tron"), defaultAddress: { base58: tron }, fullNode: { host: "https://api.trongrid.io" },
      isAddress: (value: string) => /^T[1-9A-HJ-NP-Za-km-z]{33}$/.test(value),
      address: { fromHex: () => tron, toHex: () => "41a614f803b6fd780986a42c78ec9c7f77e6ded13c" },
      contract: async () => ({
        balanceOf: () => ({ call: async () => state.tronBalance ?? "1000000000" }),
        allowance: () => ({ call: async () => allowance }),
        approve: (_spender: string, amount: string) => ({ send: async () => { if (state.rejectWallet) throw new Error("User cancelled signing"); state.approvals += 1; allowance = amount; return [tx, true]; } }),
        transfer: (recipient: string, amount: string) => ({ send: async () => { if (state.rejectWallet) throw new Error("User cancelled signing"); state.transfers += 1; state.walletCalls.push({ recipient, amount }); return tx; } }),
      }),
      transactionBuilder: {
        sendTrx: async (to: string, amount: string, from: string) => { state.walletCalls.push({ to, amount, from }); return { txID: tx }; },
        triggerSmartContract: async (to: string, selector: string, options: unknown, _parameters: unknown, from: string) => { state.walletCalls.push({ to, selector, options, from }); return { result: { result: true }, transaction: { txID: tx } }; },
      },
      trx: {
        getBalance: async () => state.tronBalance ?? "1000000000",
        sign: async (transaction: unknown) => { if (state.rejectWallet) throw new Error("User cancelled signing"); state.signatures += 1; return transaction; },
        sendRawTransaction: async () => { state.transfers += 1; return { result: !state.broadcastFailure, txid: tx }; },
      },
    };
    state.tron = {
      tronWeb: web,
      request: async ({ method }: { method: string }) => {
        if (state.rejectConnection) throw Object.assign(new Error("Connection rejected"), { code: 4001 });
        if (method === "eth_requestAccounts" || method === "tron_requestAccounts") { state.connections += 1; web.ready = true; return [tron]; }
        if (method === "eth_chainId") return "0x2b6653dc";
        if (method === "wallet_switchEthereumChain") { web.fullNode.host = "https://api.trongrid.io"; return null; }
        throw Object.assign(new Error("Unsupported method"), { code: 4200 });
      },
      on: (event: string, callback: (value: unknown) => void) => listeners.set(event, [...(listeners.get(event) ?? []), callback]),
      removeListener: (event: string, callback: unknown) => listeners.set(event, (listeners.get(event) ?? []).filter(item => item !== callback)),
    };
    state.changeTron = (address: string) => { web.defaultAddress.base58 = address; for (const listener of listeners.get("accountsChanged") ?? []) listener([address]); };
    const evmListeners = new Map<string, Array<(value: unknown) => void>>();
    state.chainSwitches = 0;
    state.ethereum = {
      on: (event: string, callback: (value: unknown) => void) => evmListeners.set(event, [...(evmListeners.get(event) ?? []), callback]),
      removeListener: (event: string, callback: unknown) => evmListeners.set(event, (evmListeners.get(event) ?? []).filter(item => item !== callback)),
      request: async ({ method, params }: { method: string; params: any[] }) => {
        if (method === "eth_accounts" || method === "eth_requestAccounts") return [evm];
        if (method === "eth_chainId") return state.evmChainId ?? "0x1";
        if (method === "wallet_switchEthereumChain") {
          state.evmChainId = params[0].chainId;
          state.chainSwitches += 1;
          for (const listener of evmListeners.get("chainChanged") ?? []) listener(state.evmChainId);
          return null;
        }
        if (method === "eth_getBalance") return "0xde0b6b3a7640000";
        if (method === "eth_call") return "0x" + (state.evmAllowance ?? 1000000000n).toString(16).padStart(64, "0");
        if (method === "eth_sendTransaction") { if (state.rejectWallet) throw Object.assign(new Error("User cancelled signing"), { code: 4001 }); state.walletCalls.push(params[0]); state.transfers += 1; return "0x" + tx; }
        if (method === "eth_estimateGas") return "0x5208";
        if (method === "eth_gasPrice" || method === "eth_maxPriorityFeePerGas") return "0x1";
        throw new Error(`Unmocked wallet method: ${method}`);
      },
    };
  }, { tron: TRON, evm: EVM, tx: TX });
  await page.route("**/api/**", route => route.fulfill({ contentType: "application/json", body: JSON.stringify([]) }));
  await page.goto("/");
  await expect(page.getByRole("button", { name: "Connect wallet", exact: true })).toBeVisible({ timeout: 20_000 });
  await page.waitForLoadState("networkidle");
}

function execution(provider = "near-intents", network = "tron", symbol = "USDT", destination = "tron") {
  return {
    id: "00000000-0000-4000-8000-000000000999", route_id: "wallet-test-route", provider,
    status: "awaiting_signature", from_asset: `${symbol}@${network}`, to_asset: `${symbol === "USDT" ? "TRX" : "USDT"}@${destination}`,
    input_amount: "12.345678", expected_output: "12", source_address: network === "tron" ? TRON : network === "near" ? "alice.near" : EVM,
    recipient: destination === "tron" ? TRON : destination === "near" ? "bob.near" : EVM,
    quote_expires_at: new Date(Date.now() + 120_000).toISOString(), updated_at: new Date().toISOString(),
    action: provider === "symbiosis" ? {
      kind: "symbiosis_transaction", chain_id: network === "tron" ? 728126428 : 1,
      source_token: "0xa614f803b6fd780986a42c78ec9c7f77e6ded13c", input_amount: "12345678", approval_spender: "0x49e1816a2cf475515e7c80c9f0f0e16ae499198b",
      transaction: { chainId: 728126428, from: "0xa614f803b6fd780986a42c78ec9c7f77e6ded13c", to: "0x0863786bbf4561f4a2a8be5a9ddf152afd8ae25c", data: "00".repeat(32), functionSelector: "swap(bytes)", feeLimit: 200000000, value: "0" },
    } : {
      kind: "near_deposit", network, asset: `${symbol}@${network}`, amount: "12.345678", deposit_address: network === "tron" ? TRON : network === "near" ? "deposit.near" : EVM,
      asset_id: "test-token", decimals: 6, token_contract: symbol === "TRX" || symbol === "ETH" ? undefined : network === "tron" ? TRON : network === "near" ? "usdt.tether-token.near" : EVM,
    },
  };
}

async function quoteApi(page: Page, value = execution(), failSubmission = false, expireFirstQuote = false) {
  let created = 0;
  let submissions = 0;
  let status = "awaiting_signature";
  await page.route("**/api/p2p/route-executions**", async route => {
    const request = route.request();
    if (request.method() === "POST" && !request.url().endsWith("/submissions")) {
      created += 1;
      const body = request.postDataJSON();
      expect(body.source_address).toBe(value.source_address);
      expect(body.recipient).toBe(value.recipient);
      return route.fulfill({ json: { ...value, status, quote_expires_at: expireFirstQuote && created === 1 ? new Date(Date.now() - 1_000).toISOString() : value.quote_expires_at } });
    }
    if (request.url().endsWith("/submissions")) {
      submissions += 1;
      if (failSubmission && submissions === 1) return route.fulfill({ status: 503, json: { error: "Temporary notification failure" } });
      expect(request.postDataJSON().reference).toBe(value.from_asset.endsWith("@tron") ? TX : "0x" + TX);
      status = "submitted";
    }
    return route.fulfill({ json: { ...value, status } });
  });
  return { counts: () => ({ created, submissions }), complete: () => { status = "completed"; } };
}

async function panel(page: Page, value = execution(), failSubmission = false, expireFirstQuote = false) {
  const mock = await quoteApi(page, value, failSubmission, expireFirstQuote);
  await page.evaluate(async ({ path, value }) => {
    const harness = await import(/* @vite-ignore */ path);
    const network = value.from_asset.split("@")[1];
    const family = network === "tron" || network === "near" ? network : "evm";
    await harness.mountPanel({ route_id: value.route_id, execution: { from_asset: value.from_asset, to_asset: value.to_asset, input_amount: value.input_amount, token: "signed-test-token", provider: value.provider } }, family);
  }, { path: harnessPath, value });
  return mock;
}

for (const mode of ["connect", "existing", "switch"]) test(`Symbiosis AVAX guide ${mode} and preserves submitted tracking on disconnect`, async ({ page }, testInfo) => {
  const preconnected = mode !== "connect";
  await wallets(page);
  await page.evaluate(mode => { (window as any).evmChainId = mode === "switch" ? "0x1" : "0xa86a"; }, mode);
  await page.route("https://api.avax.network/**", async route => {
    const balance = "0x" + (1000000000000000000n).toString(16).padStart(64, "0") as `0x${string}`;
    const aggregate = encodeAbiParameters([{ type: "tuple[]", components: [{ name: "success", type: "bool" }, { name: "returnData", type: "bytes" }] }], [[{ success: true, returnData: balance }]]);
    const respond = (request: { id: number; method: string; params?: Array<{ data?: string }> }) => ({ jsonrpc: "2.0", id: request.id, result: request.method === "eth_chainId" ? "0xa86a" : request.method === "eth_call" && request.params?.[0]?.data?.startsWith("0x82ad56cb") ? aggregate : balance });
    const requests = route.request().postDataJSON();
    await route.fulfill({ json: Array.isArray(requests) ? requests.map(respond) : respond(requests) });
  });
  const value = execution("symbiosis", "avalanche-c", "AVAX", "avalanche-c");
  value.input_amount = "0.25";
  value.expected_output = "10";
  value.action = {
    kind: "symbiosis_transaction", chain_id: 43114, source_token: "0x0000000000000000000000000000000000000000", input_amount: "250000000000000000",
    transaction: { chainId: 43114, to: "0x6F0f6393e45fE0E7215906B6f9cfeFf53EA139cf", data: "0x12345678", value: "250000000000000000" },
  } as any;
  const mock = await quoteApi(page, value);
  if (preconnected) await page.evaluate(async path => (await import(/* @vite-ignore */ path)).setEvmConnected(true), harnessPath);
  await page.evaluate(async ({ path, value }) => {
    const harness = await import(/* @vite-ignore */ path);
    await harness.mountGuide({
      route_id: value.route_id, route_kind: "crypto_to_crypto", route_provider: "symbiosis",
      source_currency: "AVAX", target_currency: "USDT", source_network: "avalanche-c", target_network: "avalanche-c", entry_asset: "AVAX",
      source_amount: "0.25", target_amount: "10", legs: [], route_path: [value.from_asset, value.to_asset],
      execution: { from_asset: value.from_asset, to_asset: value.to_asset, input_amount: value.input_amount, token: "signed-test-token", provider: value.provider },
    });
  }, { path: harnessPath, value });
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  await expect(guide.getByRole("heading", { level: 1 })).toHaveText("Swap AVAX for USDT via Symbiosis");
  if (!preconnected) {
    await expect(guide.getByTestId("wallet-swap-form")).toBeVisible();
    await expect(guide.getByLabel("Swap amount")).toHaveValue("0.25");
    await expect(guide.getByRole("region", { name: "Step instructions" })).toHaveCount(0);
    await expect(guide.getByRole("link", { name: "Open Symbiosis", exact: true })).toHaveCount(0);
    expect(mock.counts()).toEqual({ created: 0, submissions: 0 });
    await page.evaluate(async path => (await import(/* @vite-ignore */ path)).setEvmConnected(true), harnessPath);
  }
  await expect(guide.getByTestId("wallet-swap-form")).toBeVisible();
  await expect(guide.getByRole("region", { name: "Step instructions" })).toHaveCount(0);
  await expect(guide.getByLabel("Swap amount")).toHaveValue("0.25");
  await expect(guide.getByTestId("wallet-swap-form")).toContainText("Avalanche C-Chain");
  expect(mock.counts()).toEqual({ created: 0, submissions: 0 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
  const form = guide.getByTestId("wallet-swap-form");
  await expect(form).toHaveAttribute("data-provider", "symbiosis");
  await expect(form.getByLabel("Swap amount")).toHaveAttribute("readonly", "");
  await expect(form.locator("select, input[type=search]")).toHaveCount(0);
  const widget = guide.getByTestId("route-wallet-execution");
  await expect(widget).toContainText("Pay3Flow · SWAP");
  const lightBackground = await widget.evaluate(element => getComputedStyle(element).backgroundColor);
  await page.getByRole("button", { name: "Switch theme" }).click();
  await expect.poll(() => widget.evaluate(element => getComputedStyle(element).backgroundColor)).not.toBe(lightBackground);
  await page.getByRole("button", { name: "Switch theme" }).click();
  expect(await form.evaluate(element => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
  await guide.screenshot({ path: testInfo.outputPath("avax-swap.png") });
  await guide.getByRole("button", { name: "Swap AVAX for USDT", exact: true }).click();
  await expect(guide.getByTestId("route-wallet-execution")).toContainText("Submitted.");
  expect(mock.counts()).toEqual({ created: 1, submissions: 1 });
  expect(await page.evaluate(() => (window as any).chainSwitches)).toBe(mode === "switch" ? 1 : 0);
  const calls = await page.evaluate(() => (window as any).walletCalls);
  expect(BigInt(calls[0].value)).toBe(250000000000000000n);
  await page.evaluate(async path => (await import(/* @vite-ignore */ path)).setEvmConnected(false), harnessPath);
  await expect(guide.getByRole("region", { name: "Step instructions" })).toHaveCount(0);
  await expect(guide.getByTestId("wallet-swap-form")).toBeVisible();
  await expect(guide.getByLabel("Swap amount")).toHaveValue("0.25");
  await expect(guide.getByTestId("route-wallet-execution")).toContainText("Submitted.");
  await expect(guide.getByRole("button", { name: /Connect .* wallet/ })).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).transfers)).toBe(1);
});

for (const provider of ["cow-swap", "symbiosis", "near-intents"]) test(`${provider} uses the unified Pay3Flow widget without preparing a swap`, async ({ page }, testInfo) => {
  await wallets(page);
  await expect(page.locator(".introOverlay")).toHaveCount(0, { timeout: 6000 });
  const mock = await panel(page, execution(provider, "ethereum", "ETH", "ethereum"));
  const form = page.getByTestId("wallet-swap-form");
  await expect(form).toHaveAttribute("data-provider", provider);
  await expect(form.getByLabel("Swap amount")).toHaveValue("12.345678");
  await expect(form.getByLabel("Swap amount")).toHaveAttribute("readonly", "");
  await expect(form).toContainText("ETH");
  await expect(form).toContainText("USDT");
  await expect(form.locator("select, input[type=search]")).toHaveCount(0);
  await expect(form.getByRole("button", { name: "Swap ETH for USDT", exact: true })).toBeVisible();
  expect(await form.evaluate(element => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
  const widget = page.locator("#wallet-test-panel").getByTestId("route-wallet-execution");
  await expect(widget).toContainText("Pay3Flow · SWAP");
  await expect(form.locator(".assetCard")).toHaveCount(2);
  await expect(widget.locator("iframe")).toHaveCount(0);
  const lightBackground = await widget.evaluate(element => getComputedStyle(element).backgroundColor);
  await page.getByRole("button", { name: "Switch theme" }).click();
  await expect.poll(() => widget.evaluate(element => getComputedStyle(element).backgroundColor)).not.toBe(lightBackground);
  await form.screenshot({ path: testInfo.outputPath(`${provider}-wallet-dark.png`) });
  expect(mock.counts()).toEqual({ created: 0, submissions: 0 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});

for (const provider of ["cow-swap", "near-intents", "symbiosis"]) test(`${provider} guide prioritizes the native widget and preserves it across views`, async ({ page }) => {
  await wallets(page);
  const value = execution(provider, "ethereum", "ETH", "ethereum");
  const mock = await quoteApi(page, value);
  await page.evaluate(async ({ path, value }) => {
    const harness = await import(/* @vite-ignore */ path);
    await harness.mountGuide({
      route_id: value.route_id, route_kind: "crypto_to_crypto", route_provider: value.provider,
      source_currency: "ETH", target_currency: "USDT", source_network: "ethereum", target_network: "ethereum", entry_asset: "ETH",
      source_amount: value.input_amount, target_amount: "12", legs: [], route_path: [value.from_asset, value.to_asset],
      execution: { from_asset: value.from_asset, to_asset: value.to_asset, input_amount: value.input_amount, token: "signed-test-token", provider: value.provider },
    });
  }, { path: harnessPath, value });
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const widget = guide.getByTestId("route-wallet-execution");
  await expect(widget).toBeVisible();
  await expect(widget).toHaveAttribute("data-provider", provider);
  await expect(widget.getByLabel("Swap amount")).toHaveValue(value.input_amount);
  await expect(guide.getByTestId("cow-live-widget")).toHaveCount(0);
  await expect(guide.getByTestId("near-intents-live-widget")).toHaveCount(0);
  await widget.evaluate(element => element.setAttribute("data-session", "preserved"));
  await guide.getByRole("button", { name: "SWAP guide", exact: true }).click();
  await expect(widget).toBeHidden();
  await guide.getByRole("button", { name: "SWAP", exact: true }).click();
  await expect(widget).toBeVisible();
  await expect(widget).toHaveAttribute("data-session", "preserved");
  expect(mock.counts()).toEqual({ created: 0, submissions: 0 });
});

test("unsupported source networks retain the provider walkthrough and external link", async ({ page }) => {
  await wallets(page);
  const value = execution("symbiosis", "solana", "SOL", "ethereum");
  const mock = await quoteApi(page, value);
  await page.evaluate(async ({ path, value }) => {
    const harness = await import(/* @vite-ignore */ path);
    await harness.mountGuide({
      route_id: value.route_id, route_kind: "crypto_to_crypto", route_provider: "symbiosis",
      source_currency: "SOL", target_currency: "USDT", source_network: "solana", target_network: "ethereum", entry_asset: "SOL",
      source_amount: value.input_amount, target_amount: "12", legs: [], route_path: [value.from_asset, value.to_asset],
      execution: { from_asset: value.from_asset, to_asset: value.to_asset, input_amount: value.input_amount, token: "signed-test-token", provider: value.provider },
    }, "guide");
  }, { path: harnessPath, value });
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  await expect(guide.getByRole("region", { name: "Step instructions" })).toBeVisible();
  await expect(guide.getByRole("link", { name: "Open Symbiosis", exact: true })).toHaveAttribute("href", "https://app.symbiosis.finance/");
  await expect(guide.getByTestId("route-wallet-execution")).toHaveCount(0);
  expect(mock.counts()).toEqual({ created: 0, submissions: 0 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});

test("header connects and restores TRON without starting a swap, then disconnects", async ({ page }) => {
  await wallets(page);
  await page.getByRole("button", { name: "Connect wallet", exact: true }).click();
  const menu = await page.locator(".walletConnections").boundingBox();
  expect(menu).not.toBeNull();
  expect(menu!.x).toBeGreaterThanOrEqual(0);
  expect(menu!.x + menu!.width).toBeLessThanOrEqual(page.viewportSize()!.width);
  await page.getByRole("button", { name: "Connect TRON wallet", exact: true }).click();
  await expect(page.getByRole("button", { name: "Wallets (1)", exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
  await page.reload();
  await expect(page.getByRole("button", { name: "Wallets (1)", exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as any).connections)).toBe(0);
  await page.getByRole("button", { name: "Wallets (1)", exact: true }).click();
  await page.getByRole("button", { name: "Disconnect TRON wallet" }).click();
  await expect(page.getByRole("button", { name: "Connect TRON wallet", exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});

for (const symbol of ["USDT", "TRX"]) test(`TRON ${symbol} automatically deposits once and starts tracking`, async ({ page }) => {
  await wallets(page);
  const mock = await panel(page, execution("near-intents", "tron", symbol));
  await page.locator("#wallet-test-panel").getByRole("button", { name: "Connect tron wallet" }).click();
  await page.locator("#wallet-test-panel").getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(page.getByTestId("route-wallet-execution")).toContainText("Submitted.");
  expect(mock.counts()).toEqual({ created: 1, submissions: 1 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(1);
  const calls = await page.evaluate(() => (window as any).walletCalls);
  expect(calls[0].amount).toBe("12345678");
});

test("the unified form keeps amounts, output and completion in one place", async ({ page }) => {
  await wallets(page);
  const mock = await panel(page);
  const form = page.locator("#wallet-test-panel").getByTestId("wallet-swap-form");
  await expect(form).toBeVisible();
  await expect(form.getByLabel("Swap amount")).toHaveValue("12.345678");
  await expect(form.getByTestId("swap-output")).toHaveText("—");
  expect(mock.counts()).toEqual({ created: 0, submissions: 0 });
  await form.getByRole("button", { name: "Connect tron wallet" }).click();
  await form.getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(form).toContainText("Submitted.");
  await expect(form.getByTestId("swap-output")).toHaveText("12");
  await expect(form.getByRole("button", { name: "Another address", exact: true })).toBeDisabled();
  mock.complete();
  await expect(form).toContainText("Swap completed.", { timeout: 10_000 });
  await expect(form.getByRole("button", { name: "Another address", exact: true })).toBeDisabled();
  expect(mock.counts()).toEqual({ created: 1, submissions: 1 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(1);
});

test("Symbiosis TRON approval refreshes before automatically signing the swap", async ({ page }) => {
  await wallets(page);
  const mock = await panel(page, execution("symbiosis"));
  await page.locator("#wallet-test-panel").getByRole("button", { name: "Connect tron wallet" }).click();
  await page.locator("#wallet-test-panel").getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(page.getByTestId("route-wallet-execution")).toContainText("Submitted.");
  expect(mock.counts()).toEqual({ created: 2, submissions: 1 });
  expect(await page.evaluate(() => (window as any).approvals)).toBe(1);
  expect(await page.evaluate(() => (window as any).signatures)).toBe(1);
  const calls = await page.evaluate(() => (window as any).walletCalls);
  expect(calls[0].options).toEqual({ feeLimit: 200000000, callValue: "0", rawParameter: "00".repeat(32) });
});

test("backend notification retries preserve the broadcast hash and never sign twice", async ({ page }) => {
  await wallets(page);
  const mock = await panel(page, execution(), true);
  await page.locator("#wallet-test-panel").getByRole("button", { name: "Connect tron wallet" }).click();
  await page.locator("#wallet-test-panel").getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(page.getByRole("button", { name: "Retry tracking" })).toBeVisible();
  await page.getByRole("button", { name: "Retry tracking" }).click();
  await expect(page.getByTestId("route-wallet-execution")).toContainText("Submitted.");
  expect(mock.counts()).toEqual({ created: 1, submissions: 2 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(1);
});

test("wallet rejection stops automatic prompting", async ({ page }) => {
  await wallets(page);
  await panel(page);
  await page.evaluate(() => { (window as any).rejectWallet = true; });
  await page.locator("#wallet-test-panel").getByRole("button", { name: "Connect tron wallet" }).click();
  await page.locator("#wallet-test-panel").getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(page.locator("#wallet-test-panel").getByRole("alert")).toContainText("cancelled");
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});

test("a newly funded wallet automatically starts the prepared deposit", async ({ page }) => {
  await wallets(page);
  await page.evaluate(() => { (window as any).tronBalance = "0"; });
  await panel(page);
  await page.locator("#wallet-test-panel").getByRole("button", { name: "Connect tron wallet" }).click();
  await page.locator("#wallet-test-panel").getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(page.getByTestId("route-wallet-execution")).toContainText("Waiting until");
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
  await page.evaluate(() => { (window as any).tronBalance = "1000000000"; });
  await expect(page.getByTestId("route-wallet-execution")).toContainText("Submitted.", { timeout: 10_000 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(1);
});

test("account changes invalidate a funded preparation without prompting again", async ({ page }) => {
  await wallets(page);
  await page.evaluate(() => { (window as any).tronBalance = "0"; });
  const mock = await panel(page);
  await page.locator("#wallet-test-panel").getByRole("button", { name: "Connect tron wallet" }).click();
  await page.locator("#wallet-test-panel").getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(page.getByTestId("route-wallet-execution")).toContainText("Waiting until");
  await page.evaluate(() => {
    (window as any).changeTron("T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb");
    (window as any).tronBalance = "1000000000";
  });
  await expect(page.locator("#wallet-test-panel").getByTestId("swap-source-wallet")).toContainText("T9yD14");
  expect(mock.counts()).toEqual({ created: 1, submissions: 0 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});

test("an expired quote is refreshed before signing", async ({ page }) => {
  await wallets(page);
  const mock = await panel(page, execution(), false, true);
  await page.locator("#wallet-test-panel").getByRole("button", { name: "Connect tron wallet" }).click();
  await page.locator("#wallet-test-panel").getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(page.getByTestId("route-wallet-execution")).toContainText("Submitted.");
  expect(mock.counts()).toEqual({ created: 2, submissions: 1 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(1);
});

test("reload resumes notification of a broadcast transaction without signing", async ({ page }) => {
  await wallets(page);
  await panel(page, execution(), true);
  await page.locator("#wallet-test-panel").getByRole("button", { name: "Connect tron wallet" }).click();
  await page.locator("#wallet-test-panel").getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(page.getByRole("button", { name: "Retry tracking" })).toBeVisible();
  await page.reload();
  await expect(page.getByRole("button", { name: "Wallets (1)", exact: true })).toBeVisible();
  const mock = await panel(page);
  await expect(page.getByTestId("route-wallet-execution")).toContainText("Submitted.");
  expect(mock.counts()).toEqual({ created: 0, submissions: 1 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});

test("invalid cross-chain recipient cannot create a quote or transfer", async ({ page }) => {
  await wallets(page);
  const mock = await panel(page, execution("near-intents", "tron", "USDT", "near"));
  await page.locator("#wallet-test-panel").getByRole("button", { name: "Connect tron wallet" }).click();
  await page.getByLabel("Swap recipient address").fill("INVALID.NEAR");
  await page.getByLabel("Swap recipient address").press("Tab");
  await page.locator("#wallet-test-panel").getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(page.locator("#wallet-test-panel").getByRole("alert")).toContainText("Invalid NEAR");
  expect(mock.counts()).toEqual({ created: 0, submissions: 0 });
});

test("the unified form accepts a manual recipient on another network", async ({ page }) => {
  await wallets(page);
  const mock = await panel(page, execution("near-intents", "tron", "USDT", "near"));
  const form = page.locator("#wallet-test-panel").getByTestId("wallet-swap-form");
  await form.getByRole("button", { name: "Connect tron wallet" }).click();
  await expect(form.getByLabel("Swap recipient address")).toBeVisible();
  expect(mock.counts()).toEqual({ created: 0, submissions: 0 });
  await form.getByLabel("Swap recipient address").fill("bob.near");
  await form.getByLabel("Swap recipient address").press("Tab");
  await form.getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(form).toContainText("Submitted.");
  await expect(form).toContainText("bob.near");
  expect(mock.counts()).toEqual({ created: 1, submissions: 1 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(1);
});

for (const symbol of ["NEAR", "wNEAR"]) test(`${symbol} deposit uses the correct balance and wraps native NEAR before transferring`, async ({ page }) => {
  await wallets(page);
  const native = symbol === "NEAR";
  const atomicAmount = "12345678000000000000000000";
  await page.route("https://rpc.mainnet.near.org/", route => {
    const query = route.request().postDataJSON().params;
    expect(query.request_type).toBe(native ? "view_account" : "call_function");
    expect(query.account_id).toBe(native ? "alice.near" : "wrap.near");
    return route.fulfill({ json: { result: native
      ? { amount: "20000000000000000000000000" }
      : { result: [...new TextEncoder().encode('"20000000000000000000000000"')] }
    } });
  });
  const value = execution("near-intents", "near", symbol, "near");
  value.action = { ...value.action, asset_id: "nep141:wrap.near", token_contract: "wrap.near", decimals: 24, deposit_memo: "swap-memo" } as any;
  const result = await page.evaluate(async ({ path, value }) => (await import(/* @vite-ignore */ path)).perform(value, "near"), { path: harnessPath, value });
  expect(result.reference).toBe("7".repeat(44));
  const transaction = await page.evaluate(() => {
    const call = (window as any).walletCalls[0];
    return {
      receiverId: call.receiverId,
      actions: call.actions.map((action: any) => ({
        method: action.functionCall.methodName,
        args: JSON.parse(new TextDecoder().decode(action.functionCall.args)),
        deposit: action.functionCall.deposit.toString(),
      })),
    };
  });
  expect(transaction.receiverId).toBe("wrap.near");
  expect(transaction.actions).toEqual([
    ...(native ? [{ method: "near_deposit", args: {}, deposit: atomicAmount }] : []),
    { method: "ft_transfer", args: { receiver_id: "deposit.near", amount: atomicAmount, memo: "swap-memo" }, deposit: "1" },
  ]);
});

test("NEAR token deposit preserves precision, memo and redirect execution identifier", async ({ page }) => {
  await wallets(page);
  await page.route("https://rpc.mainnet.near.org/", route => route.fulfill({ json: { result: { result: [...new TextEncoder().encode('"1000000000"')] } } }));
  const value = execution("near-intents", "near", "USDT", "near");
  value.action = { ...value.action, deposit_memo: "swap-memo" } as any;
  const result = await page.evaluate(async ({ path, value }) => (await import(/* @vite-ignore */ path)).perform(value, "near"), { path: harnessPath, value });
  expect(result).toEqual({ reference: "7".repeat(44), kind: "transaction_hash" });
  const calls = await page.evaluate(() => (window as any).walletCalls);
  expect(calls[0].receiverId).toBe("usdt.tether-token.near");
  expect(calls[0].callbackUrl).toContain(`pay3flow_execution=${value.id}`);
  const args = await page.evaluate(() => JSON.parse(new TextDecoder().decode((window as any).walletCalls[0].actions[0].functionCall.args)));
  expect(args).toEqual({ receiver_id: "deposit.near", amount: "12345678", memo: "swap-memo" });
});

for (const symbol of ["ETH", "USDT"]) test(`Ethereum ${symbol} sends through the connected wallet`, async ({ page }) => {
  await wallets(page);
  const value = execution("near-intents", "ethereum", symbol, "ethereum");
  const result = await page.evaluate(async ({ path, value }) => (await import(/* @vite-ignore */ path)).perform(value, "evm"), { path: harnessPath, value });
  expect(result).toEqual({ reference: "0x" + TX, kind: "transaction_hash" });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(1);
});

for (const reverse of [false, true]) test(`Symbiosis shows a swap widget without wallet execution (${reverse ? "SOL to DAI" : "DAI to SOL"})`, async ({ page }) => {
  await wallets(page);
  await page.route("https://api.symbiosis.finance/crosschain/**", route => route.fulfill({ headers: { "access-control-allow-origin": "*" }, json: route.request().url().endsWith("/chains")
    ? [{ id: 56, name: "BNB" }, { id: 5426, name: "Solana" }]
    : [{ symbol: "DAI", chainId: 56, address: "0x1AF3F329e8BE154074D8769D1FFa4eE058B1DBc3" }, { symbol: "SOL", chainId: 5426, address: "" }] }));
  await page.route("https://app.symbiosis.finance/**", route => route.fulfill({ contentType: "text/html", body: '<!doctype html><button>Connect wallet</button><input aria-label="Solana recipient address">' }));
  let executionRequests = 0;
  await page.route("**/api/p2p/route-executions**", route => { executionRequests += 1; return route.fulfill({ status: 500, json: { error: "This route uses the provider flow" } }); });
  const from = reverse ? "SOL" : "DAI";
  const to = reverse ? "DAI" : "SOL";
  const sourceNetwork = reverse ? "solana" : "bnb-smart-chain";
  const targetNetwork = reverse ? "bnb-smart-chain" : "solana";
  await page.evaluate(async ({ path, from, to, sourceNetwork, targetNetwork }) => {
    const harness = await import(/* @vite-ignore */ path);
    await harness.mountGuide({
      route_id: "symbiosis-provider-flow", route_kind: "crypto_to_crypto", route_provider: "symbiosis",
      source_currency: from, target_currency: to, source_network: sourceNetwork, target_network: targetNetwork, entry_asset: from,
      source_amount: "200", target_amount: "1.92", legs: [], route_path: [`${from}@${sourceNetwork}`, `${to}@${targetNetwork}`],
    });
  }, { path: harnessPath, from, to, sourceNetwork, targetNetwork });
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const widget = guide.getByTestId("provider-swap-frame");
  await expect(widget).toBeVisible();
  const iframe = widget.locator("iframe");
  await expect(iframe).toBeVisible();
  const url = new URL((await iframe.getAttribute("src"))!);
  expect(url.origin).toBe("https://app.symbiosis.finance");
  expect(url.searchParams.get("chainIn")).toBe(reverse ? "Solana" : "BNB");
  expect(url.searchParams.get("chainOut")).toBe(reverse ? "BNB" : "Solana");
  expect(url.searchParams.get(reverse ? "tokenOut" : "tokenIn")).toBe("0x1AF3F329e8BE154074D8769D1FFa4eE058B1DBc3");
  expect(url.searchParams.get(reverse ? "tokenIn" : "tokenOut")).toBe("SOL");
  expect(url.searchParams.get("amountIn")).toBe("200");
  await expect(page.frameLocator('iframe[title="Symbiosis"]').getByRole("button", { name: "Connect wallet" })).toBeVisible();
  await expect(guide.getByTestId("symbiosis-swap-card")).toHaveCount(0);
  await expect(guide.getByTestId("route-wallet-execution")).toHaveCount(0);
  await iframe.evaluate(element => element.setAttribute("data-session", "preserved"));
  await guide.getByRole("button", { name: "SWAP guide", exact: true }).click();
  await expect(guide.getByTestId("symbiosis-swap-card")).toBeVisible();
  await expect(iframe).toBeHidden();
  await guide.getByRole("button", { name: "SWAP", exact: true }).click();
  await expect(iframe).toBeVisible();
  await expect(iframe).toHaveAttribute("data-session", "preserved");
  expect(executionRequests).toBe(0);
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});

for (const [provider, origin] of [["0x", "https://matcha.xyz"], ["barter", "https://app.barterswap.xyz"]]) test(`${provider} provides SWAP and SWAP guide without a Pay3Flow wallet`, async ({ page }) => {
  await wallets(page);
  await page.route(`${origin}/**`, route => route.fulfill({ contentType: "text/html", body: "<!doctype html><button>Connect wallet</button>" }));
  await page.evaluate(async ({ path, provider }) => (await import(/* @vite-ignore */ path)).mountGuide({
    route_id: `${provider}-swap`, route_kind: "crypto_to_crypto", route_provider: provider,
    source_currency: "ETH", target_currency: "USDC", source_network: "ethereum", target_network: "ethereum", entry_asset: "ETH",
    source_amount: "0.1", target_amount: "240", legs: [], route_path: ["ETH@ethereum", "USDC@ethereum"],
  }), { path: harnessPath, provider });
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const swap = guide.getByTestId("provider-swap-frame");
  await expect(swap.getByTestId("swap-quote")).toBeVisible();
  await expect(swap.locator("iframe")).toHaveCount(0);
  expect(new URL((await swap.getByRole("link").getAttribute("href"))!).origin).toBe(origin);
  if (provider !== "barter") await expect(swap).toContainText("does not allow its swap form to be embedded");
  await guide.getByRole("button", { name: "SWAP guide", exact: true }).click();
  await expect(guide.getByTestId("instruction-scene")).toBeVisible();
  await expect(swap).toBeHidden();
  await guide.getByRole("button", { name: "SWAP", exact: true }).click();
  await expect(swap).toBeVisible();
});

test("NEAR Intents mounts its standalone wallet widget and preserves it across guide and theme changes", async ({ page }) => {
  test.setTimeout(60_000);
  await wallets(page);
  await page.route("https://intents-api.aurora.dev/**", route => route.fulfill({
    headers: { "access-control-allow-origin": "*" },
    status: route.request().url().includes("/api/tokens/") ? 200 : 400,
    json: route.request().url().includes("/api/tokens/") ? { asset_stats: [], tokens: [
      { assetId: "nep141:eth.omft.near", symbol: "ETH", blockchain: "eth", decimals: 18, price: 2400 },
      { assetId: "nep141:sol.omft.near", symbol: "SOL", blockchain: "sol", decimals: 9, price: 100 },
    ] } : { message: "Quote fixture" },
  }));
  await page.evaluate(async path => (await import(/* @vite-ignore */ path)).mountGuide({
    route_id: "near-widget", route_kind: "crypto_to_crypto", route_provider: "near-intents",
    source_currency: "ETH", target_currency: "SOL", source_network: "ethereum", target_network: "solana", entry_asset: "ETH",
    source_amount: "0.1", target_amount: "2.4", legs: [], route_path: ["ETH@ethereum", "SOL@solana"],
  }), harnessPath);
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const widget = guide.getByTestId("near-intents-live-widget");
  const input = widget.locator("input").first();
  await expect(input).toHaveValue("0.1", { timeout: 45_000 });
  await expect(widget).toContainText("ETH");
  await expect(widget).toContainText("SOL");
  await widget.getByRole("button", { name: "Connect wallet", exact: true }).first().click();
  const dialog = page.getByRole("dialog", { name: "Connect wallet", exact: true });
  await expect(dialog.getByRole("heading", { name: "Connect wallet", exact: true })).toBeVisible();
  await expect(dialog).toContainText("EVM / Solana");
  await expect(dialog).toContainText("Stellar");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await page.getByRole("button", { name: "Switch theme" }).click();
  await expect(widget.locator(".sw")).toHaveCSS("--c-sw-gray-950", "#191919");
  await guide.getByRole("button", { name: "SWAP guide", exact: true }).click();
  await expect(widget).toBeHidden();
  await expect(guide.getByTestId("instruction-scene")).toBeVisible();
  await guide.getByRole("button", { name: "SWAP", exact: true }).click();
  await expect(widget).toBeVisible();
  await expect(input).toHaveValue("0.1");
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});

test("composed routes open the CoW widget with the current leg's network and exact input", async ({ page }) => {
  await wallets(page);
  await page.route("https://swap.cow.fi/**", route => route.fulfill({ contentType: "text/html", body: "<!doctype html>CoW widget" }));
  await page.evaluate(async path => (await import(/* @vite-ignore */ path)).mountGuide({
    route_id: "composed-cow-route", route_kind: "crypto_to_crypto",
    source_currency: "BTC", target_currency: "EVER", source_network: "bitcoin", target_network: "ethereum", entry_asset: "USDC",
    source_amount: "0.02", target_amount: "233689.083110962", legs: [], route_path: ["BTC@bitcoin", "USDC@ethereum", "EVER@ethereum"],
    cycle_legs: [
      { provider: "near-intents", from_asset: "BTC@bitcoin", to_asset: "USDC@ethereum", input_amount: "0.02", output_amount: "1629.515056" },
      { provider: "cow-swap", from_asset: "USDC@ethereum", to_asset: "EVER@ethereum", input_amount: "1629.515056", output_amount: "233689.083110962", source_url: "https://swap.cow.fi/#/1/swap/0x0000000000000000000000000000000000000020/0x1ffEFD8036409Cb6d652bd610DE465933b226917" },
    ],
  }, "guide"), harnessPath);
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  await guide.getByTestId("confirm-instruction-step").click();
  await guide.getByRole("button", { name: "SWAP", exact: true }).click();
  const iframe = guide.getByTestId("cow-live-widget").locator("iframe");
  await expect(iframe).toBeVisible();
  const url = new URL((await iframe.getAttribute("src"))!);
  expect(url.hash).toContain("/1/widget/swap/0x0000000000000000000000000000000000000020/0x1ffEFD8036409Cb6d652bd610DE465933b226917");
  expect(new URLSearchParams(url.hash.split("?")[1]).get("sellAmount")).toBe("1629.515056");
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});

test("NEAR widget accepts native NEAR and USDT0 catalog aliases", async ({ page }) => {
  test.setTimeout(60_000);
  await wallets(page);
  await page.route("https://intents-api.aurora.dev/**", route => {
    return route.fulfill({
    headers: { "access-control-allow-origin": "*" },
    status: route.request().url().includes("/api/tokens/") ? 200 : 400,
    json: route.request().url().includes("/api/tokens/") ? { asset_stats: [], tokens: [
      { assetId: "nep141:wrap.near", symbol: "wNEAR", blockchain: "near", decimals: 24, price: 3 },
      { assetId: "nep141:usdt0.arb.omft.near", symbol: "USDT0", blockchain: "arb", decimals: 6, price: 1 },
    ] } : { message: "Quote fixture" },
    });
  });
  await page.evaluate(async path => (await import(/* @vite-ignore */ path)).mountGuide({
    route_id: "near-aliases", route_kind: "crypto_to_crypto", route_provider: "near-intents",
    source_currency: "NEAR", target_currency: "USDT", source_network: "near", target_network: "arbitrum-one", entry_asset: "NEAR",
    source_amount: "10.123456789123", target_amount: "30.37", legs: [], route_path: ["NEAR@near", "USDT@arbitrum-one"],
  }), harnessPath);
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const widget = guide.getByTestId("near-intents-live-widget");
  // The official SDK abbreviates the displayed value to five decimals.
  await expect(widget.locator("input").first()).toHaveValue("10.12345", { timeout: 45_000 });
  await expect(widget).toContainText("NEAR");
  await expect(widget).toContainText("USDT");
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});

test("CoW guide embeds the official trading widget and connects without creating a swap", async ({ page }) => {
  await wallets(page);
  const errors: string[] = [];
  page.on("pageerror", error => errors.push(error.message));
  let executionRequests = 0;
  await page.route("**/api/p2p/route-executions**", route => { executionRequests += 1; return route.fulfill({ status: 500, json: { error: "Connection must not create an execution" } }); });
  await page.route("https://swap.cow.fi/**", route => route.fulfill({ contentType: "text/html", body: `<!doctype html><title>CoW Swap</title><main>Official widget fixture</main><script>
    window.messages = [];
    addEventListener('message', event => { window.messages.push(event.data); });
    parent.postMessage({ key: 'cowSwapWidget', method: 'READY' }, '*');
    </script>` }));
  await page.evaluate(async path => {
    const harness = await import(/* @vite-ignore */ path);
    await harness.mountGuide({
      route_id: "cow-native-route", route_kind: "crypto_to_crypto", route_provider: "cow-swap",
      source_currency: "ETH", target_currency: "USDC", source_network: "ethereum", target_network: "ethereum", entry_asset: "ETH",
      source_amount: "0.1", target_amount: "240", legs: [], route_path: ["ETH@ethereum", "USDC@ethereum"],
      route_provider_url: "https://swap.cow.fi/#/1/swap/ETH/USDC",
    });
  }, harnessPath);
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const widget = guide.getByTestId("cow-live-widget");
  await expect(widget).toBeVisible();
  await expect(guide.getByTestId("cow-swap-card")).toHaveCount(0);
  const iframe = widget.locator('iframe[title="CoW Swap"]');
  await expect(iframe).toBeVisible();
  const url = new URL((await iframe.getAttribute("src"))!);
  expect(url.origin).toBe("https://swap.cow.fi");
  expect(url.toString()).toContain("sellAmount=0.1");
  expect(url.hash).toContain("/1/widget/swap/ETH/USDC");
  await widget.getByRole("button", { name: "Connect wallet", exact: true }).click();
  await expect(widget.getByRole("button")).toContainText("0x0000…0001");
  expect(executionRequests).toBe(0);
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
  const frame = page.frameLocator('iframe[title="CoW Swap"]');
  await frame.locator("body").evaluate(() => parent.postMessage({
    key: "cowSwapIframeRpcProviderTransport", method: "PROVIDER_RPC_REQUEST",
    rpcRequest: { id: 77, jsonrpc: "2.0", method: "eth_accounts", params: [] },
  }, "*"));
  await expect.poll(() => frame.locator("body").evaluate(() => (window as any).messages.find((message: any) => message.rpcResponse?.id === 77)?.rpcResponse.result)).toEqual([EVM]);
  await page.getByRole("button", { name: "Switch theme" }).click();
  await expect.poll(() => frame.locator("body").evaluate(() => (window as any).messages.filter((message: any) => message.method === "UPDATE_PARAMS").at(-1)?.urlParams.search)).toContain("theme=dark");
  expect(errors).toEqual([]);
});

test("Symbiosis native balance includes the provider transaction fee", async ({ page }) => {
  await wallets(page);
  const value = execution("symbiosis", "ethereum", "ETH", "ethereum");
  value.input_amount = "1";
  value.action = {
    kind: "symbiosis_transaction", chain_id: 1, source_token: "", input_amount: "1000000000000000000",
    transaction: { chainId: 1, to: EVM, data: "0x12345678", value: "1000150000000000000" },
  } as any;
  const result = await page.evaluate(async ({ path, value }) => {
    try { await (await import(/* @vite-ignore */ path)).perform(value, "evm"); return "submitted"; }
    catch (cause) { return (cause as Error).message; }
  }, { path: harnessPath, value });
  expect(result).toBe("Insufficient balance");
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});

test("changing the recipient discards the prepared quote without starting another swap", async ({ page }) => {
  await wallets(page);
  await page.evaluate(() => { (window as any).tronBalance = "0"; });
  const value = execution();
  value.recipient = "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb";
  const mock = await panel(page, value);
  const executionPanel = page.getByTestId("route-wallet-execution");
  await executionPanel.getByRole("button", { name: "Connect tron wallet" }).click();
  await executionPanel.getByRole("button", { name: "Another address", exact: true }).click();
  await executionPanel.getByLabel("Swap recipient address").fill(value.recipient);
  await executionPanel.getByRole("button", { name: /^Swap .+ for .+$/ }).click();
  await expect(executionPanel).toContainText("Waiting until");
  await executionPanel.getByRole("button", { name: "Connected wallet", exact: true }).click();
  await expect(executionPanel.locator('.review')).toHaveCount(0);
  await expect(executionPanel.getByRole("button", { name: /^Swap .+ for .+$/ })).toBeEnabled();
  expect(mock.counts()).toEqual({ created: 1, submissions: 0 });
  expect(await page.evaluate(() => (window as any).transfers)).toBe(0);
});
