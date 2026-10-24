import { test, expect, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

// Browser responses are simulations; no test connects a wallet or sends funds.
test.setTimeout(60_000);
test.use({ actionTimeout: 15_000 });
const customer = "https://relay.example/actors/customer", desk = "https://relay.example/actors/desk";
const arb = "https://relay.example/actors/arbitrator";
const address = (digit: string) => `0x${digit.repeat(40)}`;
async function fixture(page: Page, options: { state?: string; direction?: string; arbitrator?: boolean; remaining?: number; callState?: string } = {}) {
  const actor = options.arbitrator ? arb : customer, direction = options.direction || "buy";
  const policy = { factory: address("8"), arbitrator: address("9"), singleton: address("7"), factory_code_hash: `0x${"a".repeat(64)}`, singleton_code_hash: `0x${"b".repeat(64)}` };
  const trade = { id: "escrow-fixture", rfq_id: "rfq-fixture", state: "accepted", created_at: new Date().toISOString(), offer: {}, decision: {}, terms: {
    direction, demo: false, input: "100", output: "200", input_units: direction === "buy" ? "100000000" : "100000000000", output_units: direction === "buy" ? "200000000000" : "200000000",
    customer_actor: customer, desk_actor: desk, customer_usdt: address("1"), desk_usdt: address("2"), escrow: policy,
    quote_by: "2099-10-10T00:02:00Z", pay_by: "2099-10-10T00:07:00Z", payout_by: "2099-10-10T00:17:00Z",
    network_costs: "Sender pays gas separately", ever_receiving_cost_units: "1000000", refund_policy: "USDT refunds to the depositor",
    fee_policy: { basis_units: direction === "buy" ? "100000000" : "200000000", fee_units: "5000000" }, proposal: {}
  }};
  const expiry = Date.now() + (options.remaining ?? 7200) * 1000;
  const instructions = { chain_id: 1, from: address("1"), to: policy.factory, data: "0x1234", value: "0x0", nonce: "0x3", checkpoint: "0x10" };
  const escrow = {
    deployment: { trade_id: `0x${"a".repeat(64)}`, terms_hash: `0x${"b".repeat(64)}`, buyer: direction === "buy" ? address("1") : address("2"), seller: direction === "buy" ? address("2") : address("1"), usdt_amount: trade.terms.fee_policy.basis_units, ever_amount: direction === "buy" ? trade.terms.output_units : trade.terms.input_units, valid_until: Date.parse(trade.terms.pay_by) / 1000 },
    policy, typed_data: {}, state: options.state || "awaiting_consent", safe: options.state ? address("3") : null, vault: options.state ? address("4") : null,
    created_at: options.state ? new Date(expiry - 14400000).toISOString() : null, expires_at: options.state ? new Date(expiry).toISOString() : null,
    observed_at: new Date().toISOString(), consents: [{ owner: address("1") }, { owner: address("2") }],
    proposals: [{ id: "proposal-fixture", action: "release", hash: `0x${"c".repeat(64)}`, typed_data: {}, dispute: true, reason: "Delivery disputed", evidence: { reference: "receipt" }, approvals: [{ owner: address("2") }] }],
    evidence: [{ kind: "funding", valid: true, evidence: { receipt: { transactionHash: `0x${"e".repeat(64)}`, blockNumber: "0x20" } } }],
    calls: options.callState ? [{ id: "saved-call", action: "deploy", actor: customer, proposal_id: null, state: options.callState, reference: options.callState === "failed" ? `0x${"d".repeat(64)}` : null, instructions }] : [] as any[]
  };
  const requests: { path: string; method: string; key?: string }[] = [];
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.addInitScript(({ actor }) => {
    localStorage.setItem("pay3flow-locale", "en");
    sessionStorage.setItem("pay3flow.otc.session.v1", JSON.stringify({ token: "simulation-token", actor, desk: false }));
  }, { actor });
  await page.route("**/api/otc/**", async route => {
    const req = route.request(), path = new URL(req.url()).pathname.replace("/api/otc", "");
    requests.push({ path, method: req.method(), key: req.headers()["idempotency-key"] });
    let response: unknown = {};
    if (path === "/config") response = { enabled: true, available: true, wallet_login_available: true, demo: false, desk, desk_name: "Fixture desk", escrow: policy };
    else if (path === "/listings") response = [];
    else if (path === "/session") response = { token: "simulation-token", actor, desk: false, expires_at: "2099-10-10T00:00:00Z", signing_wallet: null, wallets: [{ chain: "ethereum", address: options.arbitrator ? policy.arbitrator : address("1"), verified_at: new Date().toISOString() }] };
    else if (path === "/trades") response = { trades: [trade], rfqs: [] };
    else if (path === `/trades/${trade.id}`) response = { trade, attempts: [], evidence: [], customer_updates: [] };
    else if (path.endsWith("/escrow/prepare")) {
      const call = { id: "new-call", actor, action: req.postDataJSON().action, proposal_id: req.postDataJSON().proposal_id, state: "unknown", reference: null, instructions };
      escrow.calls.push(call); response = call;
    } else if (path.endsWith("/escrow")) response = escrow;
    await route.fulfill({ json: response });
  });
  await page.goto("/#/otc");
  await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-state", "connected", { timeout: 20_000 });
  await page.getByRole("tab", { name: /^My trades/ }).click();
  await page.locator(".tradeTables tbody tr[role=button]").first().click();
  await expect(page.getByTestId("otc-escrow")).toContainText("100,000 EVER");
  return { requests, escrow };
}

test("deployment requires explicit review and reload never repeats its saved handoff", async ({ page }) => {
  const f = await fixture(page);
  expect(f.requests.filter(r => r.method === "POST")).toEqual([]);
  await page.getByRole("button", { name: "Review deployment", exact: true }).click();
  await expect(page.getByRole("region", { name: "Review escrow transaction" })).toContainText(address("8"));
  await page.getByRole("button", { name: "Close review", exact: true }).click();
  await page.reload();
  await expect(page.getByText("Original result unknown. Another send from this wallet is blocked.", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Review deployment", exact: true })).toBeDisabled();
  expect(f.requests.filter(r => r.path.endsWith("/escrow/prepare"))).toHaveLength(1);
  await expect(page.getByRole("region", { name: "Review escrow transaction" })).toHaveCount(0);
});

test("only a finalized failed call enables an explicit reviewed retry", async ({ page }) => {
  const f = await fixture(page, { callState: "failed" });
  await page.getByRole("button", { name: "Review retry after confirmed failure" }).click();
  await expect(page.getByRole("region", { name: "Review escrow transaction" })).toBeVisible();
  expect(f.requests.filter(r => r.method === "POST")).toHaveLength(1);
  expect(f.requests.find(r => r.method === "POST")?.key).toBeTruthy();
});

test("arbitrator sees assigned settlement and no desk controls at 320 pixels", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 780 });
  const f = await fixture(page, { state: "funded", arbitrator: true });
  await expect(page.getByRole("heading", { name: "Desk console" })).toHaveCount(0);
  await page.getByText("Chain receipts", { exact: true }).click();
  await expect(page.getByTestId("otc-escrow")).toContainText(`0x${"e".repeat(64)}`);
  await expect(page.getByRole("button", { name: "Review and send EVER", exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Review approval", exact: true }).click();
  await expect(page.getByRole("region", { name: "Review multisig approval" })).toContainText("100 USDT");
  expect(f.requests.filter(r => r.method === "POST")).toEqual([]);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
  expect((await new AxeBuilder({ page }).include('[data-testid="otc-escrow"]').withTags(["wcag2a", "wcag2aa"]).analyze()).violations).toEqual([]);
});

test("new EVER signing closes in the final 30 minutes", async ({ page }) => {
  await fixture(page, { state: "funded", direction: "sell", remaining: 1799 });
  await expect(page.getByRole("button", { name: "Review and send EVER", exact: true })).toBeDisabled();
  await expect(page.getByText("The final 30 minutes are reserved for settlement. New EVER signing is closed.")).toBeVisible();
});

test("timeout recovery survives reload and offers direct instructions", async ({ page }) => {
  const f = await fixture(page, { state: "funded", remaining: -1 });
  await expect(page.getByRole("button", { name: "Review timeout refund" })).toBeEnabled();
  await expect(page.getByRole("button", { name: "Propose early refund" })).toHaveCount(0);
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Download direct refund instructions" }).click();
  expect((await download).suggestedFilename()).toBe("otc-escrow-recovery.json");
  await page.reload();
  await expect(page.getByRole("button", { name: "Review timeout refund" })).toBeEnabled();
  expect(f.requests.filter(r => r.method === "POST")).toEqual([]);
});
