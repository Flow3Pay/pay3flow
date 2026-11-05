import { expect, test, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

const ever = `0:${"a".repeat(64)}`, eth = `0x${"b".repeat(40)}`;
async function setup(page: Page, options: { signedIn?: boolean; lostResponse?: boolean; verifiedEthereum?: boolean; cancelledReplay?: boolean; failPublicRefresh?: boolean; failPrivateRefresh?: boolean; deals?: any[]; balances?: { ever?: string | null; usdt?: string | null; state?: "ok" | "unverified" | "unavailable"; fail?: boolean } } = {}) {
  const session = { token: "seller-token", actor: "https://relay.example/actors/seller", desk: false, expires_at: "2099-01-01T00:00:00Z", signing_wallet: { chain: "everscale", address: ever }, wallets: [{ chain: "everscale", address: ever, verified_at: new Date().toISOString() }] };
  if (options.verifiedEthereum) session.wallets.push({chain:"ethereum",address:eth,verified_at:new Date().toISOString()});
  const calls: { path: string; method: string; body: any; key: string | undefined }[] = [];
  const offers: any[] = [];
  const keys = new Map<string, any>();
  let lost = options.lostResponse;
  let cancelledReplay = options.cancelledReplay;
  await page.addInitScript(({ session, signedIn, ever }) => {
    localStorage.setItem("pay3flow-locale", "en");
    if (signedIn && !sessionStorage.getItem("public-offer-fixture-initialized")) {
      sessionStorage.setItem("pay3flow.otc.session.v1", JSON.stringify(session));
      sessionStorage.setItem("public-offer-fixture-initialized", "1");
    }
    const browser = window as any;
    browser.everCalls = [];
    browser.evmCalls = [];
    browser.ethereum = {
      request: async ({ method }: { method: string }) => {
        browser.evmCalls.push(method);
        if (["eth_accounts", "eth_requestAccounts"].includes(method)) return [`0x${"b".repeat(40)}`];
        if (method === "eth_chainId") return "0x1";
        if (method === "wallet_requestPermissions") return [{ parentCapability: "eth_accounts" }];
        throw new Error(`Unexpected Ethereum action ${method}`);
      }, on() {}, off() {},
    };
    let permissions: any = {};
    browser.__ever = {
      request: async ({ method }: { method: string }) => {
        browser.everCalls.push(method);
        if (method === "getProviderState") return { selectedConnection: "mainnet", networkId: 42, permissions };
        if (method === "requestPermissions") return permissions = { accountInteraction: { address: ever, publicKey: "a".repeat(64) } };
        if (method === "signData") return { signature: btoa("fixture-login-signature") };
        if (method === "disconnect") { permissions = {}; return; }
        throw new Error(`Unexpected wallet action ${method}`);
      }, addListener() {}, removeListener() {},
    };
  }, { session, signedIn: options.signedIn, ever });
  await page.route("**/api/**", async route => {
    const req = route.request(), path = new URL(req.url()).pathname.replace("/api/otc", "");
    const body = req.postDataJSON(), key = req.headers()["idempotency-key"];
    calls.push({ path, method: req.method(), body, key });
    let response: any = [], status = 200;
    if (path.endsWith("/market-prices")) response = { source: "stub", stale: false, updated_at: Math.floor(Date.now() / 1000), prices: { EVER: 0.00105027, USDT: 0.9998 } };
    else if (path === "/balances") {
      const wallet = options.balances;
      if (wallet?.fail) { status = 503; response = { error: "Balance reader is unavailable" }; }
      else {
        const leg = (symbol: "EVER" | "USDT", amount: string | null | undefined) => ({
          chain: symbol === "EVER" ? "everscale" : "ethereum", symbol, network: symbol === "EVER" ? "Everscale" : "Ethereum",
          amount: amount ?? null, state: wallet?.state ?? (amount === undefined ? "unavailable" : "ok"),
        });
        response = { balances: [leg("EVER", wallet?.ever), leg("USDT", wallet?.usdt)] };
      }
    }
    else if (path === "/config") response = { enabled: false, available: false, public_offers_available: true, public_offer_directions: ["buy","sell"], p2p_available: !!options.deals, wallet_login_available: true, wallet_login_chains: ["everscale", "ethereum"], demo: false, desk: "", desk_name: "", ever_resource: "", usdt_resource: "" };
    else if (path === "/session/challenge") response = { id: "challenge", message: "Sign in only; no transfer", everscale_data: btoa("sign-in") };
    else if (path === "/session/proof" || path === "/session") response = session;
    else if (path === "/session/revoke") response = { revoked: true };
    else if (path === "/p2p/trades") response = { deals: options.deals || [] };
    else if (path === "/trades") response = { trades: [], rfqs: [] };
    else if (path === "/offers" && req.method() === "POST") {
      let offer = keys.get(key!);
      if (!offer) {
        offer = { id: `offer-${keys.size}`, seller_ever: ever, terms: body, state: "open", created_at: new Date().toISOString(), expires_at: "2099-01-01T00:00:00Z" };
        if (cancelledReplay) { offer.state = "cancelled"; cancelledReplay = false; }
        offers.push(offer); keys.set(key!, offer);
      }
      response = offer;
      if (lost) { lost = false; status = 503; response = { error: "Publication response lost" }; }
    } else if (path === "/offers") { response = offers.filter(offer => offer.state === "open"); if (options.failPublicRefresh && offers.length) { status=503;response={error:"Public offer refresh unavailable"};} }
    else if (path === "/offers/mine") { response = offers; if (options.failPrivateRefresh) {status=503;response={error:"Private offer list unavailable"};} }
    else if (path === "/offers/history") { response = offers.map(({id,created_at,terms}) => ({id,created_at,terms:{direction:terms.direction || "sell",stablecoin:terms.stablecoin || "USDT",ever_amount:terms.ever_amount,usdt_amount:terms.usdt_amount}})); }
    else if (path.endsWith("/cancel")) { response = offers.find(offer => path.includes(offer.id)); response.state = "cancelled"; }
    await route.fulfill({ status, json: response });
  });
  await page.goto("/#/otc?market=EVER-USDT&side=sell");
  await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-state", "connected");
  await expect(page.locator(".bridgeForm .sideTabs").getByRole("button", {name:"Sell EVER",exact:true})).toHaveAttribute("aria-pressed", "true");
  return { calls, offers };
}
async function openOfferSettings(page: Page) {
  const toggle = page.getByRole("button", { name: "Offer settings", exact: true });
  if (await toggle.getAttribute("aria-expanded") !== "true") await toggle.click();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
}
async function closeOfferSettings(page: Page) {
  const toggle = page.getByRole("button", { name: "Offer settings", exact: true });
  if (await toggle.getAttribute("aria-expanded") === "true") await toggle.click();
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
}
async function fill(page: Page) {
  await page.getByTestId("otc-offer-ever").fill("10.000000001");
  await page.getByTestId("otc-offer-usdt").fill("1.000001");
  await openOfferSettings(page);
  await page.getByTestId("otc-offer-recipient").fill(eth);
}

test("seller signs in with EVER alone and publishes, restores, and cancels a real-network advertisement", async ({ page }) => {
  const f = await setup(page);
  await fill(page);
  await page.getByRole("button", { name: "Connect EVER Wallet to publish", exact: true }).click();
  await expect(page.getByTestId("otc-publish-offer")).toBeVisible();
  await expect(page.getByTestId("otc-offer-ever")).toHaveValue("10.000000001");
  await openOfferSettings(page);
  await expect(page.getByTestId("otc-offer-recipient")).toHaveValue(eth);
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByTestId("otc-offer-review")).toContainText(ever);
  expect(f.calls.filter(call => call.path === "/offers" && call.method === "POST")).toHaveLength(0);
  await page.getByTestId("otc-offer-publish").click();
  await expect(page.locator(".serviceControls [role=status]")).toContainText("Your EVER sale offer is published.");
  await expect(page.getByTestId("otc-public-offer")).toContainText("10.000000001");
  await expect(page.getByTestId("otc-public-offer")).toContainText("1.000001");
  expect(f.calls.find(call => call.path === "/session/challenge")?.body.chain).toBe("everscale");
  expect(f.calls.find(call => call.path === "/offers" && call.method === "POST")?.body).toEqual({ direction:"sell", stablecoin:"USDT", ever_amount: "10.000000001", usdt_amount: "1.000001", usdt_address: eth });
  expect(f.calls.some(call => /rfqs|attempts|prepare|decision/.test(call.path))).toBe(false);
  expect(await page.evaluate(() => (window as any).everCalls)).toContain("signData");
  expect(await page.evaluate(() => (window as any).everCalls)).not.toContain("sendMessage");
  await page.reload();
  await expect(page.getByTestId("otc-public-offer")).toBeVisible();
  await page.getByTestId("otc-offer-take").click();
  await page.getByRole("button", { name: "Cancel offer", exact: true }).click();
  await expect(page.getByTestId("otc-public-offer")).toHaveCount(0);
  await page.getByRole("tab", { name:"History", exact:true }).click();
  await expect(page.locator(".tradeTables tbody tr[data-kind=offer]").first()).toContainText("Cancelled");
});

