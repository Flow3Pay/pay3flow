import { test, expect, type Page } from "@playwright/test";
import { decodeFunctionData } from "viem";
import { SAFE_DEPLOYMENTS, SAFE_FACTORY, SAFE_SINGLETON, factoryAbi, safeDeployment, safeInitializer } from "../src/lib/otc/production-safe";
import { mockSafeRpc, mockSafeWallet, owners, reference, safeFixture, storageKey } from "./fixtures/safe-rpc";

async function open(page: Page) {
  await page.route("**/api/**", route => route.fulfill({ json: route.request().url().endsWith("/config") ? { enabled: false, available: false, demo: false } : [] }));
  await page.goto("/#/otc");
  await mountSafe(page);
}
async function mountSafe(page: Page) {
  await page.evaluate(async () => {
    const harness = await import("/tests/fixtures/safe-harness.ts");
    harness.mountSafe();
  });
  await page.getByTestId("production-safe").locator("summary").click();
}
const panel = (page: Page) => page.getByTestId("production-safe");
async function review(page: Page) {
  for (const [index, role] of ["Buyer", "Seller", "Arbitrator"].entries()) await panel(page).getByLabel(`${role} Ethereum address`).fill(owners[index]);
  await panel(page).getByRole("button", { name: "Review 2-of-3 Safe" }).click();
  await expect(panel(page).getByRole("button", { name: "Confirm creation in owner wallet" })).toBeEnabled();
}
async function blocked(page: Page) {
  await expect(panel(page).getByText(/Creation is pending or unresolved/)).toBeVisible();
  await expect(panel(page).getByRole("button", { name: "Confirm creation in owner wallet" })).toHaveCount(0);
  await expect(panel(page).getByRole("button", { name: "Set up another wallet" })).toHaveCount(0);
  await expect(panel(page).getByRole("button", { name: "Edit owners" })).toHaveCount(0);
}

