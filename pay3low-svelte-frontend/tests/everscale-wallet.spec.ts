import { expect, test, type Page } from "@playwright/test";

const ADDRESS = `0:${"a".repeat(64)}`;
const OTHER = `0:${"b".repeat(64)}`;

async function setup(page: Page, mode = "normal") {
  await page.addInitScript(({ address, mode }) => {
    localStorage.setItem("pay3flow-locale", "en");
    const browser = window as any;
    browser.everCalls = [];
    browser.everState = {
      selectedConnection: mode === "network" ? "testnet" : "mainnet", networkId: 42,
      permissions: JSON.parse(localStorage.getItem("pay3flow.wallet-families") ?? "[]").includes("everscale") ? { accountInteraction: { address } } : {},
    };
    const listeners = new Map<string, Set<(value: unknown) => void>>();
    browser.everEmit = (event: string) => { for (const callback of listeners.get(event) ?? []) callback({}); };
    browser.everListenerCount = () => [...listeners.values()].reduce((count, values) => count + values.size, 0);
    const provider = {
      request: async ({ method, params }: { method: string; params?: unknown }) => {
        browser.everCalls.push({ method, params });
        if (method === "getProviderState") return structuredClone(browser.everState);
        if (method === "requestPermissions") {
          if (mode === "reject") throw new Error("Connection rejected");
          browser.everState.permissions = { accountInteraction: { address: mode === "invalid" ? "0:broken" : address.toUpperCase() } };
          return structuredClone(browser.everState.permissions);
        }
        if (method === "disconnect") { browser.everState.permissions = {}; browser.everEmit("permissionsChanged"); return; }
        throw new Error(`Unexpected Everscale wallet request: ${method}`);
      },
      addListener: (event: string, callback: (value: unknown) => void) => {
        if (!listeners.has(event)) listeners.set(event, new Set());
        listeners.get(event)!.add(callback);
      },
      removeListener: (event: string, callback: (value: unknown) => void) => listeners.get(event)?.delete(callback),
    };
    if (mode !== "missing") {
      browser.__hasEverscaleProvider = true;
      if (mode === "delayed") browser.initializeEver = () => { browser.__ever = provider; browser.dispatchEvent(new Event("ever#initialized")); };
      else browser.__ever = provider;
    }
  }, { address: ADDRESS, mode });
  await page.route("**/api/**", route => route.fulfill({ json: [] }));
  await page.goto("/");
  await page.getByRole("button", { name: "Connect wallet", exact: true }).click();
}