test("market price hint fills the USDT total and toggles a live rate", async ({ page }) => {
  await setup(page, { signedIn: true });
  await page.getByTestId("otc-offer-ever").fill("100000");
  await expect(page.getByTestId("otc-offer-usdt")).toHaveAttribute("placeholder", "≈ 105.048009");
  await page.getByRole("checkbox", { name: "Market price", exact: true }).check();
  await expect(page.getByTestId("otc-offer-price")).toHaveValue("0.001050480096");
  await expect(page.getByTestId("otc-offer-price")).toHaveJSProperty("readOnly", true);
  await expect(page.getByTestId("otc-offer-usdt")).toHaveValue("105.048009");
  await page.getByRole("checkbox", { name: "Market price", exact: true }).uncheck();
  await expect(page.getByTestId("otc-offer-price")).toHaveJSProperty("readOnly", false);
  await page.getByTestId("otc-offer-price").fill("0.2");
  await expect(page.getByTestId("otc-offer-usdt")).toHaveValue("20000");
});

test("the searchable token picker publishes USDC only on Ethereum ERC-20 and keeps its identity through P2P", async ({ page }) => {
  const f = await setup(page, { signedIn: true });
  await fill(page);
  await page.getByTestId("otc-stablecoin-select").click();
  const picker = page.getByTestId("otc-stablecoin-picker");
  await expect(picker).toBeVisible();
  await picker.getByLabel("Search tokens").fill("USD Coin");
  await expect(picker.getByTestId("otc-stablecoin-usdt")).toHaveCount(0);
  await picker.getByTestId("otc-stablecoin-usdc").click();
  await expect(page.getByTestId("otc-stablecoin-select")).toContainText("USDC");
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByTestId("otc-offer-review")).toContainText("1.000001 USDC");
  await expect(page.getByTestId("otc-offer-review")).toContainText("USDC / EVER");
  await page.getByTestId("otc-offer-publish").click();
  expect(f.calls.find(call => call.path === "/offers" && call.method === "POST")?.body).toEqual({
    direction: "sell", stablecoin: "USDC", ever_amount: "10.000000001", usdt_amount: "1.000001", usdt_address: eth,
  });
  await expect(page.getByTestId("otc-public-offer")).toContainText("USDC");
  await page.getByTestId("otc-offer-take").click();
  await expect(page.getByRole("dialog", { name: "Offer details" })).toContainText("USDC receiving wallet");
});

