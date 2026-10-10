import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

const desk = "https://relay.example/actors/desk", customer = "https://relay.example/actors/customer";
const terms = {
  direction: "buy", input: "100", output: "200", input_units: "100000000", output_units: "200000000000",
  customer_actor: customer, desk_actor: desk, quote_by: "2099-10-10T00:02:00Z", pay_by: "2099-10-10T00:07:00Z", payout_by: "2099-10-10T00:17:00Z",
  network_costs: "Sender pays gas separately", ever_receiving_cost_units: "1000000", refund_policy: "Refund to proved booked wallet after reconciliation",
  fee_policy: { basis_units: "100000000", fee_units: "5000000" }, proposal: { id: `${desk}/quotes/fixture`, publishes: { resourceConformsTo: "https://assets.example/ever" }, reciprocal: { resourceConformsTo: "https://assets.example/usdt" } }
};
async function fixtures(page: import("@playwright/test").Page, signedIn = false, available = false) {
  const trade = { id: "2d602252-fb09-4517-9c9a-b431e2f5cddd", rfq_id: "fixture", state: "accepted", terms, offer: {}, decision: {} };
  const attempts: any[] = [];
  const requests: string[] = [];
  if (signedIn) await page.addInitScript(({ customer }) => sessionStorage.setItem("pay3flow.otc.session.v1", JSON.stringify({ token: "simulation-token", actor: customer, desk: false })), { customer });
  await page.route("**/api/otc/**", async route => {
    const path = new URL(route.request().url()).pathname.replace("/api/otc", ""); requests.push(path);
    let body: unknown = {};
    if (path === "/config") body = { enabled: true, available, demo: false, desk, desk_name: "Fixture desk", ever_resource: "https://assets.example/ever", usdt_resource: "https://assets.example/usdt", counterparty_risk: "Two separate transfers. The desk must deliver or refund; there is no atomic swap or automatic rollback." };
    else if (path === "/listings") body = [{ id: `${desk}/listings/buy`, name: "Buy EVER", publishes: { resourceConformsTo: "https://assets.example/ever" }, reciprocal: { resourceConformsTo: "https://assets.example/usdt" } }];
    else if (path === "/trades") body = { trades: signedIn ? [trade] : [], rfqs: [] };
    else if (path.endsWith("/prepare")) {
      trade.state = "payment_confirming";
      const attempt = { id: "attempt-fixture", kind: "payment", state: "unknown", reference: null, instructions: { leg: { chain: "ethereum", amount: "100000000", transfer_amount: "100000000", sender: `0x${"b".repeat(40)}`, recipient: `0x${"a".repeat(40)}` }, call: { family: "evm", nonce: "0x3" } } };
      attempts.push(attempt); body = attempt;
    } else if (path.startsWith("/trades/")) body = { trade, attempts, evidence: [] };
    await route.fulfill({ json: body });
  });
  return requests;
}

test("scoped Exchange displays both networks and paused status at 320 pixels", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 780 }); await fixtures(page);
  await page.goto("/#/otc"); await page.locator(".settlementSection > summary").click();
  await expect(page.getByRole("heading", { name: "EVER / USDT" })).toBeVisible();
  await expect(page.getByText("Route paused", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Request exact quote" })).toBeDisabled();
  await page.locator(".workspace").getByRole("button", { name: "Sell EVER", exact: true }).click();
  await expect(page.getByText("You send · EVER on Everscale", { exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
  const accessibility = await new AxeBuilder({ page }).include(".workspace").withTags(["wcag2a", "wcag2aa"]).analyze();
  expect(accessibility.violations).toEqual([]);
});

test("accepted terms show fixed amounts, rate, fee and separate deadlines", async ({ page }) => {
  await fixtures(page, true, true); await page.goto("/#/otc"); await page.locator(".settlementSection > summary").click();
  await page.getByRole("button", { name: /Buy EVER 100 → 200 accepted/ }).click();
  await expect(page.getByText("0.5 USDT per EVER", { exact: true })).toBeVisible();
  await expect(page.getByText("5 USDT · paid by desk", { exact: true })).toBeVisible();
  await expect(page.getByText("Payment inclusion deadline", { exact: true })).toBeVisible();
  await page.locator(".workspace").getByRole("button", { name: "Sell EVER", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Buy EVER · accepted" })).toBeVisible();
  await page.reload(); await page.locator(".settlementSection > summary").click();
  await expect(page.getByRole("heading", { name: "Buy EVER · accepted" })).toBeVisible();
});

test("unresolved wallet handoff blocks another send after refresh", async ({ page }) => {
  const requests = await fixtures(page, true, true); await page.goto("/#/otc"); await page.locator(".settlementSection > summary").click();
  await page.getByRole("button", { name: /Buy EVER 100 → 200 accepted/ }).click();
  await page.getByRole("button", { name: "Review and send customer payment" }).click();
  await expect(page.getByText("Original result unknown. Another send is blocked.", { exact: true })).toBeVisible();
  await page.reload(); await page.locator(".settlementSection > summary").click();
  await expect(page.getByText("Original result unknown. Another send is blocked.", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Review and send customer payment" })).toHaveCount(0);
  expect(requests.filter(path => path.endsWith("/prepare"))).toHaveLength(1);
});
