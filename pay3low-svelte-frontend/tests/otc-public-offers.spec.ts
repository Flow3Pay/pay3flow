import { expect, test, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

const ever = `0:${"a".repeat(64)}`, eth = `0x${"b".repeat(40)}`;
async function setup(page: Page, options: { signedIn?: boolean; lostResponse?: boolean; verifiedEthereum?: boolean; cancelledReplay?: boolean; failPublicRefresh?: boolean; failPrivateRefresh?: boolean } = {}) {
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
    if (path === "/config") response = { enabled: false, available: false, public_offers_available: true, public_offer_directions: ["buy","sell"], wallet_login_available: true, wallet_login_chains: ["everscale", "ethereum"], demo: false, desk: "", desk_name: "", ever_resource: "", usdt_resource: "" };
    else if (path === "/session/challenge") response = { id: "challenge", message: "Sign in only; no transfer", everscale_data: btoa("sign-in") };
    else if (path === "/session/proof" || path === "/session") response = session;
    else if (path === "/session/revoke") response = { revoked: true };
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
    else if (path === "/offers/history") { response = offers.map(({id,created_at,terms}) => ({id,created_at,terms:{direction:terms.direction || "sell",ever_amount:terms.ever_amount,usdt_amount:terms.usdt_amount}})); }
    else if (path.endsWith("/cancel")) { response = offers.find(offer => path.includes(offer.id)); response.state = "cancelled"; }
    await route.fulfill({ status, json: response });
  });
  await page.goto("/#/otc?market=EVER-USDT&side=sell");
  await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-state", "connected");
  await expect(page.locator(".bridgeForm .sideTabs").getByRole("button", {name:"Sell EVER",exact:true})).toHaveAttribute("aria-pressed", "true");
  return { calls, offers };
}
async function fill(page: Page) {
  await page.getByTestId("otc-offer-ever").fill("10.000000001");
  await page.getByTestId("otc-offer-usdt").fill("1.000001");
  await page.getByTestId("otc-offer-recipient").fill(eth);
}

test("seller signs in with EVER alone and publishes, restores, and cancels a real-network advertisement", async ({ page }) => {
  const f = await setup(page);
  await fill(page);
  await page.getByRole("button", { name: "Connect EVER Wallet to publish", exact: true }).click();
  await expect(page.getByTestId("otc-publish-offer")).toBeVisible();
  await expect(page.getByTestId("otc-offer-ever")).toHaveValue("10.000000001");
  await expect(page.getByTestId("otc-offer-recipient")).toHaveValue(eth);
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByTestId("otc-offer-review")).toContainText(ever);
  expect(f.calls.filter(call => call.path === "/offers" && call.method === "POST")).toHaveLength(0);
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.locator(".serviceControls [role=status]")).toContainText("Your EVER sale offer is published.");
  await expect(page.getByTestId("otc-public-offer")).toContainText("10.000000001 EVER → 1.000001 USDT");
  expect(f.calls.find(call => call.path === "/session/challenge")?.body.chain).toBe("everscale");
  expect(f.calls.find(call => call.path === "/offers" && call.method === "POST")?.body).toEqual({ direction:"sell", ever_amount: "10.000000001", usdt_amount: "1.000001", usdt_address: eth });
  expect(f.calls.some(call => /rfqs|attempts|prepare|decision/.test(call.path))).toBe(false);
  expect(await page.evaluate(() => (window as any).everCalls)).toContain("signData");
  expect(await page.evaluate(() => (window as any).everCalls)).not.toContain("sendMessage");
  await page.reload();
  await expect(page.getByTestId("otc-public-offer")).toBeVisible();
  await page.getByTestId("otc-own-offer").getByRole("button", { name: "Cancel my offer", exact: true }).click();
  await expect(page.getByTestId("otc-public-offer")).toHaveCount(0);
  await page.getByRole("tab", { name:"History", exact:true }).click();
  await expect(page.getByTestId("otc-own-offer")).toContainText("Cancelled");
});

for (const direction of ["sell","buy"] as const) test(`a lost ${direction} publication restores the exact draft and reuses its key after reload`, async ({ page }) => {
  const f = await setup(page, { signedIn: true, lostResponse: true });
  await page.locator(".sideTabs").getByRole("button",{name:direction === "buy" ? "Buy EVER" : "Sell EVER",exact:true}).click();
  await fill(page);
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByRole("alert")).toHaveText("Publication response lost");
  await page.reload();
  await expect(page.getByTestId("otc-offer-ever")).toHaveValue("10.000000001");
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByTestId("otc-public-offer")).toHaveCount(1);
  const sent = f.calls.filter(call => call.path === "/offers" && call.method === "POST");
  expect(sent).toHaveLength(2); expect(sent[0].key).toBeTruthy(); expect(sent[1].key).toBe(sent[0].key);
  expect(sent[1].body).toEqual(sent[0].body);
});