test("USD input converts the EVER amount and shows the token value", async ({ page }) => {
  await setup(page, { signedIn: true });
  await page.getByTestId("otc-offer-ever").fill("1000");
  await expect(page.getByTestId("otc-market-value")).toHaveText("$1.05");

  const toggle = page.getByRole("button", { name: "Switch amount between USD and EVER", exact: true });
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("otc-offer-ever")).toHaveValue("1.05");

  await page.getByTestId("otc-offer-ever").fill("100");
  await expect(page.getByTestId("otc-offer-ever")).toHaveValue("100");
  await expect(page.getByTestId("otc-market-value")).toHaveText("95,213.61173793 EVER");

  await toggle.click();
  await expect(page.getByTestId("otc-offer-ever")).toHaveValue("95213.611737934");
  await expect(page.getByTestId("otc-market-value")).toHaveText("$100.00");
});


test("a P2P deal opens as a modal with both wallets and one confirmation", async ({ page }) => {
  const deal = {
    id: "deal-1", offer_id: "offer-1", state: "delivery_pending", role: "owner",
    counterparty: "https://relay.example/actors/taker",
    terms: {
      direction: "sell", ever_amount: "100", usdt_amount: "11.889", ever_units: "100000000000", usdt_units: "11889000",
      usdt_payer: `0x${"c".repeat(40)}`, usdt_payee: eth,
      ever_payer: ever, ever_payee: `0:${"c".repeat(64)}`,
    },
    my_leg: "payout", counterparty_leg: "payment",
    my_ever: ever, counterparty_ever: `0:${"c".repeat(64)}`,
    my_confirmed: false, counterparty_confirmed: true,
    owner_confirmed: false, taker_confirmed: true,
    pay_by: "2099-01-01T00:00:00Z", payout_by: "2099-01-01T00:00:00Z",
    created_at: new Date().toISOString(), attempts: [],
  };
  await setup(page, { signedIn: true, verifiedEthereum: true, deals: [deal] });
  const modal = page.getByTestId("otc-p2p-modal");
  await expect(modal).toBeVisible();
  await expect(modal.getByTestId("otc-p2p-legs")).toContainText("You send");
  await expect(modal.getByTestId("otc-p2p-legs")).toContainText("100 EVER");
  await expect(modal.getByTestId("otc-p2p-legs")).toContainText("You receive");
  await expect(modal.getByTestId("otc-p2p-legs")).toContainText("11.889 USDT");
  await modal.getByRole("button", { name: "Close", exact: true }).click();
  await expect(modal).not.toBeVisible();
  await page.reload();
  await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-state", "connected");
  await expect(modal).not.toBeVisible();
  await page.getByRole("tab", { name: /^My orders/ }).click();
  await expect(modal).not.toBeVisible();
  const reopen = page.getByTestId("otc-open-active-deal");
  await expect(reopen).toBeVisible();
  await reopen.click();
  await expect(modal).toBeVisible();
  await expect(modal).toContainText("100");
  await expect(modal).toContainText("11.889");
  await expect(modal.getByTestId("otc-p2p-avatar-self")).toBeVisible();
  await expect(modal.getByTestId("otc-p2p-avatar-counterparty")).toBeVisible();
  await expect(modal.getByTestId("otc-p2p-confirm")).toBeEnabled();
  await expect(modal.getByTestId("otc-p2p-confirm")).toHaveText("Confirm");
  await expect(page.locator(".deals, otc-p2p-deal")).toHaveCount(0);
});

