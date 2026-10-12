import { test, expect, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

// All chain and desk responses here are simulations; no wallet signs or sends funds.
const desk = "https://relay.example/actors/desk", customer = "https://relay.example/actors/customer";
const ever = "https://assets.example/ever", usdt = "https://assets.example/usdt";
function makeTrade(state = "quoted", direction = "buy", id = "booking-fixture") {
  return { id, created_at: new Date().toISOString(), rfq_id: "rfq-fixture", state, offer: {}, decision: {}, terms: {
    demo: false, direction, input: "100", output: "200", input_units: direction === "buy" ? "100000000" : "100000000000", output_units: direction === "buy" ? "200000000000" : "200000000",
    customer_actor: customer, desk_actor: desk, quote_by: "2099-10-10T00:02:00Z", pay_by: "2099-10-10T00:07:00Z", payout_by: "2099-10-10T00:17:00Z",
    network_costs: "Sender pays gas separately", ever_receiving_cost_units: "1000000", refund_policy: "Refund to proved booked wallet after reconciliation",
    fee_policy: { basis_units: direction === "buy" ? "100000000" : "200000000", fee_units: "5000000" }, proposal: { id: `${desk}/quotes/${id}`, publishes: { resourceConformsTo: direction === "buy" ? ever : usdt }, reciprocal: { resourceConformsTo: direction === "buy" ? usdt : ever } }
  }};
}
async function fixtures(page: Page, options: { signedIn?: boolean; available?: boolean; state?: string; direction?: string; operator?: boolean; demo?: boolean; extra?: ReturnType<typeof makeTrade>[]; lostPrepare?: boolean; lostBooking?: boolean } = {}) {
  const trades = options.state ? [makeTrade(options.state, options.direction), ...(options.extra || [])] : options.extra || [];
  const attempts: any[] = [], rfqs: any[] = [], requests: { path: string; method: string; body: any; key?: string }[] = [];
  const control = { holdRfq: false, releaseRfq: null as null | (() => void), holdPrivate: false, releasePrivate: null as null | (() => void), enabled: true, walletChains: ["ethereum", "everscale"], available: options.available ?? true, outage: false, sessionExpired: false, wallets: ["ethereum: 0xfixture", "everscale: 0:fixture"] };
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.addInitScript(({ customer, desk, signedIn, operator }) => {
    localStorage.setItem("pay3flow-locale", "en");
    if (signedIn) {
      const actor = operator ? desk : customer;
      sessionStorage.setItem("pay3flow.otc.session.v1", JSON.stringify({ token: "simulation-token", actor, desk: !!operator }));
      sessionStorage.setItem("pay3flow.otc.wallets", JSON.stringify({ actor, bound: ["ethereum: 0xfixture", "everscale: 0:fixture"] }));
    }
  }, { customer, desk, signedIn: options.signedIn, operator: options.operator });
  await page.route("**/api/otc/**", async route => {
    const req = route.request(), path = new URL(req.url()).pathname.replace("/api/otc", "");
    const body = req.method() === "POST" ? req.postDataJSON() : null;
    requests.push({ path, method: req.method(), body, key: req.headers()["idempotency-key"] });
    let response: unknown = {}, status = 200;
    if (path === "/config") { if (control.outage) status = 503; response = { enabled: control.enabled, wallet_login_available: true, wallet_login_chains: control.walletChains, available: control.available, demo: !!options.demo, desk, desk_name: "Fixture desk", ever_resource: ever, usdt_resource: usdt, counterparty_risk: "Two separate transfers" }; }
    else if (path === "/listings") response = ["buy", "sell"].map(direction => ({ id: `${desk}/listings/${direction}`, name: `${direction} EVER`, publishes: { resourceConformsTo: direction === "buy" ? ever : usdt }, reciprocal: { resourceConformsTo: direction === "buy" ? usdt : ever } }));
    else if (path === "/session" && req.method() === "GET") {
      status = control.sessionExpired ? 401 : 200;
      response = control.sessionExpired ? { error: "OTC authentication required" } : { token: "simulation-token", actor: options.operator ? desk : customer, desk: !!options.operator, expires_at: "2099-10-10T00:00:00Z", signing_wallet: null, wallets: control.wallets.map(value => ({ chain: value.split(": ")[0], address: value.split(": ")[1], verified_at: new Date().toISOString() })) };
    }
    else if (path === "/trades") { response = structuredClone({ trades, rfqs }); if (control.holdPrivate) { control.holdPrivate = false; await new Promise<void>(resolve => control.releasePrivate = resolve); } }
    else if (path === "/rfqs") { rfqs.push({ id: "rfq-fixture", state: "waiting", actor: customer, ...body }); response = { id: "rfq-fixture" }; if (control.holdRfq) { control.holdRfq = false; await new Promise<void>(resolve => control.releaseRfq = resolve); } }
    else if (path.endsWith("/apply")) { trades[0].state = options.lostBooking ? "quoted" : "booking_pending"; if (options.lostBooking) { status = 503; response = { error: "Lost booking response" }; } }
    else if (path.endsWith("/decision")) trades[0].state = body.accept ? "accepted" : "rejected";
    else if (path.endsWith("/prepare")) {
      trades[0].state = body.kind === "payment" ? "payment_confirming" : "payout_pending";
      const attempt = { id: "attempt-fixture", kind: body.kind, state: "unknown", reference: null, instructions: { leg: { chain: "ethereum", amount: "100000000", transfer_amount: "100000000", sender: `0x${"b".repeat(40)}`, recipient: `0x${"a".repeat(40)}` }, call: { family: "evm", nonce: "0x3" } } };
      attempts.push(attempt); response = options.lostPrepare ? { error: "Lost prepare response" } : attempt; if (options.lostPrepare) status = 503;
    } else if (path.startsWith("/trades/")) response = { trade: trades.find(t => path === `/trades/${t.id}`), attempts, evidence: [], customer_updates: [] };
    else if (path === "/desk/dashboard") response = { quote_coverage: { numerator: 0, denominator: 0, rate: null }, on_time_payout: { numerator: 0, denominator: 0, rate: null }, open_funded_cases: [], economics: { collected_usdt_units: "0", unpaid_usdt_units: "0" }, observers: [], incidents: [] };
    await route.fulfill({ status, json: response });
  });
  return { trades, attempts, rfqs, requests, control };
}
async function open(page: Page) { await page.goto("/#/otc"); await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-state", "connected"); }
async function select(page: Page, state: string, direction = "buy") { await page.getByRole("button", { name: new RegExp(`${direction === "buy" ? "Buy" : "Sell"} EVER 100 → 200 ${state}`) }).click(); }

test("one real terminal, scoped networks, no demo feed, accessible at 320 pixels", async ({ page }) => {
  const sockets: string[] = []; page.on("websocket", socket => sockets.push(socket.url()));
  await page.setViewportSize({ width: 320, height: 780 }); await fixtures(page, { available: false }); await open(page);
  await expect(page.locator(".settlementSection")).toHaveCount(0);
  await expect(page.getByText("EVER / USDT · OTC desk settlement", { exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Request exact quote" })).toBeEnabled();
  await expect(page.getByText("No quote history yet", { exact: true })).toBeVisible();
  await page.locator("#otc-bridge").getByRole("button", { name: "Sell EVER", exact: true }).click();
  await expect(page.getByLabel("You send · EVER on Everscale", { exact: true })).toBeVisible();
  await expect(page.getByLabel("You receive · USDT on Ethereum", { exact: true })).toBeVisible();
  await expect(page.locator("#otc-receive")).toHaveValue("");
  expect(sockets.some(url => url.includes("/ws/otc"))).toBe(false);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
  expect((await new AxeBuilder({ page }).include(".otcWorkspace").withTags(["wcag2a", "wcag2aa"]).analyze()).violations).toEqual([]);
});

for (const direction of ["buy", "sell"]) test(`exact ${direction} RFQ uses the original structured Proposal and stable retry key`, async ({ page }) => {
  const f = await fixtures(page, { signedIn: true }); await open(page);
  await page.locator("#otc-bridge").getByRole("button", { name: direction === "buy" ? "Buy EVER" : "Sell EVER", exact: true }).click();
  await page.locator("#otc-input").fill("00100.000000"); await page.getByRole("button", { name: "Request exact quote" }).click();
  await expect(page.getByText(/Quote requested\. Reference/)).toBeVisible();
  await page.getByRole("button", { name: "Request exact quote" }).click(); await expect.poll(() => f.requests.filter(r => r.path === "/rfqs").length).toBe(2);
  const sent = f.requests.filter(r => r.path === "/rfqs"); expect(sent[0].body).toEqual({ direction, input: "100", listing_id: `${desk}/listings/${direction}` }); expect(sent[0].key).toBeTruthy(); expect(sent[1].key).toBe(sent[0].key);
  expect(new URL(page.url()).hash).not.toMatch(/amount|price|booking|rfq/);
});

test("private fixed quote invalidates on edit; accepted terms stay fixed and restore", async ({ page }) => {
  const f = await fixtures(page, { signedIn: true, state: "quoted" }); await open(page); await select(page, "quoted");
  await expect(page.locator("#otc-receive")).toHaveValue("200"); await expect(page.locator(".detail")).toContainText("0.5 USDT per EVER");
  await page.locator("#otc-input").fill("101"); await expect(page.locator("#otc-receive")).toHaveValue(""); await expect(page.getByRole("button", { name: "Accept fixed quote" })).toHaveCount(0);
  await select(page, "quoted"); await page.getByRole("button", { name: "Accept fixed quote" }).click();
  await expect(page.getByRole("heading", { name: "Buy EVER · booking pending" })).toBeVisible();
  expect(f.requests.find(r => r.path.endsWith("/apply"))?.body).toEqual({});
  f.trades[0].state = "accepted"; await page.reload(); await expect(page.getByRole("heading", { name: "Buy EVER · accepted" })).toBeVisible();
  await page.locator("#otc-bridge").getByRole("button", { name: "Sell EVER", exact: true }).click(); await page.locator("#otc-input").fill("999");
  await expect(page.locator(".detail")).toContainText("100 USDT · Ethereum"); await expect(page.locator(".detail")).toContainText("200 EVER · Everscale");
  await expect(page.getByText("5 USDT · paid by desk", { exact: true })).toBeVisible();
  for (const deadline of ["Quote expires", "Payment inclusion deadline", "Payout deadline"]) await expect(page.locator(".detail").getByText(deadline, { exact: true })).toBeVisible();
});

for (const lostPrepare of [false, true]) test(`persisted payment handoff blocks resend after reload (${lostPrepare ? "lost" : "received"} response)`, async ({ page }) => {
  const f = await fixtures(page, { signedIn: true, state: "accepted", lostPrepare }); await open(page); await select(page, "accepted");
  await page.getByRole("button", { name: "Review and send customer payment" }).click();
  if (!lostPrepare) { const review = page.getByRole("dialog", { name: "Review transfer" }); await expect(review).toContainText("100 USDT"); await expect(review.getByRole("button", { name: "Confirm in wallet" })).toBeEnabled(); await review.getByRole("button", { name: "Close and track saved transfer" }).click(); }
  await expect(page.getByText("Original result unknown. Another send is blocked.", { exact: true })).toBeVisible();
  await page.reload(); await expect(page.getByText("Original result unknown. Another send is blocked.", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Review and send customer payment" })).toHaveCount(0); await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(f.requests.filter(r => r.path.endsWith("/prepare"))).toHaveLength(1);
});

test("unknown booking blocks another selection and reconciles the original offer", async ({ page }) => {
  const f = await fixtures(page, { signedIn: true, state: "quoted", lostBooking: true }); await open(page); await select(page, "quoted"); await page.getByRole("button", { name: "Accept fixed quote" }).click();
  await expect(page.getByText(/The original booking is awaiting/)).toBeVisible(); await expect(page.getByRole("button", { name: "Accept fixed quote" })).toBeDisabled();
  await page.reload(); await expect(page.getByRole("button", { name: "Review fixed quote" })).toBeDisabled(); await page.getByRole("button", { name: "Reconcile original booking" }).click();
  await expect.poll(() => f.requests.filter(r => r.path.endsWith("/apply")).length).toBe(2); expect(f.requests.filter(r => r.path.endsWith("/apply")).every(r => r.path === "/trades/booking-fixture/apply" && JSON.stringify(r.body) === "{}")).toBe(true);
});

test("expired and rejected quotes cannot enable payment; no customer desk controls", async ({ page }) => {
  const f = await fixtures(page, { signedIn: true, state: "quoted" }); f.trades[0].terms.quote_by = "2000-01-01T00:00:00Z"; await open(page); await select(page, "quoted");
  await expect(page.getByRole("button", { name: "Accept fixed quote" })).toBeDisabled(); await expect(page.getByRole("button", { name: "Review and send customer payment" })).toHaveCount(0); await expect(page.getByRole("heading", { name: "Desk console" })).toHaveCount(0);
  f.trades[0].state = "rejected"; await page.reload(); await expect(page.getByRole("heading", { name: "Buy EVER · rejected" })).toBeVisible(); await expect(page.getByRole("button", { name: "Accept fixed quote" })).toHaveCount(0);
});

test("desk console decisions reserve first and unknown payout blocks refund", async ({ page }) => {
  const f = await fixtures(page, { signedIn: true, operator: true, state: "booking_pending" }); await open(page); await select(page, "booking pending");
  await expect(page.getByRole("heading", { name: "Desk console" })).toBeVisible(); await expect(page.getByText("0/0 · N/A").first()).toBeVisible();
  await page.getByRole("button", { name: "Reserve and accept" }).click(); await expect(page.getByRole("heading", { name: "Buy EVER · accepted" })).toBeVisible();
  expect(f.requests.find(r => r.path.endsWith("/decision"))?.body).toEqual({ accept: true });
  f.trades[0].state = "funded"; await page.reload(); await page.getByRole("button", { name: "Review and send desk payout" }).click(); await page.getByRole("button", { name: "Close and track saved transfer" }).click();
  await expect(page.getByRole("button", { name: "Review validated refund" })).toHaveCount(0); await expect(page.getByRole("button", { name: "Review and send desk payout" })).toHaveCount(0); expect(f.attempts[0].kind).toBe("payout");
});

test("server history and volume count completed USDT legs once, refunds stay visible", async ({ page }) => {
  await fixtures(page, { signedIn: true, extra: [makeTrade("completed", "buy", "done-buy"), makeTrade("completed", "sell", "done-sell"), makeTrade("refunded", "buy", "refund")] }); await open(page);
  await expect(page.locator(".volumeStat")).toContainText("300 USDT"); await expect(page.locator("#otc-chart circle")).toHaveCount(3);
  await page.getByRole("tab", { name: "History" }).click(); await expect(page.locator(".tradeList button")).toHaveCount(3); await expect(page.locator(".tradeList")).toContainText("refunded");
  await page.getByRole("tab", { name: "History" }).press("Home"); await expect(page.getByRole("tab", { name: "Desk offers" })).toHaveAttribute("aria-selected", "true");
});

test("logout discards a delayed participant response", async ({ page }) => {
  const f = await fixtures(page, { signedIn: true, state: "accepted" }); await open(page); await select(page, "accepted"); f.control.holdPrivate = true;
  await expect.poll(() => !!f.control.releasePrivate, { timeout: 12000 }).toBe(true);
  await page.locator(".walletMenu .walletToggle").click(); await page.getByRole("button", { name: "Disconnect", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Buy EVER · accepted" })).toHaveCount(0); f.control.releasePrivate!();
  await page.waitForTimeout(250); await expect(page.locator(".tradeList button")).toHaveCount(0); await expect(page.locator("#otc-chart circle")).toHaveCount(0);
});

test("backend outage pauses new RFQs; panel layout survives reload", async ({ page }) => {
  const f = await fixtures(page, { signedIn: true }); await open(page); await page.getByRole("button", { name: "Collapse Exchange", exact: true }).click(); await page.reload();
  await expect(page.getByRole("button", { name: "Expand Exchange", exact: true })).toBeVisible(); await page.getByRole("button", { name: "Expand Exchange", exact: true }).click();
  f.control.outage = true; await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-state", "unavailable", { timeout: 12000 }); await expect(page.getByRole("button", { name: "Request exact quote" })).toBeDisabled();
});

test("navigation and public sharing retain the terminal without exposing private terms", async ({ page }, testInfo) => {
  const errors: string[] = []; page.on("pageerror", error => errors.push(error.message));
  await fixtures(page, { signedIn: true, state: "quoted" }); await open(page); await select(page, "quoted");
  const sent = page.waitForRequest(request => new URL(request.url()).pathname === "/share-links" && request.method() === "POST");
  await page.getByRole("button", { name: "Share OTC", exact: true }).click();
  const payload = (await sent).postDataJSON(); expect(payload).toEqual({ target: "/#/otc?market=EVER-USDT&side=buy&lang=en" });
  await expect(page.getByRole("dialog", { name: "Share OTC", exact: true })).toBeVisible(); await page.keyboard.press("Escape");
  await page.screenshot({ path: testInfo.outputPath("real-otc-terminal-simulation.png"), fullPage: true });
  await page.getByRole("link", { name: "SWAP", exact: true }).click(); await expect(page.getByTestId("otc-workspace")).toHaveCount(0);
  await page.getByRole("link", { name: "OTC", exact: true }).click(); await expect(page.getByRole("heading", { name: "Buy EVER · quoted" })).toBeVisible();
  expect(errors).toEqual([]);
});

async function walletLoginFixtures(page: Page, reject = false) {
  const f = await fixtures(page);
  f.control.wallets = [];
  const evm = `0x${"b".repeat(40)}`, ever = `0:${"e".repeat(64)}`;
  await page.addInitScript(({ evm, ever, reject }) => {
    const w = window as any;
    w.otcWalletCalls = [];
    const listeners = new Map<string, Set<(...args: any[]) => void>>();
    w.ethereum = {
      request: async ({ method, params }: any) => {
        w.otcWalletCalls.push({ method, params });
        if (method === "eth_requestAccounts" || method === "eth_accounts") return [evm];
        if (method === "eth_chainId") return "0x1";
        if (method === "wallet_requestPermissions") return [{ parentCapability: "eth_accounts" }];
        if (method === "personal_sign") {
          if (reject) throw Object.assign(new Error("User rejected sign-in"), { code: 4001 });
          return `0x${"a".repeat(130)}`;
        }
        throw new Error(`Unexpected EVM request: ${method}`);
      },
      on: (event: string, callback: (...args: any[]) => void) => { if (!listeners.has(event)) listeners.set(event, new Set()); listeners.get(event)!.add(callback); },
      off: (event: string, callback: (...args: any[]) => void) => listeners.get(event)?.delete(callback),
    };
    w.__hasEverscaleProvider = true;
    w.__ever = {
      request: async ({ method, params }: any) => {
        w.otcWalletCalls.push({ method, params });
        const permissions = { accountInteraction: { address: ever, publicKey: "11".repeat(32) } };
        if (method === "getProviderState") return { selectedConnection: "mainnet", networkId: 42, permissions };
        if (method === "requestPermissions") return permissions;
        if (method === "signData") { if (reject) throw new Error("User rejected sign-in"); return { signature: "fixture-signature" }; }
        throw new Error(`Unexpected EVER request: ${method}`);
      }, addListener: () => {}, removeListener: () => {},
    };
  }, { evm, ever, reject });
  const auth: { path: string; body: any }[] = [];
  let selectedChain = "ethereum", selectedAddress = evm;
  for (const path of ["/session/challenge", "/session/proof", "/wallet/challenge", "/wallet/proof"]) {
    await page.route(`**/api/otc${path}`, async route => {
      const body = route.request().postDataJSON(); auth.push({ path, body });
      let response: unknown;
      if (path.endsWith("/challenge")) {
        selectedChain = body.chain; selectedAddress = body.address;
        response = { id: `fixture-${selectedChain}`, message: `Pay3Flow OTC sign-in: ${selectedChain} ${selectedAddress}`, everscale_data: "Zml4dHVyZQ==" };
      } else {
        f.control.wallets = [...f.control.wallets.filter(value => !value.startsWith(`${selectedChain}:`)), `${selectedChain}: ${selectedAddress}`];
        response = path === "/wallet/proof" ? { address: selectedAddress } : { token: "simulation-token", actor: customer, desk: false, expires_at: "2099-10-10T00:00:00Z", signing_wallet: { chain: selectedChain, address: selectedAddress }, wallets: f.control.wallets.map(value => ({ chain: value.split(": ")[0], address: value.split(": ")[1], verified_at: new Date().toISOString() })) };
      }
      await route.fulfill({ json: response });
    });
  }
  return { ...f, auth };
}

for (const chain of ["ethereum", "everscale"]) test(`OTC ${chain} wallet sign-in replaces actor fields and verifies the other wallet`, async ({ page }) => {
  const f = await walletLoginFixtures(page); await open(page);
  await expect(page.locator(".connectionBar")).toHaveCount(0);
  await expect(page.getByText("Route paused", { exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).otcWalletCalls.some((call: any) => ["eth_requestAccounts", "requestPermissions", "personal_sign", "signData"].includes(call.method)))).toBe(false);
  if (chain === "ethereum") await page.locator(".walletMenu .walletToggle").click();
  else await page.locator("#otc-bridge").getByRole("button", { name: "Request exact quote", exact: true }).click();
  const dialog = page.locator("#wallet-connections");
  await dialog.evaluate(node => node.dataset.quoteFlow = "original");
  await expect(dialog.getByRole("searchbox", { name: "Search wallets" })).toBeVisible();
  await expect(dialog.getByText("LF credential", { exact: true })).toHaveCount(0);
  await dialog.getByRole("button", { name: chain === "ethereum" ? "Connect Ethereum wallet" : "Connect Everscale wallet", exact: true }).click();
  await expect(dialog.getByRole("heading", { name: "Wallet connections", exact: true })).toBeVisible();
  await expect(dialog.getByText("Verified", { exact: true })).toHaveCount(1);
  const other = chain === "ethereum" ? "everscale" : "ethereum";
  await dialog.getByRole("button", { name: other === "ethereum" ? "Verify Ethereum wallet" : "Verify Everscale wallet", exact: true }).click();
  await expect(dialog.getByText("Verified", { exact: true })).toHaveCount(2);
  await expect(dialog).toHaveAttribute("data-quote-flow", "original");
  expect(f.requests.filter(request => request.path === "/rfqs")).toHaveLength(0);
  await expect(page.getByRole("dialog")).toHaveCount(1);
  if (chain === "ethereum") {
    await expect(dialog.getByRole("button", { name: "Request exact quote", exact: true })).toHaveCount(0);
    await dialog.locator(".closeButton").click();
    await page.locator("#otc-bridge").getByRole("button", { name: "Request exact quote", exact: true }).click();
  } else await dialog.getByRole("button", { name: "Request exact quote", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByText(/Quote requested/)).toBeVisible();
  expect(f.auth.filter(request => request.path === "/session/proof")).toHaveLength(1);
  expect(f.auth.find(request => request.path === "/session/challenge")?.body.chain).toBe(chain);
  await page.reload();
  await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-state", "connected");
  await page.locator(".walletMenu .walletToggle").click();
  await expect(page.locator("#wallet-connections").getByText("Verified", { exact: true })).toHaveCount(2);
});

for (const chain of ["ethereum", "everscale"]) test(`rejected ${chain} signature leaves OTC signed out`, async ({ page }) => {
  const f = await walletLoginFixtures(page, true); await open(page);
  await page.locator("#otc-bridge").getByRole("button", { name: "Request exact quote", exact: true }).click();
  const dialog = page.locator("#wallet-connections");
  await dialog.getByRole("button", { name: chain === "ethereum" ? "Connect Ethereum wallet" : "Connect Everscale wallet", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText(/rejected/i);
  await expect(dialog.getByRole("heading", { name: "Wallet connections", exact: true })).toBeVisible();
  expect(f.auth.filter(request => request.path === "/session/proof")).toHaveLength(0);
  expect(await page.evaluate(() => sessionStorage.getItem("pay3flow.otc.session.v1"))).toBeNull();
});

test("server wallet status replaces cached proofs and session expiry clears private views", async ({ page }) => {
  const f = await fixtures(page, { signedIn: true, state: "accepted" });
  f.control.wallets = []; await open(page);
  await page.locator(".walletMenu .walletToggle").click();
  await expect(page.locator("#wallet-connections").getByText("Verified", { exact: true })).toHaveCount(0);
  await page.locator("#wallet-connections .closeButton").click();
  await select(page, "accepted");
  await expect(page.getByRole("heading", { name: "Buy EVER · accepted", exact: true })).toBeVisible();
  f.control.sessionExpired = true;
  await expect(page.getByRole("heading", { name: "Buy EVER · accepted", exact: true })).toHaveCount(0, { timeout: 12000 });
  await expect(page.locator(".walletMenu .walletToggle")).toBeVisible();
  await expect(page.locator("#otc-bridge").getByRole("button", { name: "Request exact quote", exact: true })).toBeEnabled();
  expect(await page.evaluate(() => sessionStorage.getItem("pay3flow.otc.session.v1"))).toBeNull();
});


test("logout waits for a submitted RFQ while pending reads can still be discarded", async ({ page }) => {
  const f = await fixtures(page, { signedIn: true }); await open(page);
  f.control.holdRfq = true;
  await page.getByRole("button", { name: "Request exact quote", exact: true }).click();
  await expect.poll(() => !!f.control.releaseRfq).toBe(true);
  await page.locator(".walletMenu .walletToggle").click();
  const dialog = page.locator("#wallet-connections");
  await expect(dialog.getByRole("button", { name: "Disconnect", exact: true })).toBeDisabled();
  f.control.releaseRfq!();
  await expect(dialog.getByRole("button", { name: "Disconnect", exact: true })).toBeEnabled();
  await dialog.getByRole("button", { name: "Disconnect", exact: true }).click();
  await expect(page.locator(".walletMenu .walletToggle")).toBeVisible();
  await expect(page.locator("#otc-bridge").getByRole("button", { name: "Request exact quote", exact: true })).toBeEnabled();
  expect(await page.evaluate(() => sessionStorage.getItem("pay3flow.otc.session.v1"))).toBeNull();
});


test("wallet sign-in remains available when new trading is disabled", async ({ page }) => {
  const f = await walletLoginFixtures(page);
  f.control.enabled = false; f.control.available = false; await open(page);
  await page.locator("#otc-bridge").getByRole("button", { name: "Request exact quote", exact: true }).click();
  const dialog = page.locator("#wallet-connections");
  await dialog.getByRole("button", { name: "Connect Ethereum wallet", exact: true }).click();
  await expect(dialog.getByRole("heading", { name: "Wallet connections", exact: true })).toBeVisible();
  await dialog.getByRole("button", { name: "Verify Everscale wallet", exact: true }).click();
  await expect(dialog.getByText("Verified", { exact: true })).toHaveCount(2);
  await expect(dialog.getByRole("button", { name: "Request exact quote", exact: true })).toBeDisabled();
  await dialog.locator(".closeButton").click();
  await expect(page.getByRole("button", { name: "Request exact quote", exact: true })).toBeDisabled();
  expect(f.requests.filter(request => request.path === "/rfqs")).toHaveLength(0);
});

test("wallet dialog disables chains unavailable on this deployment", async ({ page }) => {
  const f = await walletLoginFixtures(page);
  f.control.walletChains = ["ethereum"]; await open(page);
  await page.locator("#otc-bridge").getByRole("button", { name: "Request exact quote", exact: true }).click();
  const dialog = page.locator("#wallet-connections");
  await expect(dialog.getByRole("button", { name: "Connect Ethereum wallet", exact: true })).toBeEnabled();
  await expect(dialog.getByRole("button", { name: "Connect Everscale wallet", exact: true })).toBeDisabled();
  expect(f.requests.filter(request => request.path === "/session/challenge")).toHaveLength(0);
});


test("quote reuses the header picker; cancelling preserves the ticket without an RFQ", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 780 });
  const f = await walletLoginFixtures(page); await open(page);
  await page.locator("#otc-input").fill("123.456");
  const request = page.locator("#otc-bridge").getByRole("button", { name: "Request exact quote", exact: true });
  await request.click();
  const dialog = page.getByRole("dialog", { name: "Wallet connections", exact: true });
  await expect(dialog).toHaveAttribute("id", "wallet-connections");
  await expect(page.getByRole("dialog")).toHaveCount(1);
  await expect(dialog.getByRole("searchbox", { name: "Search wallets" })).toBeFocused();
  await expect.poll(() => dialog.locator(".optionLogo img").evaluateAll(images => images.length === 4 && images.every(image => (image as HTMLImageElement).complete && (image as HTMLImageElement).naturalWidth > 0))).toBe(true);
  expect(await dialog.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  await page.keyboard.press("Shift+Tab");
  await expect(dialog.getByRole("button", { name: "Connect Everscale wallet", exact: true })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(dialog.getByRole("searchbox", { name: "Search wallets" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(dialog.locator(".closeButton")).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(dialog.getByRole("searchbox", { name: "Search wallets" })).toBeFocused();
  await expect(page.locator(".appShell")).toHaveAttribute("inert", "");
  await expect(dialog.getByRole("button", { name: "Request exact quote", exact: true })).toBeDisabled();
  expect((await new AxeBuilder({ page }).include("#wallet-connections").withTags(["wcag2a", "wcag2aa"]).analyze()).violations).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(request).toBeFocused();
  await expect(page.locator("#otc-input")).toHaveValue("123.456");
  await expect(page.locator(".walletMenu")).toHaveCount(1);
  await expect(page.locator("#otc-bridge .accountButton")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Connect wallet", exact: true })).toHaveCount(1);
  await page.locator(".walletMenu .walletToggle").click();
  await expect(dialog).toHaveAttribute("id", "wallet-connections");
  await expect(page.getByRole("dialog")).toHaveCount(1);
  await expect(dialog.getByRole("button", { name: "Request exact quote", exact: true })).toHaveCount(0);
  await expect(dialog.locator(".optionName")).toHaveText(["Ethereum", "NEAR", "TRON", "Everscale"]);
  await dialog.locator(".closeButton").click();
  expect(f.requests.filter(request => request.path === "/rfqs")).toHaveLength(0);
  expect(f.auth).toHaveLength(0);
});


test("closing wallet connection discards a late sign-in response and never requests a quote", async ({ page }) => {
  const f = await walletLoginFixtures(page);
  let release: (() => void) | null = null;
  await page.route("**/api/otc/session/proof", async route => {
    await new Promise<void>(resolve => release = resolve);
    await route.fulfill({ json: { token: "late-simulation-token", actor: customer, desk: false, expires_at: "2099-10-10T00:00:00Z", signing_wallet: null, wallets: [{ chain: "ethereum", address: `0x${"b".repeat(40)}`, verified_at: new Date().toISOString() }] } });
  });
  await open(page);
  await page.locator("#otc-bridge").getByRole("button", { name: "Request exact quote", exact: true }).click();
  const dialog = page.locator("#wallet-connections");
  await dialog.getByRole("button", { name: "Connect Ethereum wallet", exact: true }).click();
  await expect.poll(() => !!release).toBe(true);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  release!();
  await expect.poll(() => f.requests.some(request => request.path === "/session/revoke")).toBe(true);
  await expect(page.locator(".walletMenu .walletToggle")).toBeVisible();
  await expect(page.locator("#otc-bridge").getByRole("button", { name: "Request exact quote", exact: true })).toBeEnabled();
  expect(await page.evaluate(() => sessionStorage.getItem("pay3flow.otc.session.v1"))).toBeNull();
  expect(f.requests.filter(request => request.path === "/rfqs")).toHaveLength(0);
});
