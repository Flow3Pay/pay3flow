import { expect, test, type Page } from "@playwright/test";

async function openOtc(page: Page, hash = "#/otc") {
  await page.addInitScript(() => localStorage.setItem("pay3flow-locale", "en"));
  await page.route("**/api/**", (route) => route.fulfill({ status: 503, contentType: "application/json", body: "{}" }));
  await page.goto(`/${hash}`);
  await expect(page.getByTestId("otc-workspace")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Trade on your terms." })).toBeVisible();
  await expect(page.locator(".introOverlay")).toHaveCount(0, { timeout: 6000 });
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

test("EVER/USDT is the default and shared market with native icons and buy/sell amounts", async ({ page }) => {
  await openOtc(page);
  const market = page.getByRole("combobox", { name: "Choose market" });
  await expect(market).toHaveValue("EVER-USDT");
  await expect(page.locator(".marketSelector")).toContainText("Everscale");
  const icon = page.locator('.marketSelector img[src="/icons/assets/ever.svg"]');
  await expect.poll(() => icon.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBeGreaterThan(0);
  const bridge = page.locator("#otc-bridge");
  await expect(bridge.getByRole("button", { name: "Buy", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(bridge.getByRole("button", { name: /Select sending asset:/ })).toContainText("USDT");
  await expect(bridge.getByRole("button", { name: /Select recipient asset:/ })).toContainText("EVER");
  await bridge.getByRole("textbox", { name: "Price", exact: true }).fill("0.01234");
  await bridge.getByRole("textbox", { name: "You send", exact: true }).fill("1");
  await expect(bridge.getByRole("textbox", { name: "You receive", exact: true })).toHaveValue("81.037277");
  await bridge.getByRole("button", { name: "Sell", exact: true }).click();
  await expect(bridge.getByRole("button", { name: /Select sending asset:/ })).toContainText("EVER");
  await expect(bridge.getByRole("button", { name: /Select recipient asset:/ })).toContainText("USDT");
  await bridge.getByRole("textbox", { name: "You send", exact: true }).fill("100.25");
  await expect(bridge.getByRole("textbox", { name: "You receive", exact: true })).toHaveValue("1.23709");
  await market.selectOption("BTC-USDT");
  await market.selectOption("EVER-USDT");
  await expect(page).toHaveURL(/#\/otc\?market=EVER-USDT$/);
  await page.reload();
  await expect(market).toHaveValue("EVER-USDT");
  await expect(bridge.getByRole("button", { name: "Buy", exact: true })).toBeVisible();
});

test("panels collapse, chart controls update, and markets survive reload", async ({ page }) => {
  await openOtc(page);
  for (const [title, id] of [["Bridge", "otc-bridge"], ["Orderbook", "otc-book"], ["Market activity", "otc-activity"]]) {
    await page.getByRole("button", { name: `Collapse ${title}`, exact: true }).click();
    await expect(page.locator(`#${id}`)).toBeHidden();
    await page.getByRole("button", { name: `Expand ${title}`, exact: true }).click();
    await expect(page.locator(`#${id}`)).toBeVisible();
  }
  const originalLevels = await page.locator("#otc-book .askLevels .level").count();
  await page.getByRole("button", { name: "Price grouping", exact: true }).click();
  await page.getByRole("dialog", { name: "Price grouping" }).getByRole("button", { name: "0.00005", exact: true }).click();
  await expect.poll(() => page.locator("#otc-book .askLevels .level").count()).toBeLessThan(originalLevels);
  await page.getByRole("button", { name: "Price grouping", exact: true }).click();
  await page.getByRole("dialog", { name: "Price grouping" }).getByRole("button", { name: "0.00001", exact: true }).click();
  const line = await page.locator(".buyLine").getAttribute("d");
  await page.getByRole("button", { name: "7D", exact: true }).click();
  await expect(page.locator(".buyLine")).not.toHaveAttribute("d", line!);
  await page.getByRole("button", { name: "Candlesticks", exact: true }).click();
  await expect(page.locator(".buyLine")).toHaveCount(0);
  await page.getByRole("button", { name: "Zoom in", exact: true }).click();
  await page.getByRole("button", { name: "Reset chart", exact: true }).click();
  await page.getByRole("combobox", { name: "Choose market" }).selectOption("ETH-USDT");
  await expect(page).toHaveURL(/#\/otc\?market=ETH-USDT$/);
  await expect(page.locator("#otc-bridge").getByRole("button", { name: "Buy", exact: true })).toBeVisible();
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
  await page.locator("#otc-book .askLevels .level").last().click();
  const chosenPrice = await page.getByRole("textbox", { name: "Price", exact: true }).inputValue();
  await expect(page.locator("#otc-book .askLevels .level").last()).toHaveClass(/selected/);
  await page.getByRole("textbox", { name: "You send", exact: true }).fill("1500");
  await page.getByTestId("otc-review").click();
  const review = page.getByRole("dialog", { name: "One last look." });
  await expect(review).toBeVisible();
  await expect(review).toContainText("no funds move");
  await review.getByRole("button", { name: "Create demo order", exact: true }).click();
  await expect(review).toHaveCount(0);
  await expect(page.getByRole("tab", { name: /My orders/ })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator("#otc-orders-content tbody tr")).toHaveCount(1);
  await expect(page.locator("#otc-orders-content")).toContainText(Number(chosenPrice).toLocaleString("en-US", { minimumFractionDigits: 5, maximumFractionDigits: 5 }));
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
  for (const amount of ["", "-1", "1e10", "abc", "0.000000000001"]) {
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

test("inter-panel arrows reclaim space and preserve the layout", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    if (!localStorage.getItem("pay3flow.otc.panels.v1")) localStorage.setItem("pay3flow.otc.panels.v1", JSON.stringify({ chartOpen: false }));
  });
  await openOtc(page);
  const workspace = page.getByTestId("otc-workspace");
  await expect(workspace.locator(".marketFootnote, .bookHint, .previewBadge, .eyebrow, .workspaceFooter")).toHaveCount(0);
  const chart = page.locator("#otc-chart-column");
  const book = page.locator("#otc-book-column");
  const bridge = page.locator("#otc-bridge-column");
  await expect(chart).toBeVisible();
  const panelsBounds = (await page.locator(".otcGrid").boundingBox())!;
  const stripBounds = (await page.locator(".marketStrip").boundingBox())!;
  expect(stripBounds.y + stripBounds.height).toBeLessThanOrEqual(panelsBounds.y);
  if (page.viewportSize()!.width <= 980) {
    await expect(page.locator(".bridgeToggle span")).toHaveCSS("transform", "matrix(0, 1, -1, 0, 0, 0)");
    await expect(page.locator(".bookToggle span")).toHaveCSS("transform", "matrix(0, -1, 1, 0, 0, 0)");
  }
  const originalWidth = (await chart.boundingBox())!.width;
  await expect(page.getByRole("button", { name: /(?:Collapse|Expand) Chart/ })).toHaveCount(0);
  if (page.viewportSize()!.width > 980) {
    await expect(page.locator(".bridgeToggle span")).toHaveCSS("transform", "matrix(1, 0, 0, 1, 0, 0)");
    await expect(page.locator(".bookToggle span")).toHaveCSS("transform", "matrix(-1, 0, 0, -1, 0, 0)");
  }
  await page.getByRole("button", { name: "Collapse Bridge", exact: true }).click();
  await expect(bridge).toBeHidden();
  await expect(chart).toBeVisible();
  await expect(page.getByRole("button", { name: "Expand Bridge", exact: true })).toHaveAttribute("aria-expanded", "false");
  if (page.viewportSize()!.width > 980) {
    await expect.poll(async () => (await chart.boundingBox())!.width).toBeGreaterThan(originalWidth + 50);
    await expect(page.locator(".bridgeToggle span")).toHaveCSS("transform", "matrix(-1, 0, 0, -1, 0, 0)");
  }
  await page.getByRole("button", { name: "Collapse Orderbook", exact: true }).click();
  await expect(book).toBeHidden();
  await expect(chart).toBeVisible();
  if (page.viewportSize()!.width > 980) {
    await expect(page.locator(".bookToggle span")).toHaveCSS("transform", "matrix(1, 0, 0, 1, 0, 0)");
    await expect.poll(async () => chart.evaluate((element) => {
      const card = element.getBoundingClientRect();
      const workspace = element.closest(".otcWorkspace")!.getBoundingClientRect();
      return Math.abs(card.left + card.width / 2 - workspace.left - workspace.width / 2);
    })).toBeLessThan(1);
    await expect.poll(async () => (await chart.boundingBox())!.width).toBeGreaterThan((await workspace.boundingBox())!.width - 65);
  }
  await page.getByRole("button", { name: "Collapse Market activity", exact: true }).click();
  await expect(page.locator("#otc-activity-section")).toBeHidden();
  await page.reload();
  await expect(bridge).toBeHidden();
  await expect(chart).toBeVisible();
  await expect(book).toBeHidden();
  await expect(page.locator("#otc-activity-section")).toBeHidden();
  for (const title of ["Bridge", "Orderbook", "Market activity"]) {
    const toggle = page.getByRole("button", { name: `Expand ${title}`, exact: true });
    await toggle.focus();
    await page.keyboard.press("Enter");
  }
  await expect(chart).toBeVisible();
  await expect(bridge).toBeVisible();
  await expect(book).toBeVisible();
  await expect(page.locator("#otc-activity-section")).toBeVisible();
  await workspace.screenshot({ path: testInfo.outputPath("otc-panels.png") });
  await page.getByRole("button", { name: "Buy / Sell", exact: true }).click();
  await expect(page.locator(".bridgeIcon")).toHaveClass(/bridgeIconReversed/);
  await expect(page.getByRole("button", { name: "Sell", exact: true })).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Buy / Sell", exact: true }).click();
  await expect(page.locator(".bridgeIcon")).not.toHaveClass(/bridgeIconReversed/);
});


test("asset and network pickers keep the bridge and order review consistent", async ({ page }) => {
  await openOtc(page);
  await page.getByRole("button", { name: /Select sending network: Ethereum/ }).click();
  const network = page.getByRole("dialog", { name: "Choose network", exact: true });
  await network.getByRole("option", { name: /TRON/ }).click();
  await page.getByRole("textbox", { name: "You send", exact: true }).fill("2500");
  await page.getByRole("button", { name: "Collapse Bridge", exact: true }).click();
  await page.getByRole("button", { name: "Expand Bridge", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "You send", exact: true })).toHaveValue("2500");
  await expect(page.getByRole("button", { name: /Select sending network: TRON/ })).toBeVisible();
  await page.getByRole("button", { name: "Select recipient asset: EVER", exact: true }).click();
  const assets = page.getByRole("dialog", { name: "Choose the asset you receive", exact: true });
  await assets.getByRole("option", { name: /ETH/ }).click();
  await assets.getByRole("option", { name: /Ethereum/ }).click();
  await expect(page.getByRole("combobox", { name: "Choose market" })).toHaveValue("ETH-USDT");
  await expect(page.getByRole("button", { name: "Buy", exact: true })).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Sell", exact: true }).click();
  await expect(page.getByRole("button", { name: /Select recipient network: TRON/ })).toBeVisible();
  await page.getByTestId("otc-review").click();
  await expect(page.getByRole("dialog", { name: "One last look." })).toContainText(/TRON \(TRC-?20\)/);
  await page.keyboard.press("Escape");
  await page.reload();
  await expect(page.getByRole("button", { name: "Sell", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("button", { name: /Select recipient network: TRON/ })).toBeVisible();
});

test("grouping menu supports keyboard selection and restores focus", async ({ page }) => {
  await openOtc(page);
  const trigger = page.getByRole("button", { name: "Price grouping", exact: true });
  await trigger.click();
  const menu = page.getByRole("dialog", { name: "Price grouping" });
  await expect(menu.getByRole("button", { name: "0.00001", exact: true })).toBeFocused();
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");
  await expect(trigger).toContainText("0.00005");
  await expect(trigger).toBeFocused();
  await trigger.click();
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await expect(trigger).toBeFocused();
});

test("Armenian OTC keeps the brand heading and fits a narrow viewport", async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem("pay3flow-locale", "hy"));
  await page.route("**/api/**", route => route.fulfill({ status: 503, body: "{}" }));
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 320, height: 900 });
  await page.goto("/?lang=hy#/otc");
  const workspace = page.getByTestId("otc-workspace");
  await expect(workspace.getByRole("heading", { level: 1 })).toHaveText("Առևտուր՝ ձեր պայմաններով։");
  await expect(workspace.getByRole("button", { name: "Գնել", exact: true })).toBeVisible();
  await expect(workspace.getByRole("button", { name: "Վաճառել", exact: true })).toBeVisible();
  expect(await workspace.locator(".titleAccent").evaluate(el => getComputedStyle(el, "::after").content)).toBe('""');
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
  await workspace.screenshot({ path: testInfo.outputPath("otc-armenian-mobile.png") });
});


test("OTC amount fields use the swap typography and control sizing", async ({ page }) => {
  await openOtc(page);
  const appearance = (element: Element) => {
    const style = getComputedStyle(element);
    return { font: style.fontFamily, size: style.fontSize, weight: style.fontWeight, line: style.lineHeight, spacing: style.letterSpacing };
  };
  const otc = await page.locator("#otc-send").evaluate(appearance);
  const assetHeight = await page.locator("#otc-bridge .methodTrigger").first().evaluate(el => (el as HTMLElement).offsetHeight);
  await expect(page.locator("#otc-chart-column .panelHead")).not.toContainText("BTC/USDT");
  const head = (await page.locator("#otc-chart-column .panelHead").boundingBox())!;
  const tools = (await page.locator(".chartTools").boundingBox())!;
  expect(tools.y - head.y - head.height).toBeLessThanOrEqual(4);
  await page.getByRole("link", { name: "SWAP", exact: true }).click();
  await expect(page.locator(".workspace")).toBeVisible();
  expect(await page.getByLabel("Amount to send", { exact: true }).evaluate(appearance)).toEqual(otc);
  expect(await page.locator(".moneyPanelSource .methodTrigger").evaluate(el => (el as HTMLElement).offsetHeight)).toBe(assetHeight);
});


test("market activity has green bids on the left and red asks on the right", async ({ page }, testInfo) => {
  await openOtc(page);
  const buy = page.getByTestId("otc-buy-book"), sell = page.getByTestId("otc-sell-book");
  await expect(buy.locator(".bidLevels .level")).toHaveCount(9);
  await expect(buy.locator(".askLevels")).toHaveCount(0);
  await expect(sell.locator(".askLevels .level")).toHaveCount(9);
  await expect(sell.locator(".bidLevels")).toHaveCount(0);
  const left = (await buy.boundingBox())!, right = (await sell.boundingBox())!;
  expect(left.x + left.width).toBeLessThanOrEqual(right.x);
  for (const [theme, green, red] of [["light", "rgb(36, 115, 56)", "rgb(212, 61, 53)"], ["dark", "rgb(98, 206, 121)", "rgb(255, 119, 112)"]]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme, theme);
    await expect(buy.locator(".level").first()).toHaveCSS("color", green);
    await expect(sell.locator(".level").first()).toHaveCSS("color", red);
    await expect(buy.locator(".level").first()).toHaveCSS("font-style", "normal");
    await expect(buy.locator(".level").first()).toHaveCSS("font-family", await page.locator(".panelHead h2").first().evaluate(el => getComputedStyle(el).fontFamily));
  }
  await sell.locator(".level").first().click();
  await expect(page.getByRole("button", { name: "Buy", exact: true })).toHaveAttribute("aria-pressed", "true");
  const price = (await sell.locator(".levelPrice").first().innerText()).replaceAll(",", "");
  await expect(page.getByRole("textbox", { name: "Price", exact: true })).toHaveValue(price);
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
  await page.locator(".splitBooks").screenshot({ path: testInfo.outputPath("split-orderbooks.png") });
});

test("chart hover shows both line points, the full date and side volumes", async ({ page }) => {
  await openOtc(page);
  const plot = page.locator(".priceChart");
  const box = (await plot.boundingBox())!;
  await plot.hover({ position: { x: box.width * .4, y: box.height * .4 } });
  const tooltip = page.getByRole("tooltip");
  await expect(tooltip).toContainText(/2026.*UTC/);
  for (const label of ["Buy price", "Sell price", "Buy volume", "Sell volume"]) await expect(tooltip.getByText(label, { exact: true })).toBeVisible();
  await expect(plot.locator(".buyPoint")).toHaveCount(1);
  await expect(plot.locator(".sellPoint")).toHaveCount(1);
  expect(Number(await plot.locator(".sellPoint").getAttribute("cy"))).toBeLessThan(Number(await plot.locator(".buyPoint").getAttribute("cy")));
  const popup = (await tooltip.boundingBox())!;
  expect(popup.x).toBeGreaterThanOrEqual(box.x);
  expect(popup.x + popup.width).toBeLessThanOrEqual(box.x + box.width);
  await page.getByRole("heading", { name: "Trade on your terms." }).hover();
  await expect(tooltip).toHaveCount(0);
});

test("chart width changes with dragging and keyboard and survives reload", async ({ page, isMobile }) => {
  test.skip(isMobile, "Column resizing applies to the horizontal desktop layout.");
  await openOtc(page);
  const chart = page.locator("#otc-chart-column");
  const handle = page.getByRole("separator", { name: "Resize chart · Bridge", exact: true });
  const initial = (await chart.boundingBox())!.width;
  const box = (await handle.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + 20);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 - 90, box.y + 20, { steps: 10 });
  await page.mouse.up();
  await expect.poll(async () => (await chart.boundingBox())!.width).toBeGreaterThan(initial + 70);
  await handle.focus();
  await page.keyboard.press("ArrowRight");
  await expect.poll(async () => (await chart.boundingBox())!.width).toBeLessThan(initial + 85);
  await chart.evaluate(async el => { await Promise.all(el.parentElement!.getAnimations().map(animation => animation.finished)); });
  const changed = (await chart.boundingBox())!.width;
  await page.reload();
  await expect(chart).toBeVisible();
  await expect(page.locator(".introOverlay")).toHaveCount(0, { timeout: 6000 });
  await expect.poll(async () => (await chart.boundingBox())!.width).toBeCloseTo(changed, 0);
  await handle.focus();
  await page.keyboard.press("Home");
  await expect.poll(async () => (await chart.boundingBox())!.width).toBeCloseTo(initial, 0);
});