test("P2P deal details identify USDC as the ERC-20 payment leg", async ({ page }) => {
  const deal = {
    id: "usdc-deal", offer_id: "usdc-offer", state: "delivery_pending", role: "owner",
    counterparty: "https://relay.example/actors/taker",
    terms: {
      direction: "sell", stablecoin: "USDC", ever_amount: "100", usdt_amount: "11.889",
      ever_units: "100000000000", usdt_units: "11889000",
      usdt_payer: `0x${"c".repeat(40)}`, usdt_payee: eth,
      ever_payer: ever, ever_payee: `0:${"c".repeat(64)}`,
    },
    my_leg: "payout", counterparty_leg: "payment",
    my_ever: ever, counterparty_ever: `0:${"c".repeat(64)}`,
    my_confirmed: false, counterparty_confirmed: true,
    owner_confirmed: false, taker_confirmed: true,
    pay_by: "2099-01-01T00:00:00Z", payout_by: "2099-01-01T00:00:00Z",
    created_at: new Date().toISOString(), attempts: [],
  };
  await setup(page, { signedIn: true, verifiedEthereum: true, deals: [deal] });
  const modal = page.getByTestId("otc-p2p-modal");
  await expect(modal).toContainText("11.889 USDC");
  await expect(modal.getByTestId("otc-p2p-legs")).toContainText("11.889 USDC");
});

test("the delivery button names the EVER leg instead of a generic confirmation", async ({ page }) => {
  const deal = {
    id: "deal-1", offer_id: "offer-1", state: "delivery_pending", role: "owner",
    counterparty: "https://relay.example/actors/taker",
    terms: {
      direction: "sell", ever_amount: "100", usdt_amount: "11.889", ever_units: "100000000000", usdt_units: "11889000",
      usdt_payer: `0x${"c".repeat(40)}`, usdt_payee: eth,
      ever_payer: ever, ever_payee: `0:${"c".repeat(64)}`,
    },
    my_leg: "payout", counterparty_leg: "payment",
    my_ever: ever, counterparty_ever: `0:${"c".repeat(64)}`,
    my_confirmed: true, counterparty_confirmed: true,
    owner_confirmed: true, taker_confirmed: true,
    pay_by: "2099-01-01T00:00:00Z", payout_by: "2099-01-01T00:00:00Z",
    created_at: new Date().toISOString(), attempts: [],
  };
  // Auto-send is deliberately blocked so the primary action stays visible for inspection.
  await page.addInitScript(() => sessionStorage.setItem("pay3flow.otc.p2p.auto.deal-1:payout", "1"));
  await setup(page, { signedIn: true, verifiedEthereum: true, deals: [deal] });
  const modal = page.getByTestId("otc-p2p-modal");
  await expect(modal).toBeVisible();
  await expect(modal.getByTestId("otc-p2p-legs")).toContainText("You send");
  await expect(modal.getByTestId("otc-p2p-legs")).toContainText("100 EVER");
  await expect(modal.getByTestId("otc-p2p-confirm")).toHaveText("Send EVER");
});