test("publication rejects invalid precision and zero recipients, and fits an accessible 320px screen", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 820 });
  await setup(page, { signedIn: true }); await fill(page);
  for (const value of ["0", "1.0000000001", "-10", "100001"]) {
    await page.getByTestId("otc-offer-ever").fill(value);
    await expect(page.getByTestId("otc-publish-offer")).toBeDisabled();
  }
  await fill(page); await page.getByTestId("otc-offer-recipient").fill(`0x${"0".repeat(40)}`);
  await expect(page.getByTestId("otc-publish-offer")).toBeDisabled();
  await fill(page); await expect(page.getByTestId("otc-publish-offer")).toBeEnabled();
  await page.getByTestId("otc-publish-offer").click();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  expect((await new AxeBuilder({ page }).include(".otcWorkspace").withTags(["wcag2a", "wcag2aa"]).analyze()).violations).toEqual([]);
});

test("receiving address can be obtained from Ethereum without a USDT balance or a signature", async ({ page }) => {
  await setup(page, { signedIn: true });
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
  await page.getByTestId("otc-offer-recipient").fill(eth);
  await expect(page.getByRole("textbox", { name:"Price", exact:true })).toHaveValue("0.0011889");
  await expect(page.locator(".bridgeColumn .bridgeForm")).toBeVisible();
  await expect(page.locator(".chartColumn")).toContainText("Public asking prices");
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByTestId("otc-offer-review")).toContainText("0.0011889 USDT / EVER");
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.locator(".bookColumn")).toContainText("100 EVER → 0.11889 USDT");
  await expect(page.locator(".chartColumn circle")).toHaveCount(1);
  await page.getByRole("button", { name:"Collapse Orderbook", exact:true }).click();
  await expect(page.locator(".bookColumn")).toHaveCount(0);
  await page.setViewportSize({width:1440,height:1000});
  await expect.poll(async () => page.evaluate(() => {
    const grid=document.querySelector(".otcGrid")!.getBoundingClientRect();
    const chart=document.querySelector(".chartColumn")!.getBoundingClientRect();
    return Math.abs(grid.right-chart.right);
  })).toBeLessThan(1);
  await page.getByRole("button", { name:"Expand Orderbook", exact:true }).click();
  await expect(page.getByRole("group", {name:"OTC mode",exact:true})).toHaveCount(0);
  await expect(page.locator(".otcGrid")).toHaveCount(1);
  await expect(page.locator(".bridgeForm")).toHaveCount(1);
  await page.locator(".bridgeForm .sideTabs").getByRole("button", { name:"Buy EVER", exact:true }).click();
  await expect(page.getByTestId("otc-publish-offer")).toBeVisible();
  await expect(page.locator("#otc-input")).toHaveAttribute("data-testid","otc-offer-ever");
  await page.locator(".bridgeForm .sideTabs").getByRole("button", { name:"Sell EVER", exact:true }).click();
  await expect(page.getByTestId("otc-offer-ever")).toHaveValue("100");
  await expect(page.getByTestId("otc-offer-usdt")).toHaveValue("0.11889");
  await expect(page.getByTestId("otc-public-offer")).toContainText("100 EVER → 0.11889 USDT");
  await expect(page.getByTestId("otc-own-offer")).toContainText("100 EVER → 0.11889 USDT");
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
  await expect(page.locator("#otc-sale-status")).toContainText("Ethereum wallet address");
  await page.getByTestId("otc-offer-recipient").fill(`  ${eth}  `);
  await expect(page.getByTestId("otc-publish-offer")).toBeEnabled();
  await expect(page.locator("#otc-sale-status")).toHaveCount(0);
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByTestId("otc-offer-review")).toContainText(`${direction === "buy" ? "Buy" : "Sell"} 100 EVER · 11.889 USDT`);
  expect(f.calls.filter(c=>c.path === "/offers" && c.method === "POST")).toHaveLength(0);
  await page.getByTestId("otc-publish-offer").click();
  const offer=page.locator(".bookColumn").getByTestId("otc-public-offer");
  await expect(offer).toContainText(direction === "buy" ? "Buy EVER" : "Sell EVER");
  await expect(offer).toContainText(direction === "buy" ? "11.889 USDT → 100 EVER" : "100 EVER → 11.889 USDT");
  expect(f.calls.find(c=>c.path === "/offers" && c.method === "POST")?.body).toEqual({direction,ever_amount:"100",usdt_amount:"11.889",usdt_address:eth});
  await offer.getByRole("button",{name:"Cancel my offer",exact:true}).click();
  await expect(offer).toHaveCount(0);
  await page.getByRole("tab",{name:"History",exact:true}).click();
  await expect(page.getByTestId("otc-own-offer")).toContainText("Cancelled");
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
  await page.getByTestId("otc-offer-recipient").fill(eth);
  await expect(page.getByTestId("otc-publish-offer")).toBeEnabled();
  await page.getByTestId("otc-offer-price").fill("0");
  await expect(page.getByTestId("otc-publish-offer")).toBeDisabled();
  await expect(page.locator("#otc-sale-status")).toContainText("positive price");
});


