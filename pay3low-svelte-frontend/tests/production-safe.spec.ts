import { test, expect } from "@playwright/test";
const owners = ["0x1111111111111111111111111111111111111111", "0x2222222222222222222222222222222222222222", "0x3333333333333333333333333333333333333333"];
async function open(page: import("@playwright/test").Page) {
  await page.route("**/api/otc/**", route => route.fulfill({ json: route.request().url().endsWith("/config") ? { enabled: false, available: false, demo: false } : [] }));
  await page.goto("/#/otc");
  await page.getByTestId("production-safe").locator("summary").click();
}
test("production Safe is usable while desk is paused and opens existing wallets", async ({ page }) => {
  await open(page);
  const panel = page.getByTestId("production-safe");
  await expect(panel.getByText(/no automatic expiry or refund/)).toBeVisible();
  await expect(panel.getByRole("button", { name: "Review 2-of-3 Safe" })).toBeEnabled();
  await panel.getByLabel("Already have a Safe? Enter its Ethereum address").fill(owners[0]);
  await panel.getByRole("button", { name: "Open existing Safe" }).click();
  await expect(panel.getByRole("link", { name: "Continue in Safe" })).toHaveAttribute("href", `https://app.safe.global/home?safe=eth:${owners[0]}`);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});
test("duplicate owners cannot reach a wallet prompt or Ethereum RPC", async ({ page }) => {
  let rpc = 0;
  await page.route("https://ethereum-rpc.publicnode.com/**", async route => { rpc++; await route.abort(); });
  await open(page);
  for (const role of ["Buyer", "Seller", "Arbitrator"]) await page.getByLabel(`${role} Ethereum address`).fill(owners[0]);
  await page.getByRole("button", { name: "Review 2-of-3 Safe" }).click();
  await expect(page.getByTestId("production-safe").getByRole("alert")).toContainText("different addresses");
  expect(rpc).toBe(0);
});
test("unresolved creation survives reload and blocks another send", async ({ page }) => {
  await page.addInitScript(owners => localStorage.setItem("pay3flow.otc.production-safe.v1", JSON.stringify({ owners, salt: "1", address: owners[0], reference: null, state: "unknown" })), owners);
  await open(page);
  const panel = page.getByTestId("production-safe");
  await expect(panel.getByText(/Creation is pending or unresolved/)).toBeVisible();
  await expect(panel.getByRole("button", { name: "Confirm creation in owner wallet" })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Set up another wallet" })).toHaveCount(0);
  await page.reload(); await panel.locator("summary").click();
  await expect(panel.getByText(/Creation is pending or unresolved/)).toBeVisible();
});