for (const direction of ["sell","buy"] as const) test(`a lost ${direction} publication restores the exact draft and reuses its key after reload`, async ({ page }) => {
  const f = await setup(page, { signedIn: true, lostResponse: true });
  await page.locator(".sideTabs").getByRole("button",{name:direction === "buy" ? "Buy EVER" : "Sell EVER",exact:true}).click();
  await fill(page);
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-offer-publish").click();
  await expect(page.getByRole("alert")).toHaveText("Publication response lost");
  await page.reload();
  await expect(page.getByTestId("otc-offer-ever")).toHaveValue("10.000000001");
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-offer-publish").click();
  await expect(page.getByTestId("otc-public-offer")).toHaveCount(1);
  const sent = f.calls.filter(call => call.path === "/offers" && call.method === "POST");
  expect(sent).toHaveLength(2); expect(sent[0].key).toBeTruthy(); expect(sent[1].key).toBe(sent[0].key);
  expect(sent[1].body).toEqual(sent[0].body);
});

test("publication rejects zero, negative and over-cap amounts, floors extra digits, and fits an accessible 320px screen", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 820 });
  await setup(page, { signedIn: true }); await fill(page);
  for (const value of ["0", "-10"]) {
    await page.getByTestId("otc-offer-ever").fill(value);
    await expect(page.getByTestId("otc-publish-offer")).toBeDisabled();
  }
  await page.getByTestId("otc-offer-ever").fill("1000000000001");
  await expect(page.getByTestId("otc-publish-offer")).toBeDisabled();
  await page.getByTestId("otc-offer-ever").fill("1.0000000001");
  await expect(page.getByTestId("otc-publish-offer")).toBeEnabled();
  await expect(page.locator(".priceHint")).toContainText("extra digits you typed were dropped");
  await fill(page); await page.getByTestId("otc-offer-recipient").fill(`0x${"0".repeat(40)}`);
  await expect(page.getByTestId("otc-publish-offer")).toBeDisabled();
  await fill(page); await expect(page.getByTestId("otc-publish-offer")).toBeEnabled();
  await page.getByTestId("otc-publish-offer").click();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  expect((await new AxeBuilder({ page }).include(".otcWorkspace").withTags(["wcag2a", "wcag2aa"]).analyze()).violations).toEqual([]);
});

test("receiving address can be obtained from Ethereum without a USDT balance or a signature", async ({ page }) => {
  await setup(page, { signedIn: true });
  await openOfferSettings(page);
  await page.getByRole("button", { name: "Use my Ethereum wallet address", exact: true }).click();
  await expect.poll(async () => (await page.getByTestId("otc-offer-recipient").inputValue()).toLowerCase()).toBe(eth);
  const calls: string[] = await page.evaluate(() => (window as any).evmCalls);
  expect(calls).toContain("eth_requestAccounts");
  expect(calls.some(method => /sign|sendTransaction|balance/i.test(method))).toBe(false);
});