test("wallet dialog has avatars, contains keyboard focus and closes from every product page", async ({ page }) => {
  await setup(page, "missing");
  for (const path of [null, "/about", "/#/otc"]) {
    if (path) {
      await page.goto(path);
      await page.getByRole("button", { name: "Connect wallet", exact: true }).click();
    }
    const dialog = page.getByRole("dialog", { name: "Wallet connections", exact: true });
    await expect(dialog).toBeVisible();
    await expect(dialog).toHaveAttribute("aria-modal", "true");
    await expect(dialog.locator(".walletIdentity strong")).toHaveText(["Ethereum", "NEAR", "TRON", "Everscale"]);
    await expect.poll(() => dialog.locator(".walletAvatar").evaluateAll(images => images.length === 4 && images.every(image => (image as HTMLImageElement).complete && (image as HTMLImageElement).naturalWidth > 0))).toBe(true);
    await expect(page.locator(".appShell")).toHaveAttribute("inert", "");
    const search = dialog.getByRole("searchbox", { name: "Search wallets" });
    await expect(search).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(dialog.getByRole("button", { name: "Connect Everscale wallet" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(search).toBeFocused();
    const bounds = await dialog.boundingBox();
    expect(bounds!.x).toBeGreaterThanOrEqual(0);
    expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(page.viewportSize()!.width);
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".appShell")).not.toHaveAttribute("inert", "");
    await expect(page.getByRole("button", { name: "Connect wallet", exact: true })).toBeFocused();
    await page.getByRole("button", { name: "Connect wallet", exact: true }).click();
    await dialog.getByRole("button", { name: "Close wallet dialog" }).click();
    await expect(dialog).toHaveCount(0);
    await page.getByRole("button", { name: "Connect wallet", exact: true }).click();
    await page.mouse.click(2, 2);
    await expect(dialog).toHaveCount(0);
    expect(await page.evaluate(() => document.body.style.position)).toBe("");
  }
  await page.setViewportSize({ width: 320, height: 900 });
  await page.locator(".languageToggle").click();
  await page.getByRole("option", { name: "Հայերեն", exact: true }).click();
  await page.locator(".walletToggle").click();
  const dialog = page.locator("#wallet-connections");
  await expect(dialog.locator("h2")).toHaveText("Դրամապանակների միացում");
  expect(await dialog.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  const bounds = await dialog.boundingBox(), closeBounds = await dialog.locator(".closeButton").boundingBox();
  expect(closeBounds!.x + closeBounds!.width).toBeLessThanOrEqual(bounds!.x + bounds!.width);
  await dialog.locator(".closeButton").click();
  await expect(dialog).toHaveCount(0);
});

test("wallet search filters network names and tickers, handles empty results and resets on reopening", async ({ page }) => {
  await setup(page, "missing");
  const dialog = page.getByRole("dialog", { name: "Wallet connections", exact: true });
  const search = dialog.getByRole("searchbox", { name: "Search wallets" });
  await expect(search).toBeFocused();
  for (const [query, expected] of [["  tRx  ", "TRON"], ["ETH evm", "Ethereum"], ["NeAr", "NEAR"], ["эверскейл", "Everscale"]]) {
    await search.fill(query);
    await expect(dialog.locator(".walletIdentity strong")).toHaveText([expected]);
    await page.keyboard.press("ArrowDown");
    await expect(dialog.locator(".connectionButton")).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(search).toBeFocused();
  }
  await search.fill("unknown-wallet");
  await expect(dialog.locator(".walletChoice")).toHaveCount(0);
  await expect(dialog.getByRole("status")).toContainText("No wallets found");
  await page.keyboard.press("Shift+Tab");
  await expect(dialog.getByRole("button", { name: "Close wallet dialog" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(search).toBeFocused();
  await dialog.getByRole("button", { name: "Clear wallet search" }).click();
  await expect(search).toHaveValue("");
  await expect(search).toBeFocused();
  await expect(dialog.locator(".walletChoice")).toHaveCount(4);
  await search.fill("TRX");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await page.getByRole("button", { name: "Connect wallet", exact: true }).click();
  await expect(search).toHaveValue("");
  await expect(dialog.locator(".walletChoice")).toHaveCount(4);
});

test("wallet search finds connected addresses and keeps focus when disconnecting removes a result", async ({ page }) => {
  await setup(page);
  const dialog = page.getByRole("dialog", { name: "Wallet connections", exact: true });
  const search = dialog.getByRole("searchbox", { name: "Search wallets" });
  await search.fill("EVER");
  await connect(page);
  await search.fill(ADDRESS.toUpperCase());
  await expect(dialog.locator(".walletIdentity strong")).toHaveText(["Everscale"]);
  await dialog.getByRole("button", { name: "Disconnect Everscale wallet" }).click();
  await expect(dialog.getByRole("status")).toContainText("No wallets found");
  await expect(search).toBeFocused();
  await dialog.getByRole("button", { name: "Clear wallet search" }).click();
  await expect(dialog.getByRole("button", { name: "Connect Everscale wallet", exact: true })).toBeVisible();
});

test("wallet dialog yields to the NEAR selector and returns after cancellation", async ({ page }) => {
  await setup(page, "missing");
  await page.getByRole("button", { name: "Connect NEAR wallet", exact: true }).click();
  const selector = page.locator("#near-wallet-selector-modal .nws-modal-wrapper.open");
  await expect(selector).toBeVisible({ timeout: 15000 });
  await expect(page.locator(".walletBackdrop")).toBeHidden();
  await selector.locator(".close-button").click();
  const dialog = page.getByRole("dialog", { name: "Wallet connections", exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("alert")).toHaveText("NEAR wallet connection was cancelled");
  await expect(dialog.getByRole("button", { name: "Connect NEAR wallet", exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".appShell")).not.toHaveAttribute("inert", "");
});

async function connect(page: Page) {
  await page.getByRole("button", { name: "Connect Everscale wallet", exact: true }).click();
  await expect(page.getByRole("button", { name: "Disconnect Everscale wallet" })).toBeVisible();
  await expect(page.locator(`.address[title="${ADDRESS}"]`)).toBeVisible();
}

test("Everscale connects, restores without prompting, and revokes permissions on disconnect", async ({ page }) => {
  await setup(page);
  await connect(page);
  expect(await page.evaluate(() => (window as any).everCalls.filter((call: any) => call.method === "requestPermissions"))).toEqual([
    { method: "requestPermissions", params: { permissions: ["basic", "accountInteraction"] } },
  ]);
  await page.reload();
  await page.getByRole("button", { name: "Wallets (1)", exact: true }).click();
  await expect(page.getByRole("button", { name: "Disconnect Everscale wallet" })).toBeVisible();
  expect(await page.evaluate(() => (window as any).everCalls.every((call: any) => call.method === "getProviderState"))).toBe(true);
  await page.getByRole("button", { name: "Disconnect Everscale wallet" }).click();
  await expect(page.getByRole("button", { name: "Connect Everscale wallet", exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as any).everCalls.at(-1).method)).toBe("disconnect");
  expect(await page.evaluate(() => (window as any).everListenerCount())).toBe(0);
  await page.reload();
  await expect(page.getByRole("button", { name: "Connect wallet", exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as any).everCalls)).toEqual([]);
});

test("Everscale follows account changes and invalidates other networks and revoked access", async ({ page }) => {
  await setup(page);
  await connect(page);
  await page.evaluate(address => { (window as any).everState.permissions.accountInteraction.address = address; (window as any).everEmit("permissionsChanged"); }, OTHER);
  await expect(page.locator(`.address[title="${OTHER}"]`)).toBeVisible();
  await page.evaluate(() => { (window as any).everState.selectedConnection = "venom-mainnet"; (window as any).everEmit("networkChanged"); });
  await expect(page.getByRole("button", { name: "Connect Everscale wallet", exact: true })).toBeVisible();
  await page.evaluate(() => { (window as any).everState.selectedConnection = "mainnet"; (window as any).everEmit("networkChanged"); });
  await expect(page.locator(`.address[title="${OTHER}"]`)).toBeVisible();
  await page.evaluate(() => { (window as any).everState.permissions = {}; (window as any).everEmit("permissionsChanged"); });
  await expect(page.getByRole("button", { name: "Connect Everscale wallet", exact: true })).toBeVisible();
});

for (const event of ["loggedOut", "disconnected"]) test(`Everscale clears the account on ${event}`, async ({ page }) => {
  await setup(page);
  await connect(page);
  await page.evaluate(event => (window as any).everEmit(event), event);
  await expect(page.getByRole("button", { name: "Connect Everscale wallet", exact: true })).toBeVisible();
});

test("Everscale waits for delayed EVER Wallet initialization", async ({ page }) => {
  await setup(page, "delayed");
  await page.getByRole("button", { name: "Connect Everscale wallet", exact: true }).click();
  await expect(page.getByRole("button", { name: "Connecting…" })).toBeVisible();
  await page.evaluate(() => (window as any).initializeEver());
  await expect(page.getByRole("button", { name: "Disconnect Everscale wallet" })).toBeVisible();
});

for (const [mode, error] of [
  ["missing", "Install EVER Wallet, or open Pay3Flow in its browser, to connect Everscale."],
  ["network", "Switch EVER Wallet to Everscale mainnet and try again."],
  ["reject", "Connection rejected"],
  ["invalid", "Unlock EVER Wallet and authorize this website."],
]) test(`Everscale handles ${mode} without retaining a session`, async ({ page }) => {
  await setup(page, mode);
  await page.getByRole("button", { name: "Connect Everscale wallet", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Wallet connections" }).getByRole("alert")).toHaveText(error);
  await expect(page.getByRole("button", { name: "Connect wallet", exact: true })).toBeVisible();
  expect(await page.evaluate(() => localStorage.getItem("pay3flow.wallet-families"))).toBeNull();
  if (mode === "network") expect(await page.evaluate(() => (window as any).everCalls.some((call: any) => call.method === "requestPermissions"))).toBe(false);
});

test("Everscale connections cannot execute an unsupported swap action", async ({ page }) => {
  await setup(page);
  await connect(page);
  const message = await page.evaluate(async () => {
    const execution = await import(/* @vite-ignore */ "/src/lib/wallet-execution.ts");
    const wallet = await execution.restoreWallet("everscale");
    try { await execution.prepareWalletAction({ from_asset: "EVER@everscale" } as any, wallet!); }
    catch (error) { return (error as Error).message; }
  });
  expect(message).toBe("Everscale swap execution is not available for this provider.");
  expect(await page.evaluate(() => (window as any).everCalls.every((call: any) => ["getProviderState", "requestPermissions"].includes(call.method)))).toBe(true);
});