test("OTC Safe is Mainnet-only and opens existing wallets on Ethereum", async ({ page }) => {
  await open(page);
  await expect(panel(page).getByText("Ethereum Mainnet", { exact: false })).toBeVisible();
  await expect(panel(page).getByText(/Sepolia|testnet/i)).toHaveCount(0);
  await expect(panel(page).getByLabel("Safe network")).toHaveCount(0);
  await expect(panel(page).getByText(/no automatic expiry or refund/)).toBeVisible();
  await expect(panel(page).getByRole("button", { name: "Review 2-of-3 Safe" })).toBeEnabled();
  await expect(panel(page).getByRole("link", { name: "Continue in Safe" })).toHaveCount(0);
  await panel(page).getByLabel("Already have a Safe? Enter its Ethereum address").fill(owners[0]);
  await panel(page).getByRole("button", { name: "Open existing Safe" }).click();
  await expect(panel(page).getByRole("link", { name: "Continue in Safe" })).toHaveAttribute("href", `https://app.safe.global/home?safe=eth:${owners[0]}`);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test("duplicate owners cannot reach a wallet prompt or RPC", async ({ page }) => {
  const rpc = await mockSafeRpc(page);
  await mockSafeWallet(page);
  await open(page);
  for (const role of ["Buyer", "Seller", "Arbitrator"]) await panel(page).getByLabel(`${role} Ethereum address`).fill(owners[0]);
  await panel(page).getByRole("button", { name: "Review 2-of-3 Safe" }).click();
  await expect(panel(page).getByRole("alert")).toContainText("different addresses");
  expect(rpc.calls).toHaveLength(0);
  expect(await page.evaluate(() => (window as any).safeWallet.calls)).toHaveLength(0);
});

for (const [address] of SAFE_DEPLOYMENTS) test(`Mainnet refuses mismatched contract ${address}`, async ({ page }) => {
  const rpc = await mockSafeRpc(page);
  await mockSafeWallet(page);
  await open(page);
  await review(page);
  rpc.invalidContract = address;
  await panel(page).getByRole("button", { name: "Confirm creation in owner wallet" }).click();
  await expect(panel(page).getByRole("alert")).toContainText("contract verification failed");
  expect(await page.evaluate(() => (window as any).safeWallet.calls)).toHaveLength(0);
});

test("OTC Safe rejects an RPC reporting a different network before review", async ({ page }) => {
  const rpc = await mockSafeRpc(page);
  rpc.chainId = 11155111;
  await open(page);
  for (const [index, role] of ["Buyer", "Seller", "Arbitrator"].entries()) await panel(page).getByLabel(`${role} Ethereum address`).fill(owners[index]);
  await panel(page).getByRole("button", { name: "Review 2-of-3 Safe" }).click();
  await expect(panel(page).getByRole("alert")).toContainText("RPC returned another network");
  expect(rpc.calls).toEqual(["eth_chainId"]);
});

test("creation sends only the reviewed factory call on Ethereum Mainnet", async ({ page }) => {
  const rpc = await mockSafeRpc(page);
  await mockSafeWallet(page);
  await open(page);
  await review(page);
  await panel(page).getByRole("button", { name: "Confirm creation in owner wallet" }).click();
  await blocked(page);
  const wallet = await page.evaluate(() => (window as any).safeWallet);
  expect(wallet.sends).toHaveLength(1);
  expect(Number(wallet.chainId)).toBe(1);
  expect(wallet.switches).toEqual([]);
  const transaction = wallet.sends[0];
  expect(transaction.to.toLowerCase()).toBe(SAFE_FACTORY.toLowerCase());
  expect(transaction.from.toLowerCase()).toBe(owners[0]);
  expect(BigInt(transaction.value)).toBe(0n);
  const saved = await page.evaluate(key => JSON.parse(localStorage.getItem(key)!), storageKey(1));
  const decoded = decodeFunctionData({ abi: factoryAbi, data: transaction.data });
  expect(decoded.args).toEqual([SAFE_SINGLETON, safeInitializer(owners), BigInt(saved.salt)]);
  expect(saved.chainId).toBe(1);
  expect(saved.reference).toBe(reference);
  await expect(panel(page).getByRole("link", { name: "View original transaction" })).toHaveAttribute("href", `https://etherscan.io/tx/${reference}`);
  await page.reload();
  await mountSafe(page);
  await blocked(page);
  rpc.deployed = true;
  await panel(page).getByRole("button", { name: "Check on-chain status" }).click();
  await expect(panel(page).getByText("Safe wallet ready", { exact: true })).toBeVisible();
  await expect(panel(page).getByRole("link", { name: "Open Safe to fund, approve and transfer" })).toHaveAttribute("href", `https://app.safe.global/home?safe=eth:${saved.address}`);
});

test("owner wallet outside the reviewed three cannot send", async ({ page }) => {
  await mockSafeRpc(page);
  await mockSafeWallet(page, { address: "0x4444444444444444444444444444444444444444" });
  await open(page); await review(page);
  await panel(page).getByRole("button", { name: "Confirm creation in owner wallet" }).click();
  await expect(panel(page).getByRole("alert")).toContainText("three reviewed owner wallets");
  expect(await page.evaluate(() => (window as any).safeWallet.sends)).toHaveLength(0);
});

test("Safe actions are locked throughout the wallet operation", async ({ page }) => {
  await mockSafeRpc(page); await mockSafeWallet(page);
  await open(page); await review(page);
  await page.evaluate(() => (window as any).safeWallet.hold = true);
  await panel(page).getByRole("button", { name: "Confirm creation in owner wallet" }).click();
  await expect.poll(() => page.evaluate(() => (window as any).safeWallet.sends.length)).toBe(1);
  await expect(panel(page).getByRole("button", { name: "Check on-chain status" })).toBeDisabled();
  await page.evaluate(() => (window as any).safeWallet.release());
  await expect(panel(page).getByRole("button", { name: "Check on-chain status" })).toBeEnabled();
  await blocked(page);
});

test("lost wallet response stays unresolved across reload", async ({ page }) => {
  await mockSafeRpc(page); await mockSafeWallet(page, { outcome: "unknown" });
  await open(page); await review(page);
  await panel(page).getByRole("button", { name: "Confirm creation in owner wallet" }).click();
  await blocked(page);
  await page.reload(); await mountSafe(page);
  await blocked(page);
  const saved = await page.evaluate(key => JSON.parse(localStorage.getItem(key)!), storageKey(1));
  expect(saved.state).toBe("unknown");
  expect(saved.reference).toBeNull();
});

test("explicit wallet rejection permits retry of the same reviewed Safe", async ({ page }) => {
  await mockSafeRpc(page); await mockSafeWallet(page, { outcome: "rejected" });
  await open(page); await review(page);
  const before = await page.evaluate(key => JSON.parse(localStorage.getItem(key)!), storageKey(1));
  await panel(page).getByRole("button", { name: "Confirm creation in owner wallet" }).click();
  await expect(panel(page).getByRole("alert")).toContainText("User rejected");
  await expect(panel(page).getByRole("button", { name: "Confirm creation in owner wallet" })).toBeEnabled();
  const after = await page.evaluate(key => JSON.parse(localStorage.getItem(key)!), storageKey(1));
  expect(after).toEqual(before);
});

test("legacy pending Mainnet operation migrates without losing transaction evidence", async ({ page }) => {
  await open(page);
  const legacy = { owners, salt: "1", address: safeDeployment(owners, 1n, safeFixture.creationCode).address, reference, state: "submitted" };
  await page.evaluate(value => localStorage.setItem("pay3flow.otc.production-safe.v1", JSON.stringify(value)), legacy);
  await page.reload(); await mountSafe(page);
  await blocked(page);
  await expect(panel(page).getByRole("link", { name: "View original transaction" })).toHaveAttribute("href", `https://etherscan.io/tx/${reference}`);
  expect(await page.evaluate(() => localStorage.getItem("pay3flow.otc.production-safe.v1"))).toBeNull();
  await blocked(page);
});

test("saved Mainnet plan restores after reload", async ({ page }) => {
  await mockSafeRpc(page); await open(page);
  await review(page);
  const mainnet = await page.evaluate(key => JSON.parse(localStorage.getItem(key)!), storageKey(1));
  await page.reload(); await mountSafe(page);
  await expect(panel(page).getByText(mainnet.address, { exact: true })).toBeVisible();
});

test("corrupt saved Mainnet operation blocks a new creation", async ({ page }) => {
  await open(page);
  await page.evaluate(key => localStorage.setItem(key, "{broken"), storageKey(1));
  await expect(panel(page).getByRole("alert")).toContainText("could not be restored");
  await expect(panel(page).getByRole("button", { name: "Review 2-of-3 Safe" })).toBeDisabled();
});

test("on-chain recovery rejects an unexpected owner set or threshold", async ({ page }) => {
  const rpc = await mockSafeRpc(page);
  await open(page); await review(page);
  rpc.deployed = true; rpc.threshold = 1n;
  await panel(page).getByRole("button", { name: "Check on-chain status" }).click();
  await expect(panel(page).getByRole("alert")).toContainText("owners, threshold or singleton differs");
  rpc.threshold = 2n; rpc.actualOwners = [owners[0], owners[1], "0x4444444444444444444444444444444444444444"];
  await panel(page).getByRole("button", { name: "Check on-chain status" }).click();
  await expect(panel(page).getByRole("alert")).toContainText("owners, threshold or singleton differs");
  await expect(panel(page).getByRole("link", { name: "Open Safe to fund, approve and transfer" })).toHaveCount(0);
});