test("sale publication uses the original Buy/Sell ticket, book and activity without another market screen", async ({ page }) => {
  await setup(page, { signedIn: true });
  await page.getByTestId("otc-offer-ever").fill("100");
  await page.getByTestId("otc-offer-usdt").fill("0.11889");
  await openOfferSettings(page);
  await page.getByTestId("otc-offer-recipient").fill(eth);
  await expect(page.getByRole("textbox", { name:"Price", exact:true })).toHaveValue("0.0011889");
  await expect(page.locator(".bridgeColumn .bridgeForm")).toBeVisible();
  await expect(page.locator(".chartColumn .marketChart")).toContainText("EVER / USDT");
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByTestId("otc-offer-review")).toContainText("0.0011889 USDT / EVER");
  const split = page.getByTestId("otc-offer-fee-split");
  await expect(split).toContainText("Offer amount");
  await expect(split).toContainText("Service fee (3%)");
  await expect(split).toContainText("0.003567 USDT");
  await expect(split).toContainText("11.889 USDT");
  await page.getByTestId("otc-offer-publish").click();
  const published = page.locator(".bookColumn .obRow");
  await expect(published).toHaveCount(1);
  await expect(published).toContainText("100");
  await expect(published).toContainText("0.11889");
  await expect(page.locator(".chartColumn .marketChart")).toHaveAttribute("data-points", "1");
  await page.setViewportSize({width:1440,height:1000});
  await expect.poll(async () => page.evaluate(() => {
    const grid=document.querySelector(".otcGrid")!.getBoundingClientRect();
    const chart=document.querySelector(".chartColumn")!.getBoundingClientRect();
    return Math.abs(grid.right-chart.right);
  })).toBeLessThan(1);
  await expect(page.getByRole("group", {name:"OTC mode",exact:true})).toHaveCount(0);
  await expect(page.locator(".otcGrid")).toHaveCount(1);
  await expect(page.locator(".bridgeForm")).toHaveCount(1);
  await page.locator(".bridgeForm .sideTabs").getByRole("button", { name:"Buy EVER", exact:true }).click();
  await expect(page.getByTestId("otc-publish-offer")).toBeVisible();
  await expect(page.locator("#otc-input")).toHaveAttribute("data-testid","otc-offer-ever");
  await page.locator(".bridgeForm .sideTabs").getByRole("button", { name:"Sell EVER", exact:true }).click();
  await expect(page.getByTestId("otc-offer-ever")).toHaveValue("100");
  await expect(page.getByTestId("otc-offer-usdt")).toHaveValue("0.11889");
  await expect(page.getByTestId("otc-public-offer")).toContainText("100");
  await expect(page.getByTestId("otc-public-offer")).toContainText("0.11889");
  await expect(page.locator(".bookColumn .obRow.own")).toContainText("0.11889");
});


for (const direction of ["buy","sell"] as const) test(`price and amount publish and cancel an exact ${direction} offer without silent blocking`, async ({ page }) => {
  const f=await setup(page,{signedIn:true});
  await page.locator(".sideTabs").getByRole("button",{name:direction === "buy" ? "Buy EVER" : "Sell EVER",exact:true}).click();
  await expect(page.getByTestId("otc-offer-price")).toHaveJSProperty("readOnly",false);
  await page.getByTestId("otc-offer-price").fill("0,11889");
  await page.locator("#otc-input").fill("100");
  await expect(page.getByTestId("otc-offer-ever")).toHaveValue("100");
  await expect(page.getByTestId("otc-offer-usdt")).toHaveValue("11.889");
  await expect(page.getByTestId("otc-publish-offer")).toBeDisabled();
  await openOfferSettings(page);
  await page.getByTestId("otc-offer-recipient").fill(`  ${eth}  `);
  await expect(page.getByTestId("otc-publish-offer")).toBeEnabled();
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByTestId("otc-offer-review")).toContainText(`${direction === "buy" ? "Buy" : "Sell"} 100 EVER · 11.889 USDT`);
  expect(f.calls.filter(c=>c.path === "/offers" && c.method === "POST")).toHaveLength(0);
  await page.getByTestId("otc-offer-publish").click();
  const offer=page.locator(`.bookColumn .obTable[data-side="${direction === "buy" ? "bids" : "asks"}"]`).getByTestId("otc-public-offer");
  await expect(offer).toHaveCount(1);
  await expect(offer).toContainText("0.11889");
  await expect(offer).toContainText("100");
  await expect(offer).toContainText("11.889");
  expect(f.calls.find(c=>c.path === "/offers" && c.method === "POST")?.body).toEqual({direction,stablecoin:"USDT",ever_amount:"100",usdt_amount:"11.889",usdt_address:eth});
  await offer.getByTestId("otc-offer-take").click();
  await page.getByRole("button",{name:"Cancel offer",exact:true}).click();
  await expect(page.locator(".bookColumn").getByTestId("otc-public-offer")).toHaveCount(0);
  await page.getByRole("tab",{name:"History",exact:true}).click();
  await expect(page.locator(".tradeTables tbody tr[data-kind=offer]").first()).toContainText("Cancelled");
});

test("decimal commas, final totals, and price rounding remain explicit", async ({ page }) => {
  await setup(page,{signedIn:true});
  await page.getByTestId("otc-offer-price").fill("0.123456789012");
  await page.getByTestId("otc-offer-ever").fill("1");
  await expect(page.getByTestId("otc-offer-usdt")).toHaveValue("0.123456");
  await expect(page.getByText(/total is rounded down/)).toBeVisible();
  await page.getByTestId("otc-offer-usdt").fill("1,25");
  await expect(page.getByTestId("otc-offer-price")).toHaveValue("0.123456789012");
  await expect(page.getByTestId("otc-offer-ever")).toHaveValue("10.125000091");
  await openOfferSettings(page);
  await page.getByTestId("otc-offer-recipient").fill(eth);
  await expect(page.getByTestId("otc-publish-offer")).toBeEnabled();
  await page.getByTestId("otc-offer-price").fill("0");
  await expect(page.getByTestId("otc-publish-offer")).toBeDisabled();
});