test("an already configured Ethereum wallet fills the address without a second connection", async ({ page }) => {
  const f=await setup(page,{signedIn:true,verifiedEthereum:true});
  await page.getByTestId("otc-offer-price").fill("0.1");
  await page.getByTestId("otc-offer-ever").fill("100");
  await expect(page.getByTestId("otc-offer-recipient")).toHaveValue(eth);
  await expect(page.getByTestId("otc-publish-offer")).toBeEnabled();
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByTestId("otc-offer-review")).toContainText("Sell 100 EVER · 10 USDT");
  expect(f.calls.some(c=>c.path === "/offers" && c.method === "POST")).toBe(false);
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
  await expect(page.getByTestId("otc-offer-equation")).toContainText("10 EVER × 0.205 USDT / EVER = 2.05 USDT");
  await page.getByTestId("otc-offer-recipient").fill(eth);
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-publish-offer").click();
  const row=page.locator(".bookColumn").getByTestId("otc-public-offer");
  await expect(row.getByTestId("otc-book-price")).toHaveText("0.205");
  await expect(row.locator(".offerNumbers")).toContainText("2.05");
  await expect(page.getByTestId("otc-chart-readout")).toContainText("0.205 USDT / EVER");
  await expect(page.getByTestId("otc-chart-price")).toHaveCount(5);
  await expect(page.locator(".chartColumn polyline")).toHaveCount(0);
  expect(f.calls.find(c=>c.method === "POST" && c.path === "/offers")?.body).toEqual({direction,ever_amount:"10",usdt_amount:"2.05",usdt_address:eth});
  await page.reload();
  await expect(page.locator(".bookColumn").getByTestId("otc-book-price")).toHaveText("0.205");
  await expect(page.getByTestId("otc-chart-readout")).toContainText("0.205 USDT / EVER");
});

test("a cancelled publication replay is never reported as a newly published offer",async({page})=>{
  const f=await setup(page,{signedIn:true,cancelledReplay:true});
  await fill(page);
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByRole("alert")).toContainText("already cancelled or expired");
  await expect(page.locator(".serviceControls [role=status]")).toHaveCount(0);
  await expect(page.locator(".bookColumn").getByTestId("otc-public-offer")).toHaveCount(0);
  await expect(page.getByTestId("otc-publish-offer")).toHaveText(/Review offer/);
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.locator(".serviceControls [role=status]")).toContainText("is published");
  await expect(page.locator(".bookColumn").getByTestId("otc-public-offer")).toHaveCount(1);
  const posts=f.calls.filter(c=>c.method === "POST" && c.path === "/offers");
  expect(posts).toHaveLength(2);expect(posts[0].key).not.toBe(posts[1].key);
});

test("a saved publication populates the book and chart even if the following public refresh fails",async({page})=>{
  await setup(page,{signedIn:true,failPublicRefresh:true});
  await fill(page);
  await page.getByTestId("otc-publish-offer").click();
  await page.getByTestId("otc-publish-offer").click();
  await expect(page.getByRole("alert")).toHaveText("Public offer refresh unavailable");
  await expect(page.locator(".bookColumn").getByTestId("otc-public-offer")).toHaveCount(1);
  await expect(page.locator(".chartColumn circle")).toHaveCount(1);
  await page.locator(".bookColumn").getByRole("button",{name:"Cancel my offer",exact:true}).click();
  await expect(page.locator(".bookColumn").getByTestId("otc-public-offer")).toHaveCount(0);
  await expect(page.locator(".chartColumn circle")).toHaveCount(1);
});

test("public book loads despite a failed private history, with asks ascending and bids descending",async({page})=>{
  const f=await setup(page,{signedIn:true,failPrivateRefresh:true});
  for(const [id,direction,amount,total] of [["ask-high","sell","10","3"],["bid-low","buy","10","1"],["ask-low","sell","10","2"],["bid-high","buy","10","1.5"]]){
    f.offers.push({id,seller_ever:ever,state:"open",terms:{direction,ever_amount:amount,usdt_amount:total,usdt_address:eth},created_at:new Date().toISOString(),expires_at:"2099-01-01T00:00:00Z"});
  }
  await page.reload();
  await expect(page.getByRole("alert")).toHaveText("Private offer list unavailable");
  await expect(page.locator(".bookColumn").getByTestId("otc-book-price")).toHaveText(["0.2","0.3","0.15","0.1"]);
  await expect(page.locator(".chartColumn circle")).toHaveCount(4);
});
