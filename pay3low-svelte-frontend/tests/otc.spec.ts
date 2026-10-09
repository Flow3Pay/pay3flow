import { expect, test, type Page } from "@playwright/test";

async function openOtc(page: Page, hash = "#/otc") {
  await page.addInitScript(() => localStorage.setItem("pay3flow-locale", "en"));
  await page.route("**/api/**", (route) => route.fulfill({ status: 503, contentType: "application/json", body: "{}" }));
  await page.goto(`/${hash}`);
  await expect(page.getByTestId("otc-workspace")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Trade on your terms." })).toBeVisible();
}

test("OTC navigation, drawer and share work at every screen size", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await openOtc(page);
  await expect(page.getByRole("link", { name: "OTC", exact: true })).toHaveAttribute("aria-current", "page");
  await expect(page.getByRole("link", { name: "Open API documentation" })).toBeHidden();
  const share = page.getByRole("button", { name: "Share exchange", exact: true });
  const theme = await page.getByRole("button", { name: "Switch theme" }).boundingBox();
  const shareBox = await share.boundingBox();
  expect(shareBox!.x).toBeGreaterThan(theme!.x);
  await page.getByRole("button", { name: "Open menu" }).click();
  const menu = page.getByRole("dialog", { name: "Menu", exact: true });
  await expect(menu).toBeVisible();
  await expect(menu.getByRole("link")).toHaveCount(3);
  await expect(page.locator(".appShell")).toHaveAttribute("inert", "");
  await page.keyboard.press("Shift+Tab");
  await expect(menu.getByRole("link", { name: "Open Pay3Flow on GitHub" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Open menu" })).toBeFocused();
  await page.evaluate(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (text: string) => sessionStorage.setItem("test.otc-share", text) } }));
  await share.click();
  expect(await page.evaluate(() => sessionStorage.getItem("test.otc-share"))).toContain("/#/otc");
  await page.getByRole("link", { name: "SWAP", exact: true }).click();
  await expect(page.locator(".workspace")).toBeVisible();
  await expect(page.getByTestId("otc-workspace")).toHaveCount(0);
  await page.getByRole("link", { name: "OTC", exact: true }).click();
  await expect(page.getByTestId("otc-workspace")).toBeVisible();
  await page.goBack();
  await expect(page.locator(".workspace")).toBeVisible();
  expect(errors).toEqual([]);
});