test("an already configured Ethereum wallet fills the address without a second connection", async ({ page }) => {
  const f=await setup(page,{signedIn:true,verifiedEthereum:true});
  await page.getByTestId("otc-offer-price").fill("0.1");
  await page.getByTestId("otc-offer-ever").fill("100");
  await openOfferSettings(page);
  await expect(page.getByTestId("otc-offer-recipient")).toHaveValue(eth);
  await expect(page.getByTestId("otc-publish-offer")).toBeEnabled();
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByTestId("otc-offer-review")).toContainText("Sell 100 EVER · 10 USDT");
  expect(f.calls.some(c=>c.path === "/offers" && c.method === "POST")).toBe(false);
});

test("the offer review is a modal that Escape returns from and a failed publication reports inside", async ({ page }) => {
  await setup(page, { signedIn: true, lostResponse: true });
  await fill(page);
  await page.getByTestId("otc-publish-offer").click();
  const review = page.getByRole("dialog", { name: "Review your offer", exact: true });
  await expect(review).toBeVisible();
  await expect(review.getByTestId("otc-offer-fee-split")).toContainText("Service fee (3%)");
  await page.keyboard.press("Escape");
  await expect(review).toHaveCount(0);
  await expect(page.getByTestId("otc-publish-offer")).toHaveText(/Review offer/);
  await expect(page.getByTestId("otc-publish-offer")).toBeFocused();
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-offer-publish").click();
  await expect(review.getByRole("alert")).toHaveText("Publication response lost");
});

for (const direction of ["buy","sell"] as const) test(`HEX-style ${direction} quantity keeps unit price identical in the ticket, book and chart`, async ({ page }) => {
  const f=await setup(page,{signedIn:true});
  await page.locator(".sideTabs").getByRole("button",{name:direction === "buy" ? "Buy EVER" : "Sell EVER",exact:true}).click();
  await page.getByTestId("otc-offer-price").fill("0.205");
  await page.locator("#otc-input").fill("10");
  await expect(page.locator("#otc-input")).toHaveAttribute("data-testid","otc-offer-ever");
  await expect(page.getByTestId("otc-offer-usdt")).toHaveValue("2.05");
  await expect(page.getByTestId("otc-offer-price")).toHaveValue("0.205");
  await expect(page.locator(".marketStat.last")).toContainText("0.205 USDT / EVER");
  await openOfferSettings(page);
  await page.getByTestId("otc-offer-recipient").fill(eth);
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-offer-publish").click();
  const row=page.locator(".bookColumn").getByTestId("otc-public-offer");
  await expect(row.getByTestId("otc-book-price")).toHaveText("0.205");
  await expect(row).toContainText("2.05");
  await expect(page.getByTestId("otc-chart-price")).toHaveText("0.205 $");
  await expect(page.locator(".chartColumn .marketChart")).toHaveAttribute("data-points", "1");
  expect(f.calls.find(c=>c.method === "POST" && c.path === "/offers")?.body).toEqual({direction,stablecoin:"USDT",ever_amount:"10",usdt_amount:"2.05",usdt_address:eth});
  await page.reload();
  await expect(page.locator(".bookColumn").getByTestId("otc-book-price")).toHaveText("0.205");
  await expect(page.getByTestId("otc-chart-price")).toHaveText("0.205 $");
});

test("a cancelled publication replay is never reported as a newly published offer",async({page})=>{
  const f=await setup(page,{signedIn:true,cancelledReplay:true});
  await fill(page);
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-offer-publish").click();
  await expect(page.getByRole("alert")).toContainText("already cancelled or expired");
  await expect(page.locator(".serviceControls [role=status]")).toHaveCount(0);
  await expect(page.locator(".bookColumn").getByTestId("otc-public-offer")).toHaveCount(0);
  await expect(page.getByTestId("otc-publish-offer")).toHaveText(/Review offer/);
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-offer-publish").click();
  await expect(page.locator(".serviceControls [role=status]")).toContainText("is published");
  await expect(page.locator(".bookColumn").getByTestId("otc-public-offer")).toHaveCount(1);
  const posts=f.calls.filter(c=>c.method === "POST" && c.path === "/offers");
  expect(posts).toHaveLength(2);expect(posts[0].key).not.toBe(posts[1].key);
});

