import { expect, test, type Page } from "@playwright/test";

async function prepare(page: Page) {
  await page.route("**/api/**", route => route.fulfill({ status: 503, contentType: "application/json", body: "{}" }));
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow-locale", "en");
    sessionStorage.setItem("test.startup-modes", "[]");
    new MutationObserver(records => {
      for (const record of records) for (const node of record.addedNodes) {
        if (!(node instanceof Element)) continue;
        const overlay = node.matches(".introOverlay") ? node : node.querySelector(".introOverlay");
        if (overlay) {
          const modes = JSON.parse(sessionStorage.getItem("test.startup-modes")!);
          modes.push(overlay.getAttribute("data-startup-mode"));
          sessionStorage.setItem("test.startup-modes", JSON.stringify(modes));
        }
      }
    }).observe(document, { childList: true, subtree: true });
  });
}

const startupModes = (page: Page) => page.evaluate(() => JSON.parse(sessionStorage.getItem("test.startup-modes")!));

test("switching SWAP and OTC never repeats the startup animation", async ({ page }) => {
  await prepare(page);
  await page.goto("/");
  await expect(page.locator('.introOverlay[data-startup-mode="swap"]')).toBeVisible();
  await expect(page.locator(".introOverlay")).toHaveCount(0, { timeout: 6000 });
  for (let cycle = 0; cycle < 2; cycle++) {
    await page.getByRole("link", { name: "OTC", exact: true }).click();
    await expect(page.getByTestId("otc-workspace")).toBeVisible();
    await expect(page.locator(".introOverlay")).toHaveCount(0);
    await page.getByRole("link", { name: "SWAP", exact: true }).click();
    await expect(page.locator(".workspace")).toBeVisible();
    await expect(page.locator(".introOverlay")).toHaveCount(0);
  }
  expect(await startupModes(page)).toEqual(["swap"]);
});

test("loading and reloading OTC shows only OTC and keeps the requested market", async ({ page }) => {
  await prepare(page);
  await page.goto("/#/otc?market=SOL-USDT");
  const intro = page.locator(".introOverlay");
  for (let load = 0; load < 2; load++) {
    await expect(intro).toHaveAttribute("data-startup-mode", "otc");
    await expect(intro.locator(".introTitle")).toHaveText("OTC");
    await expect(page.getByRole("link", { name: "OTC", exact: true })).toHaveAttribute("aria-current", "page");
    await expect(intro).toHaveCount(0, { timeout: 6000 });
    await expect(page.getByTestId("otc-workspace")).toBeVisible();
    await expect(page.getByRole("combobox", { name: "Choose market" })).toHaveValue("SOL-USDT");
    await expect(page.locator(".workspace")).toHaveCount(0);
    expect(await startupModes(page)).toEqual(["otc"]);
    if (load === 0) await page.reload();
  }
});

test("changing modes during the startup cancels it and cleans up the page", async ({ page }) => {
  await prepare(page);
  await page.goto("/");
  await expect(page.locator(".introOverlay")).toBeVisible();
  await page.getByRole("link", { name: "OTC", exact: true }).click();
  await expect(page.getByTestId("otc-workspace")).toBeVisible();
  await expect(page.locator(".introOverlay")).toHaveCount(0);
  await expect(page.locator("html")).not.toHaveClass(/introPlaying/);
  await page.getByRole("link", { name: "SWAP", exact: true }).click();
  await expect(page.locator(".introOverlay")).toHaveCount(0);
  expect(await startupModes(page)).toEqual(["swap"]);
});

test("reduced motion opens OTC directly without a startup overlay", async ({ page }) => {
  await prepare(page);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/#/otc");
  await expect(page.getByTestId("otc-workspace")).toBeVisible();
  await expect(page.locator(".introOverlay")).toHaveCount(0);
  await page.getByRole("link", { name: "SWAP", exact: true }).click();
  await expect(page.locator(".workspace")).toBeVisible();
  expect(await startupModes(page)).toEqual([]);
});