test("panels collapse, chart controls update, and markets survive reload", async ({ page }) => {
  await openOtc(page);
  for (const [title, id] of [["Bridge", "otc-bridge"], ["Chart", "otc-chart"], ["Orderbook", "otc-book"], ["Market activity", "otc-activity"]]) {
    await page.getByRole("button", { name: `Collapse ${title}`, exact: true }).click();
    await expect(page.locator(`#${id}`)).toBeHidden();
    await page.getByRole("button", { name: `Expand ${title}`, exact: true }).click();
    await expect(page.locator(`#${id}`)).toBeVisible();
  }
  const originalLevels = await page.locator(".askLevels .level").count();
  await page.getByRole("combobox", { name: "Price grouping" }).selectOption("5");
  await expect.poll(() => page.locator(".askLevels .level").count()).toBeLessThan(originalLevels);
  await page.getByRole("combobox", { name: "Price grouping" }).selectOption("1");
  const line = await page.locator(".buyLine").getAttribute("d");
  await page.getByRole("button", { name: "7D", exact: true }).click();
  await expect(page.locator(".buyLine")).not.toHaveAttribute("d", line!);
  await page.getByRole("button", { name: "Candlesticks", exact: true }).click();
  await expect(page.locator(".buyLine")).toHaveCount(0);
  await page.getByRole("button", { name: "Zoom in", exact: true }).click();
  await page.getByRole("button", { name: "Reset chart", exact: true }).click();
  await page.getByRole("combobox", { name: "Choose market" }).selectOption("ETH-USDT");
  await expect(page).toHaveURL(/#\/otc\?market=ETH-USDT$/);
  await expect(page.locator("#otc-bridge").getByRole("button", { name: "Buy ETH", exact: true })).toBeVisible();
  await page.reload();
  await expect(page.getByRole("combobox", { name: "Choose market" })).toHaveValue("ETH-USDT");
  for (const width of [320, 393, 768, 1024, 1512]) {
    await page.setViewportSize({ width, height: 900 });
    await expect(page.locator(".marketStrip")).toBeVisible();
    expect(await page.evaluate(() => document.body.scrollWidth <= innerWidth)).toBe(true);
    expect(await page.locator("header .inner").evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
  }
});

test("book selection creates, retains and cancels a demo limit order", async ({ page }) => {
  await openOtc(page);
  await page.locator(".askLevels .level").last().click();
  const chosenPrice = await page.getByRole("textbox", { name: "Price", exact: true }).inputValue();
  await expect(page.locator(".askLevels .level").last()).toHaveClass(/selected/);
  await page.getByRole("textbox", { name: "You send", exact: true }).fill("1500");
  await page.getByTestId("otc-review").click();
  const review = page.getByRole("dialog", { name: "One last look." });
  await expect(review).toBeVisible();
  await expect(review).toContainText("no funds move");
  await review.getByRole("button", { name: "Create demo order", exact: true }).click();
  await expect(review).toHaveCount(0);
  await expect(page.getByRole("tab", { name: /My orders/ })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator("#otc-orders-content tbody tr")).toHaveCount(1);
  await expect(page.locator("#otc-orders-content")).toContainText(Number(chosenPrice).toLocaleString("en-US", { minimumFractionDigits: 2 }));
  await page.reload();
  await page.getByRole("tab", { name: /My orders/ }).click();
  await expect(page.locator("#otc-orders-content tbody tr")).toHaveCount(1);
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.locator("#otc-orders-content")).toContainText("Your first order starts here.");
  await page.getByRole("tab", { name: "Order history", exact: true }).click();
  await expect(page.locator("#otc-history-content")).toContainText("Cancelled");
  expect(await page.evaluate(() => JSON.parse(sessionStorage.getItem("pay3flow.otc.demo-orders.v1")!)[0].status)).toBe("cancelled");
});

test("invalid amounts and exhausted liquidity cannot create orders", async ({ page }) => {
  await openOtc(page);
  const input = page.getByRole("textbox", { name: "You send", exact: true });
  for (const amount of ["", "-1", "1e10", "abc", "0.00000001"]) {
    await input.fill(amount);
    await expect(page.getByTestId("otc-review")).toBeDisabled();
  }
  await page.locator("#otc-bridge").getByRole("button", { name: "Market", exact: true }).click();
  await input.fill("999999999");
  await expect(page.locator("#otc-bridge")).toContainText("Not enough demo liquidity");
  await expect(page.getByTestId("otc-review")).toBeDisabled();
  await input.fill("1000");
  await page.getByTestId("otc-review").click();
  const review = page.getByRole("dialog", { name: "One last look." });
  await page.keyboard.press("Escape");
  await expect(review).toHaveCount(0);
  await expect(page.getByTestId("otc-review")).toBeFocused();
  await page.getByTestId("otc-review").click();
  await review.getByRole("button", { name: "Simulate market order", exact: true }).click();
  await expect(page.locator("#otc-history-content")).toContainText("Simulated");
});

test("SWAP retains its selected corridor and amount across OTC navigation", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("pay3flow-locale", "en"));
  await page.route("**/api/**", (route) => route.fulfill({ status: 503, contentType: "application/json", body: "{}" }));
  await page.goto("/#/swap/USDT/BTC?amount=1000");
  await expect(page.locator(".workspace")).toBeVisible();
  await expect(page.getByRole("link", { name: "SWAP", exact: true })).toHaveAttribute("href", "/#/swap/USDT/BTC?amount=1000");
  await page.getByRole("link", { name: "OTC", exact: true }).click();
  await expect(page.getByTestId("otc-workspace")).toBeVisible();
  await page.getByRole("link", { name: "SWAP", exact: true }).click();
  await expect(page).toHaveURL(/#\/swap\/USDT\/BTC\?amount=1000$/);
});