test("a saved publication populates the book and chart even if the following public refresh fails",async({page})=>{
  await setup(page,{signedIn:true,failPublicRefresh:true});
  await fill(page);
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-offer-publish").click();
  await expect(page.getByRole("alert")).toHaveText("Public offer refresh unavailable");
  await expect(page.locator(".bookColumn").getByTestId("otc-public-offer")).toHaveCount(1);
  await expect(page.locator(".chartColumn .marketChart")).toHaveAttribute("data-points", "1");
  await page.locator(".bookColumn").getByTestId("otc-offer-take").click();
  await page.getByRole("button",{name:"Cancel offer",exact:true}).click();
  await expect(page.locator(".bookColumn").getByTestId("otc-public-offer")).toHaveCount(0);
  await expect(page.locator(".chartColumn .marketChart")).toHaveAttribute("data-points", "1");
});

test("public book loads despite a failed private history, with asks ascending and bids descending",async({page})=>{
  const f=await setup(page,{signedIn:true,failPrivateRefresh:true});
  for(const [id,direction,amount,total] of [["ask-high","sell","10","3"],["bid-low","buy","10","1"],["ask-low","sell","10","2"],["bid-high","buy","10","1.5"]]){
    f.offers.push({id,seller_ever:ever,state:"open",terms:{direction,ever_amount:amount,usdt_amount:total,usdt_address:eth},created_at:new Date().toISOString(),expires_at:"2099-01-01T00:00:00Z"});
  }
  await page.reload();
  await expect(page.getByRole("alert")).toHaveText("Private offer list unavailable");
  await expect(page.locator(".bookColumn").getByTestId("otc-book-price")).toHaveText(["0.2","0.3","0.15","0.1"]);
  await expect(page.locator(".chartColumn .marketChart")).toHaveAttribute("data-points", "4");
});

test("a shortfall on the funded leg warns beside the amount and still publishes", async ({ page }) => {
  const f = await setup(page, { signedIn: true, balances: { ever: "1.5", usdt: "900" } });
  await fill(page);
  const warning = page.getByTestId("otc-shortfall-warning");
  await expect(warning).toHaveCount(1);
  await expect(page.locator("#otc-input")).toHaveClass(/shortfall/);
  await expect(page.locator("#otc-receive")).not.toHaveClass(/shortfall/);
  await expect(warning).toHaveAttribute("data-tooltip", "Not enough EVER in your wallet — available 1.5 EVER. You can still publish the offer.");
  await closeOfferSettings(page);
  await warning.hover();
  await expect(page.getByRole("tooltip")).toContainText("Not enough EVER in your wallet");
  await expect(page.getByTestId("otc-publish-offer")).toBeEnabled();
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByTestId("otc-offer-review")).toBeVisible();
  expect(f.calls.filter(call => call.path === "/offers" && call.method === "POST")).toHaveLength(0);
});

test("a buy offer warns on the USDT total and only for the verified paying wallet", async ({ page }) => {
  await setup(page, { signedIn: true, verifiedEthereum: true, balances: { ever: "1000", usdt: "2" } });
  await page.locator(".sideTabs").getByRole("button", { name: "Buy EVER", exact: true }).click();
  await page.getByTestId("otc-offer-ever").fill("100");
  await page.getByTestId("otc-offer-usdt").fill("11.889");
  const warning = page.getByTestId("otc-shortfall-warning");
  await expect(warning).toHaveCount(1);
  await expect(page.locator("#otc-receive")).toHaveClass(/shortfall/);
  await expect(page.locator("#otc-input")).not.toHaveClass(/shortfall/);
  await expect(warning).toHaveAttribute("data-tooltip", /Not enough USDT in your wallet — available 2 USDT/);
  await openOfferSettings(page);
  await page.getByTestId("otc-offer-recipient").fill(`0x${"c".repeat(40)}`);
  await expect(warning).toHaveCount(0);
});

for (const [label, balances] of [["unverified", { state: "unverified" as const }], ["unavailable", { state: "unavailable" as const }], ["a failed reader", { fail: true }]] as const) test(`a balance that is ${label} warns nothing and stays out of the errors`, async ({ page }) => {
  await setup(page, { signedIn: true, balances });
  await fill(page);
  await expect(page.getByTestId("otc-shortfall-warning")).toHaveCount(0);
  await expect(page.locator("#otc-input")).not.toHaveClass(/shortfall/);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(page.getByTestId("otc-publish-offer")).toBeEnabled();
});
