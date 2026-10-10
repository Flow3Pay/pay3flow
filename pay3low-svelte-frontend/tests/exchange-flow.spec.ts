import { expect, test, type Locator, type Page } from "@playwright/test";

async function selectLanguage(page: Page, language: "en" | "ru" | "hy") {
  await page.locator(".languageToggle").click();
  await page.getByRole("listbox").getByRole("option", { name: { en: "English", ru: "Русский", hy: "Հայերեն" }[language], exact: true }).click();
}

async function expectNumberedTimeline(instructions: Locator, numbers: string[]) {
  const steps = instructions.getByTestId("instruction-step");
  await expect(steps).toHaveCount(numbers.length);
  await expect(steps.locator(".stepNumber")).toHaveText(numbers);
  const markerShape = await steps.locator(".stepNumber").first().evaluate((element) => {
    const rect = element.getBoundingClientRect();
    return { width: rect.width, height: rect.height, radius: getComputedStyle(element).borderRadius };
  });
  expect(Math.abs(markerShape.width - markerShape.height)).toBeLessThan(0.1);
  expect(markerShape.radius).toBe("50%");
}

async function openApp(page: Page, path = "/") {
  await page.goto(path);
  await expect
    .poll(() => page.locator(".appShell").evaluate((element) => (element as HTMLElement).style.getPropertyValue("--puzzle-pattern")))
    .not.toBe("");
  await expect(page.locator(".workspace")).toBeVisible();
  await expect(page.locator(".introOverlay")).toHaveCount(0, { timeout: 6000 });
}

async function settleEntrance(page: Page) {
  await page.locator(".workspace, .hero > p").evaluateAll(async (elements) => {
    await Promise.all(elements.flatMap((element) => element.getAnimations().map((animation) => animation.finished.catch(() => {}))));
  });
}

async function expectPeriodButtonBesidePair(chart: Locator) {
  await chart.evaluate(async (element) => {
    const dialog = element.closest('[role="dialog"]');
    if (dialog) await Promise.all(dialog.getAnimations().map((animation) => animation.finished.catch(() => {})));
  });
  await expect(chart.locator(".pairCurrency")).toHaveCount(2);
  await expect(chart.locator(".periodButton")).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
  await expect(chart.locator(".periodButton")).toHaveCSS("border-top-width", "0px");
  await expect(chart.locator(".pairCurrency img")).toHaveCount(2);
  for (const icon of await chart.locator(".pairCurrency img").all()) {
    await expect(icon).toHaveAttribute("src", /^\/icons\/(flags|assets)\//);
    await expect.poll(() => icon.evaluate((element: HTMLImageElement) => element.complete && element.naturalWidth > 0)).toBe(true);
  }
  const pair = await chart.locator(".pair").boundingBox();
  const button = await chart.getByRole("button", { name: "Chart time range" }).boundingBox();
  expect(pair).not.toBeNull();
  expect(button).not.toBeNull();
  expect(button!.x).toBeGreaterThanOrEqual(pair!.x + pair!.width - 1);
  expect(Math.abs(button!.y + button!.height / 2 - pair!.y - pair!.height / 2)).toBeLessThan(5);
  await expect(chart.locator(".periodButton img")).toHaveCSS("filter", "none");
}

test("shared fiat and crypto link restores its currencies in a new browser", async ({ page }) => {
  await mockBackend(page);
  await page.goto("/#/swap/USDT/KZT?amount=287.0062069");
  await expect.poll(() => page.evaluate(() => localStorage.getItem("pay3flow.exchange.source-method"))).toBe("global-usdt");
  await expect.poll(() => page.evaluate(() => localStorage.getItem("pay3flow.exchange.target-method"))).toBe("kz-kaspi");
  await expect(page.getByLabel("Amount to send")).toHaveValue("287.0062069");
  await expect(page).toHaveURL(/#\/swap\/USDT\/KZT\?amount=287\.0062069$/);
});

test("main exchange share copies settings and restores them in a fresh browser", async ({ page, browser, isMobile }) => {
  await mockBackend(page, { usdtFiatNetwork: "tron" });
  await page.goto("/#/swap/USDT/KZT?amount=287.0062069&from=global-usdt&to=kz-kaspi&fromNetwork=tron&sources=bybit&methods=p2p&assets=USDC");
  await expect(page.getByLabel("Amount to send")).toHaveValue("287.0062069");
  await expect.poll(() => page.evaluate(() => localStorage.getItem("pay3flow.exchange.source-network"))).toBe("tron");
  await page.evaluate(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (text: string) => sessionStorage.setItem("test.exchange-share", text) } }));
  await expect(page.locator(".introOverlay")).toHaveCount(0, { timeout: 6000 });
  const trigger = page.getByRole("button", { name: "Share exchange", exact: true });
  await trigger.click();
  const dialog = page.getByRole("dialog", { name: "Share exchange", exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".sharePreview")).toHaveAttribute("src", /share-image\.png.*from=USDT&to=KZT/);
  await expect.poll(() => dialog.locator(".sharePreview").evaluate((image: HTMLImageElement) => image.naturalWidth)).toBe(1200);
  await expect.poll(() => page.locator(".appShell").evaluate((node) => (node as HTMLElement).inert)).toBe(true);
  await expect(dialog.getByRole("button", { name: "Close share dialog" })).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(dialog.getByRole("button", { name: "Copy link" })).toBeFocused();
  await dialog.getByRole("button", { name: "Copy link" }).click();
  await expect(dialog.getByRole("button", { name: "Link copied" })).toBeVisible();
  const link = await page.evaluate(() => sessionStorage.getItem("test.exchange-share"));
  expect(link).toContain("/swap/USDT/KZT?");
  const params = new URL(link!).searchParams;
  expect(Object.fromEntries(params)).toMatchObject({ amount: "287.0062069", from: "global-usdt", to: "kz-kaspi", fromNetwork: "tron", sources: "bybit", methods: "p2p", assets: "USDC" });
  expect(params.get("toName")).toContain("Kaspi");
  expect(params.get("fromNetworkName")).toMatch(/tron/i);
  const previewParams = new URL(await dialog.locator(".sharePreview").getAttribute("src") as string).searchParams;
  expect(previewParams.get("toName")).toBe(params.get("toName"));
  expect(previewParams.get("fromNetworkName")).toBe(params.get("fromNetworkName"));
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(trigger).toBeFocused();
  expect(await page.locator(".appShell").evaluate((node) => (node as HTMLElement).inert)).toBe(false);
  const fresh = await browser.newPage();
  await mockBackend(fresh, { usdtFiatNetwork: "tron" });
  await fresh.goto(link!);
  await expect(fresh.getByLabel("Amount to send")).toHaveValue("287.0062069");
  await expect.poll(() => fresh.evaluate(() => ({
    source: localStorage.getItem("pay3flow.exchange.source-method"),
    target: localStorage.getItem("pay3flow.exchange.target-method"),
    network: localStorage.getItem("pay3flow.exchange.source-network"),
    sources: localStorage.getItem("pay3flow.exchange.p2p-sources"),
    methods: localStorage.getItem("pay3flow.exchange.methods"),
    assets: localStorage.getItem("pay3flow.exchange.intermediary-assets"),
  }))).toMatchObject({ source: "global-usdt", target: "kz-kaspi", network: "tron", sources: "bybit", methods: "p2p", assets: "USDC" });
  await fresh.close();
});

test("exchange share selects its link when clipboard access is denied", async ({ page, isMobile }) => {
  await mockBackend(page);
  await openApp(page);
  await page.evaluate(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async () => { throw new Error("Denied"); } } }));
  await page.getByRole("button", { name: "Share exchange", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Share exchange", exact: true });
  await dialog.getByRole("button", { name: "Copy link" }).click();
  const input = dialog.getByLabel("Exchange link");
  await expect(input).toBeFocused();
  expect(await input.evaluate((node: HTMLInputElement) => node.selectionEnd! - node.selectionStart!)).toBe((await input.inputValue()).length);
  await expect(dialog).toContainText("Select and copy the link manually.");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
});

test("system theme follows the browser until the user chooses a theme", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await mockBackend(page);
  await openApp(page);

  const root = page.locator("html");
  const exchangesIcon = page.getByRole("button", { name: "Choose exchanges" }).locator("img");
  const refreshIcon = page.getByRole("button", { name: "Refresh routes now" }).locator("img");
  const settingsIcon = page.getByRole("button", { name: "Route refresh settings" }).locator("img");
  await expect(root).toHaveAttribute("data-theme", "dark");
  await expect(exchangesIcon).toHaveCSS("filter", "brightness(0) invert(1)");
  await expect(refreshIcon).toHaveCSS("filter", "brightness(0) invert(1)");
  await expect(settingsIcon).toHaveCSS("filter", "brightness(0) invert(1)");

  await page.emulateMedia({ colorScheme: "light" });
  await expect(root).toHaveAttribute("data-theme", "light");
  await expect(exchangesIcon).toHaveCSS("filter", "brightness(0)");
  await expect(refreshIcon).toHaveCSS("filter", "brightness(0)");
  await expect(settingsIcon).toHaveCSS("filter", "brightness(0)");

  await page.getByRole("button", { name: "Switch theme" }).click();
  await expect(root).toHaveAttribute("data-theme", "dark");
  await page.emulateMedia({ colorScheme: "dark" });
  await page.emulateMedia({ colorScheme: "light" });
  await expect(root).toHaveAttribute("data-theme", "dark");
  await page.reload();
  await expect(root).toHaveAttribute("data-theme", "dark");
});

test("search activity uses hourly counts and fits beside routes or opens in a mobile modal", async ({ page, isMobile }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  const activityRequests: string[] = [];
  page.on("request", (request) => { if (request.url().includes("/api/p2p/route-activity")) activityRequests.push(request.url()); });
  await mockBackend(page);
  await openApp(page);
  const chart = page.getByTestId("search-activity");
  const chartToggle = page.getByRole("button", { name: "Show search activity" });
  await expect(chart).toHaveCount(0);
  if (!isMobile) {
    await expect(chartToggle).toBeVisible();
    await expect(chartToggle).toHaveAttribute("aria-expanded", "false");
    const toggleAppearance = await chartToggle.evaluate((element) => ({
      top: element.getBoundingClientRect().top,
      cardBottom: document.querySelector(".card")!.getBoundingClientRect().bottom,
      background: getComputedStyle(element).backgroundColor,
    }));
    expect(toggleAppearance.top).toBeGreaterThanOrEqual(toggleAppearance.cardBottom + 6);
    expect(toggleAppearance.top).toBeLessThanOrEqual(toggleAppearance.cardBottom + 10);
    expect(toggleAppearance.background).toBe("rgba(0, 0, 0, 0)");
  }
  const backgroundBefore = isMobile ? null : await page.screenshot({ clip: { x: 8, y: 180, width: 1, height: 1 } });
  if (isMobile) {
    await expect(chartToggle).toBeHidden();
    await page.getByRole("button", { name: "Open search activity graph" }).click();
    const dialog = page.getByRole("dialog", { name: "Searches for this exchange" });
    await expect(dialog.getByTestId("search-activity")).toBeVisible();
    await expectPeriodButtonBesidePair(dialog.getByTestId("search-activity"));
    await page.emulateMedia({ colorScheme: "light" });
    await expect(dialog.locator(".periodButton img")).toHaveCSS("filter", "brightness(0)");
    await expect(dialog.locator(".activityStats strong")).toHaveText("28");
    await expect(dialog.locator(".brush")).toHaveCount(0);
    await expect.poll(() => page.evaluate(() => document.body.style.position)).toBe("fixed");
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect.poll(() => page.evaluate(() => document.body.style.position)).toBe("");
  } else {
    await chartToggle.click();
    await expect(page.getByRole("button", { name: "Hide search activity" })).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByRole("button", { name: "Hide search activity" })).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
    await page.evaluate(() => window.scrollTo(0, 0));
    await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
    expect(await page.screenshot({ clip: { x: 8, y: 180, width: 1, height: 1 } })).toEqual(backgroundBefore);
    await expect(chart).toBeVisible();
    await expect(chart.locator(".activityStats strong")).toHaveText("28");
    await expectPeriodButtonBesidePair(chart);
    await page.emulateMedia({ colorScheme: "light" });
    await expect(chart.locator(".periodButton img")).toHaveCSS("filter", "brightness(0)");
    await expect(chart.locator(".activityStats span")).toHaveText("searches in period · 1 week");
    await expect(chart.locator(".brush")).toHaveCount(0);
    const surfaces = await page.evaluate(() => ({
      chart: getComputedStyle(document.querySelector(".activityCard")!).backgroundColor,
      routes: getComputedStyle(document.querySelector(".side .panel")!).backgroundColor,
    }));
    expect(surfaces.chart).toBe(surfaces.routes);
    const dimensions = await page.evaluate(() => {
      const stack = document.querySelector(".converterStack")!.getBoundingClientRect();
      const side = document.querySelector(".side .panel")!.getBoundingClientRect();
      const card = document.querySelector(".converterStack .card")!.getBoundingClientRect();
      const chart = document.querySelector(".activityReveal .activityCard")!.getBoundingClientRect();
      return { stackBottom: stack.bottom, sideBottom: side.bottom, chartBottom: chart.bottom, chartRight: chart.right, cardRight: card.right };
    });
    expect(dimensions.stackBottom).toBeLessThanOrEqual(dimensions.sideBottom + 1);
    expect(Math.abs(dimensions.chartBottom - dimensions.sideBottom)).toBeLessThanOrEqual(2);
    expect(Math.abs(dimensions.chartRight - dimensions.cardRight)).toBeLessThanOrEqual(1);
    await expect(chart.locator(".mainPlot svg path.area")).toHaveCount(1);
    await expect(page.locator(".activityReveal")).toHaveCSS("overflow", "visible");
    const chartSize = await chart.evaluate((element) => ({ visible: element.clientHeight, content: element.scrollHeight }));
    expect(chartSize.content).toBeLessThanOrEqual(chartSize.visible + 1);
    await page.setViewportSize({ width: 1024, height: 900 });
    const narrowDimensions = await page.evaluate(() => ({
      stackBottom: document.querySelector(".converterStack")!.getBoundingClientRect().bottom,
      sideBottom: document.querySelector(".side .panel")!.getBoundingClientRect().bottom,
      chartBottom: document.querySelector(".activityReveal .activityCard")!.getBoundingClientRect().bottom,
      chartRight: document.querySelector(".activityReveal .activityCard")!.getBoundingClientRect().right,
      cardRight: document.querySelector(".converterStack .card")!.getBoundingClientRect().right,
    }));
    expect(narrowDimensions.stackBottom).toBeLessThanOrEqual(narrowDimensions.sideBottom + 1);
    expect(Math.abs(narrowDimensions.chartBottom - narrowDimensions.sideBottom)).toBeLessThanOrEqual(2);
    expect(Math.abs(narrowDimensions.chartRight - narrowDimensions.cardRight)).toBeLessThanOrEqual(1);
    await page.getByRole("button", { name: "Hide search activity" }).click();
    await expect(chart).toHaveCount(0);
  }
  expect(activityRequests.length).toBeGreaterThan(0);
  expect(new URL(activityRequests[0]).searchParams.has("anonymous_id")).toBe(false);
});

test("routes can be hidden while the bridge stays centered and restored after reload", async ({ page, isMobile }) => {
  await mockBackend(page);
  await openApp(page);

  const routes = page.locator("#routes");
  const hideRoutes = page.getByRole("button", { name: "Hide routes" });
  await expect(routes).toBeVisible();
  await expect(page.getByTestId("search-activity")).toHaveCount(0);
  await expect(hideRoutes).toHaveAttribute("aria-expanded", "true");
  const togglePosition = await page.evaluate((mobile) => {
    const card = document.querySelector(".converterStack .card")!.getBoundingClientRect();
    const bridge = document.querySelector(".flowBridge")!.getBoundingClientRect();
    const toggle = document.querySelector(mobile ? ".mobileRoutesToggle" : ".routesToggle")!.getBoundingClientRect();
    return { cardRight: card.right, cardCenter: card.left + card.width / 2, cardBottom: card.bottom, toggleLeft: toggle.left, toggleRight: toggle.right, toggleTop: toggle.top, bridgeCenter: bridge.top + bridge.height / 2, toggleCenter: toggle.top + toggle.height / 2, toggleHorizontalCenter: toggle.left + toggle.width / 2 };
  }, isMobile);
  if (isMobile) {
    expect(Math.abs(togglePosition.toggleHorizontalCenter - togglePosition.cardCenter)).toBeLessThan(1);
    expect(togglePosition.toggleTop).toBeGreaterThanOrEqual(togglePosition.cardBottom + 6);
    expect(togglePosition.toggleTop).toBeLessThanOrEqual(togglePosition.cardBottom + 10);
  } else {
    expect(Math.abs(togglePosition.toggleCenter - togglePosition.bridgeCenter)).toBeLessThan(1);
    expect(togglePosition.toggleLeft).toBeGreaterThanOrEqual(togglePosition.cardRight - 1);
    expect(togglePosition.toggleLeft).toBeLessThanOrEqual(togglePosition.cardRight + 1);
  }
  await hideRoutes.click();
  await expect(routes).toHaveCount(0);
  await expect(page.getByRole("dialog", { name: "Searches for this exchange" })).toHaveCount(0);
  await expect(page.getByTestId("search-activity")).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => {
    const card = document.querySelector(".converterStack .card")!.getBoundingClientRect();
    const workspace = document.querySelector(".workspace")!.getBoundingClientRect();
    return Math.abs(card.left + card.width / 2 - workspace.left - workspace.width / 2);
  })).toBeLessThan(1);
  await expect.poll(() => page.evaluate(() => localStorage.getItem("pay3flow.exchange.routes-visible"))).toBe("false");

  if (!isMobile) {
    await page.getByRole("button", { name: "Show search activity" }).click();
    await expect(page.getByTestId("search-activity")).toBeVisible();
    await expect(page.locator(".activityReveal .mainPlot svg path.area")).toHaveCount(1);
    await expect.poll(() => page.locator(".activityReveal .mainPlot svg path:not(.area)").evaluate((element) => (element as SVGGeometryElement).getBBox().height)).toBeGreaterThan(0);
    await expect.poll(() => page.locator(".activityReveal").evaluate((element) => element.getBoundingClientRect().bottom - window.innerHeight)).toBeLessThanOrEqual(-8);
    await expect.poll(() => page.evaluate(() => localStorage.getItem("pay3flow.exchange.activity-visible"))).toBe("true");
  }

  await page.reload();
  await expect(page.locator(".workspace")).toBeVisible();
  await expect(page.getByRole("button", { name: "Show routes" })).toHaveAttribute("aria-expanded", "false");
  await expect(routes).toHaveCount(0);
  if (!isMobile) {
    await expect(page.getByTestId("search-activity")).toBeVisible();
    await expect.poll(() => page.locator(".activityReveal").evaluate((element) => element.getBoundingClientRect().bottom - window.innerHeight)).toBeLessThanOrEqual(-8);
    await page.getByRole("button", { name: "Hide search activity" }).click();
    await expect(page.getByTestId("search-activity")).toHaveCount(0);
  }
  await page.getByRole("button", { name: "Show routes" }).click();
  await expect(routes).toBeVisible();
  if (!isMobile) {
    await expect.poll(() => page.evaluate(() => {
      const card = document.querySelector(".converterStack .card")!.getBoundingClientRect();
      const workspace = document.querySelector(".workspace")!.getBoundingClientRect();
      return Math.abs(card.left - workspace.left);
    })).toBeLessThan(1);
  }
});

test("search placeholders stay inside the route panel", async ({ page }) => {
  await mockBackend(page);
  let releaseSearch!: () => void;
  const holdSearch = new Promise<void>((resolve) => { releaseSearch = resolve; });
  await page.route("**/api/p2p/routes**", async (route) => {
    await holdSearch;
    await route.abort();
  });
  await page.routeWebSocket(/\/ws\/p2p\/routes(?:\?|$)/, (socket) => {
    socket.onMessage(() => {});
  });
  await openApp(page);
  await page.getByLabel("Amount to send").fill("100000");
  const skeletons = page.locator("#routes .skeletonCard");
  await expect(skeletons).toHaveCount(5);
  await expect(page.getByTestId("start-search")).toBeDisabled();
  await expect(page.getByTestId("start-search")).toHaveCSS("background-color", "rgb(243, 246, 240)");
  const bounds = await page.evaluate(() => ({
    panelBottom: document.querySelector("#routes .panel")!.getBoundingClientRect().bottom,
    lastBottom: [...document.querySelectorAll("#routes .skeletonCard")].at(-1)!.getBoundingClientRect().bottom,
    lastContentBottom: [...document.querySelectorAll("#routes .skeletonCard")].at(-1)!.lastElementChild!.getBoundingClientRect().bottom,
  }));
  expect(bounds.lastBottom).toBeLessThanOrEqual(bounds.panelBottom - 1);
  expect(bounds.lastContentBottom).toBeLessThanOrEqual(bounds.lastBottom - 1);
  releaseSearch();
});

test("search activity keeps valid dimensions when opening guides and resizing", async ({ page, isMobile }) => {
  const chartWarnings: string[] = [];
  page.on("console", message => { if (message.text().includes("[LayerCake]")) chartWarnings.push(message.text()); });
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
    localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
  });
  await mockBackend(page);
  await openApp(page);
  await page.getByLabel("Amount to send").fill("100000");
  await expect(page.getByTestId("complete-route").first()).toBeVisible();
  const chart = page.getByTestId("search-activity");
  await page.getByRole("button", { name: isMobile ? "Open search activity graph" : "Show search activity" }).click();
  await expect(chart.locator(".mainPlot svg path.area")).toHaveCount(1);
  if (isMobile) await page.keyboard.press("Escape");
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();
  const guide = page.getByTestId("route-guide");
  await expect(guide.getByTestId("start-guide")).toBeVisible();
  await expect(chart).toHaveCount(0);
  await page.reload();
  await expect(guide.getByTestId("start-guide")).toBeVisible();
  await expect(chart).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(guide).toHaveCount(0);
  if (!isMobile) {
    await expect(chart.locator(".mainPlot svg path.area")).toHaveCount(1);
    await page.setViewportSize({ width: 393, height: 851 });
    await expect(chart).toHaveCount(0);
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.getByRole("button", { name: "Show search activity" }).click();
  } else {
    await page.getByRole("button", { name: "Open search activity graph" }).click();
  }
  await expect(chart.locator(".mainPlot svg path.area")).toHaveCount(1);
  const size = await chart.locator(".mainPlot svg").boundingBox();
  expect(size!.width).toBeGreaterThan(0);
  expect(size!.height).toBeGreaterThan(0);
  expect(chartWarnings).toEqual([]);
});

test("search activity range menu filters and remembers the selected period", async ({ page, isMobile }) => {
  await mockBackend(page);
  await openApp(page);
  await page.getByRole("button", { name: isMobile ? "Open search activity graph" : "Show search activity" }).click();
  const chart = isMobile ? page.getByRole("dialog", { name: "Searches for this exchange" }).getByTestId("search-activity") : page.getByTestId("search-activity");
  await expect(chart.locator(".activityStats strong")).toHaveText("28");
  const rangeButton = chart.getByRole("button", { name: "Chart time range" });
  await expect(rangeButton.locator("img")).toHaveAttribute("src", "/icons/ui/chart-period.png");
  await rangeButton.click();
  await expect(chart.locator(".headingCopy small")).toHaveCount(0);
  const menu = page.getByRole("dialog", { name: "Chart time range", exact: true });
  const selected = menu.locator('.periodOptions button[aria-pressed="true"]');
  await expect(menu.locator(".periodOptions button")).toHaveCount(8);
  await expect(selected).toHaveText("1 week");
  const accent = await page.evaluate(() => {
    const probe = document.createElement("span");
    probe.style.color = "var(--color-accent)";
    document.body.append(probe);
    const color = getComputedStyle(probe).color;
    probe.remove();
    return color;
  });
  await expect(selected).toHaveCSS("background-color", accent);
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(selected).toHaveCSS("background-color", accent);
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  if (isMobile) await expect(page.getByRole("dialog", { name: "Searches for this exchange" })).toBeVisible();
  await rangeButton.click();
  await menu.getByRole("button", { name: "1 hour", exact: true }).click();
  await expect(menu).toHaveCount(0);
  await expect(chart.locator(".activityStats span")).toHaveText("searches in period · 1 hour");
  await expect(chart.locator(".activityStats strong")).toHaveText("3");
  await expect.poll(() => page.evaluate(() => localStorage.getItem("pay3flow.exchange.activity-period"))).toBe("1h");
  await page.reload({ waitUntil: "networkidle" });
  if (isMobile || await page.getByTestId("search-activity").count() === 0) {
    await page.getByRole("button", { name: isMobile ? "Open search activity graph" : "Show search activity" }).click();
  }
  const restoredChart = isMobile ? page.getByRole("dialog", { name: "Searches for this exchange" }).getByTestId("search-activity") : page.getByTestId("search-activity");
  await expect(restoredChart.locator(".activityStats span")).toHaveText("searches in period · 1 hour");
  await restoredChart.getByRole("button", { name: "Chart time range" }).click();
  const restoredMenu = page.getByRole("dialog", { name: "Chart time range", exact: true });
  await expect(restoredMenu.getByRole("button", { name: "1 hour", pressed: true })).toHaveCSS("background-color", accent);
  await restoredMenu.getByRole("button", { name: "Close chart time range" }).click();
  await expect(restoredMenu).toHaveCount(0);
  await expect(restoredChart.getByRole("button", { name: "Chart time range" })).toBeFocused();
});

test("mobile sheets cover the viewport and the graph closes by dragging its handle", async ({ page, isMobile }) => {
  test.skip(!isMobile, "Mobile layout only");
  await mockBackend(page);
  await openApp(page);

  const expectFullViewport = async (selector: string) => {
    const bounds = await page.locator(selector).boundingBox();
    expect(bounds).not.toBeNull();
    expect(bounds!.x).toBe(0);
    expect(bounds!.y).toBe(0);
    expect(bounds!.width).toBe(page.viewportSize()!.width);
    expect(bounds!.height).toBe(page.viewportSize()!.height);
  };

  await page.locator(".menuToggle").click();
  const menuBounds = await page.locator("#header-menu").boundingBox();
  expect(menuBounds!.width).toBeLessThan(page.viewportSize()!.width);
  expect(menuBounds!.height).toBeLessThan(page.viewportSize()!.height);
  await expect(page.locator(".actions a, .actions button")).toHaveCount(4);
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Route refresh settings" }).click();
  await expectFullViewport(".settingsBackdrop");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Choose exchanges" }).click();
  await expectFullViewport(".settingsBackdrop");
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Open search activity graph" }).click();
  const graph = page.getByRole("dialog", { name: "Searches for this exchange" });
  await expect(graph.getByTestId("search-activity").locator(".sheetHandle")).toBeVisible();
  const handleAppearance = await graph.locator(".sheetHandle").evaluate((element) => ({ height: element.getBoundingClientRect().height, marker: getComputedStyle(element.querySelector("span")!).backgroundColor }));
  expect(handleAppearance.height).toBeGreaterThanOrEqual(44);
  expect(handleAppearance.marker).not.toBe("rgba(0, 0, 0, 0)");
  await expect(graph.locator(".close")).toBeHidden();
  await graph.evaluate(async (element) => {
    await Promise.all(element.getAnimations().map((animation) => animation.finished));
  });
  const handle = await graph.locator(".sheetHandle").boundingBox();
  expect(handle).not.toBeNull();
  await page.mouse.move(handle!.x + handle!.width / 2, handle!.y + handle!.height / 2);
  await page.mouse.down();
  await page.mouse.move(handle!.x + handle!.width / 2, handle!.y + handle!.height / 2 + 130, { steps: 5 });
  await expect(graph).toHaveAttribute("style", /--sheet-drag:/);
  await page.mouse.up();
  await expect(graph).toHaveCount(0);
});

test("fiat currency controls show local country flags", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  const currencyButton = page.getByRole("button", { name: "Select sending currency: AMD" });
  await expect(currencyButton.locator("img")).toHaveAttribute("src", "/icons/flags/am.svg");
  await currencyButton.click();
  const currencyPicker = page.getByRole("dialog", { name: "Choose currency" });
  await expect(currencyPicker.getByRole("option", { name: /AMD.*Armenian dram/ }).locator("img")).toHaveAttribute("src", "/icons/flags/am.svg");
  await expect(currencyPicker.getByRole("option", { name: /USD.*US dollar/ }).locator("img")).toHaveAttribute("src", "/icons/flags/us.svg");
  await page.keyboard.press("Escape");

  const methodPicker = await openCryptoPicker(page, "sending");
  const rubFlag = methodPicker.getByRole("option", { name: /RUB.*Russian ruble/ }).locator("img");
  await expect(rubFlag).toHaveAttribute("src", "/icons/flags/ru.svg");
  await expect.poll(() => rubFlag.evaluate((image: HTMLImageElement) => image.naturalWidth)).toBeGreaterThan(0);
});

test("dialogs keep the page still and restore its scroll position", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 600 });
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Open menu" }).click();
  const telegram = page.getByRole("menuitem", { name: "Open Pay3Flow Telegram channel" });
  await expect(telegram).toHaveAttribute("href", "https://t.me/+-lq4m5E_aT4xM2Y6");
  await expect(telegram.locator("img")).toHaveJSProperty("naturalWidth", 1024);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: "Open menu" })).toBeFocused();

  await page.evaluate(() => window.scrollTo(0, 180));
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
  const picker = await openCryptoPicker(page, "sending");
  await expect(picker).toBeVisible();

  const lockedAt = await page.locator("body").evaluate((body) => -parseFloat(body.style.top));
  expect(lockedAt).toBeGreaterThan(0);
  await page.mouse.move(5, 550);
  await page.mouse.wheel(0, 400);
  expect(await page.locator("body").evaluate((body) => -parseFloat(body.style.top))).toBe(lockedAt);

  const methods = picker.locator(".methods");
  await expect.poll(() => methods.evaluate((element) => element.scrollHeight - element.clientHeight)).toBeGreaterThan(0);
  await methods.evaluate((element) => element.scrollTo(0, 100));
  await expect.poll(() => methods.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);

  await page.keyboard.press("Escape");
  await expect(picker).not.toBeVisible();
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(lockedAt);
});

test("settings and exchange menus leave the page scrollable", async ({ page }, testInfo) => {
  const mobile = testInfo.project.name === "mobile";
  const height = mobile ? 600 : 500;
  await page.setViewportSize({ width: mobile ? 390 : 1280, height });
  await mockBackend(page);
  await openApp(page);

  for (const [button, dialog] of [
    ["Route refresh settings", "Refresh settings"],
    ["Choose exchanges", "Exchange settings"],
  ]) {
    await page.getByRole("button", { name: button }).click();
    await expect(page.getByRole("dialog", { name: dialog })).toBeVisible();
    await expect(page.locator("body")).not.toHaveCSS("position", "fixed");
    if (mobile) await expect(page.locator(".settingsBackdrop")).toHaveCSS("pointer-events", "none");
    await page.mouse.move(5, mobile ? 100 : height - 50);
    await page.mouse.wheel(0, 350);
    await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: dialog })).not.toBeVisible();
    await page.evaluate(() => window.scrollTo(0, 0));
  }
});

test("route guide uses normal page scrolling and returns to routes", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);
  const sourcePicker = await openCryptoPicker(page, "sending");
  await sourcePicker.getByRole("option", { name: /AMD.*Armenian dram/ }).click();
  await sourcePicker.getByLabel("Search banks and payment methods").fill("IDBank");
  await sourcePicker.getByRole("option", { name: /IDBank/ }).click();

  const targetPicker = await openCryptoPicker(page, "recipient");
  await targetPicker.getByRole("option", { name: /RUB.*Russian ruble/ }).click();
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();

  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("start-search").click();
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();

  const instructions = page.getByTestId("route-guide");
  await expect(instructions).toBeVisible();
  await expect(page.locator("body")).not.toHaveCSS("position", "fixed");
  await expect(page.locator(".workspace")).toBeHidden();
  await instructions.locator(".guideToolbar").getByRole("button", { name: /Back to routes/ }).click();
  await expect(instructions).toBeHidden();
  await expect(page.locator("body")).not.toHaveCSS("position", "fixed");
});

test("brand slogan and underline stay visible when resizing", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await mockBackend(page);
  await openApp(page);
  await expect(page.locator(".hero h1")).toHaveText("Move money. Keep more.");
  const underline = await page.locator(".hero h1 span").evaluate((span) => {
    const style = getComputedStyle(span, "::after");
    return { content: style.content, height: parseFloat(style.height), background: style.backgroundColor };
  });
  expect(underline.content).toBe('""');
  expect(underline.height).toBeGreaterThan(0);
  expect(underline.background).not.toBe("rgba(0, 0, 0, 0)");
  await expect(page.locator(".workspace")).toBeVisible();
  const viewport = page.viewportSize()!;
  await page.setViewportSize({ width: viewport.width - 40, height: viewport.height });
  await expect(page.locator(".workspace")).toBeVisible();
  await expect(page.locator(".introOverlay")).toHaveCount(0);
  expect(errors).toEqual([]);
});


test("original entrance animates the slogan and cleans up resize after docking", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await mockBackend(page);
  await page.goto("/");
  const intro = page.locator(".introOverlay");
  await expect(intro).toBeVisible();
  await expect(page.locator("html")).toHaveClass(/introPlaying/);
  await expect(page.locator("html")).toHaveCSS("scrollbar-width", "none");
  await expect(page.locator(".workspace")).toBeVisible();
  await expect(intro.locator(".introMarker")).toHaveCount(1);
  await expect(intro.locator(".introWordMore")).toHaveCSS("opacity", "1");
  await expect(intro).toHaveCount(0, { timeout: 6000 });
  await expect(page.locator("html")).not.toHaveClass(/introPlaying/);
  await expect(page.locator("html")).toHaveCSS("scrollbar-width", "auto");
  await page.setViewportSize({ width: 1024, height: 900 });
  await page.setViewportSize({ width: 393, height: 851 });
  await expect(page.locator(".hero h1")).toHaveText("Move money. Keep more.");
  await expect(page.locator(".workspace")).toBeVisible();
  expect(errors).toEqual([]);
});

async function openCryptoPicker(page: Page, side: "sending" | "recipient") {
  await page.getByRole("button", { name: new RegExp(`^Select ${side} (?:bank|payment method|asset):`) }).click();
  return page.getByRole("dialog", { name: side === "sending" ? "Choose where you pay from" : "Choose where the recipient gets paid" });
}

async function chooseCrypto(page: Page, side: "sending" | "recipient", search: string) {
  const picker = await openCryptoPicker(page, side);
  const [currency, ...networkTerms] = search.split(" ");
  await picker.getByRole("textbox", { name: "Currencies and digital assets" }).fill(currency);
  await picker.getByRole("option", { name: new RegExp(`^${currency}\\b`) }).click();
  await picker.getByRole("textbox", { name: "Blockchains" }).fill(networkTerms.join(" "));
  return picker;
}

for (const side of ["sending", "recipient"] as const) test(`native EVER selects Everscale for ${side} and survives reload`, async ({ page }) => {
  await mockBackend(page, { includeEverscale: true });
  const searches: Record<string, string>[] = [];
  await page.route("**/api/p2p/routes**", async route => {
    const params = Object.fromEntries(new URL(route.request().url()).searchParams);
    searches.push(params);
    await route.fulfill({ json: {
      search_id: "00000000-0000-4000-8000-000000000109", searched_at: new Date().toISOString(),
      source_fiat: params.source_fiat, target_fiat: params.target_fiat, source_amount: params.source_amount,
      assets_searched: [], routes_found: 0, routes: [], can_exchange_to_target: false,
      entry_sources: [], exit_sources: [],
    } });
  });
  await page.goto("/");
  await expect.poll(() => page.locator(".appShell").evaluate((element) => (element as HTMLElement).style.getPropertyValue("--puzzle-pattern"))).not.toBe("");
  await expect(page.locator(".workspace")).toBeVisible();
  await expect(page.locator(".introOverlay")).toHaveCount(0, { timeout: 6000 });
  const otherSide = side === "sending" ? "recipient" : "sending";
  const usdtPicker = await chooseCrypto(page, otherSide, "USDT Ethereum");
  await usdtPicker.getByRole("option", { name: /Ethereum.*USDT/ }).click();
  const everPicker = await chooseCrypto(page, side, "EVER Everscale");
  const native = everPicker.getByRole("option", { name: /Everscale.*EVER/ });
  await expect(native).toHaveCount(1);
  await expect.poll(() => native.locator('img[src="/icons/assets/ever.svg"]').first().evaluate((image: HTMLImageElement) => image.naturalWidth)).toBeGreaterThan(0);
  await expect(everPicker.getByRole("option", { name: /TON.*EVER/ })).toHaveCount(0);
  await native.click();
  const storageSide = side === "sending" ? "source" : "target";
  await expect.poll(() => page.evaluate(key => localStorage.getItem(key), `pay3flow.exchange.${storageSide}-method`)).toBe("global-ever");
  await expect(page.getByRole("button", { name: `Select ${side} network: Everscale`, exact: true })).toBeVisible();
  await page.getByLabel("Amount to send").fill("1.25");
  await expect.poll(() => searches.some(params => params.source_fiat === (side === "sending" ? "EVER" : "USDT")
    && params.target_fiat === (side === "recipient" ? "EVER" : "USDT")
    && params[`${storageSide}_network`] === "everscale" && params.source_amount === "1.25")).toBe(true);
  await expect(page.getByText("No routes found", { exact: true })).toBeVisible();
  await expect(page.getByTestId("wallet-swap-form")).toHaveCount(0);
  await page.reload();
  await expect(page.getByRole("button", { name: `Select ${side} network: Everscale`, exact: true })).toBeVisible();
  await expect(page.getByLabel("Amount to send")).toHaveValue("1.25");
});

async function mockBackend(page: Page, options: { includeNewProviders?: boolean; includeEverscale?: boolean; routeCount?: number; spotCycle?: boolean; spotVenue?: string; spotGuidance?: boolean; reviews?: boolean; usdtFiatNetwork?: string; mexc?: boolean; guideVenue?: string; cifra?: boolean } = {}) {
  await page.addInitScript(() => {
    if (!localStorage.getItem("pay3flow-locale")) localStorage.setItem("pay3flow-locale", "en");
    // These route fixtures use a saved AMD/RUB corridor independently of app defaults.
    if (!localStorage.getItem("pay3flow.exchange.source-method")) localStorage.setItem("pay3flow.exchange.source-method", "am-ameriabank");
    if (!localStorage.getItem("pay3flow.exchange.target-method")) localStorage.setItem("pay3flow.exchange.target-method", "ru-sberbank");
  });
  await page.route("**/api/**", async (route) => {
    const url = new URL(route.request().url());
    const method = route.request().method();
    const json = (value: unknown, status = 200) =>
      route.fulfill({ status, contentType: "application/json", body: JSON.stringify(value) });

    if (url.pathname === "/api/market-prices") {
      return json({
        source: "DefiLlama",
        stale: false,
        updated_at: Math.floor(Date.now() / 1000),
        prices: { USDT: 1, USDC: 1, BTC: 10000, ETH: 1000 },
      });
    }

    if (url.pathname === "/api/p2p/route-activity") {
      const end = Date.UTC(2026, 9, 4, 12);
      const oneHour = url.searchParams.get("period") === "1h";
      return json({
        source_currency: url.searchParams.get("source_currency"),
        target_currency: url.searchParams.get("target_currency"),
        hours: Array.from({ length: oneHour ? 60 : 168 }, (_, index) => ({
          started_at: new Date(end - ((oneHour ? 59 : 167) - index) * (oneHour ? 60_000 : 3_600_000)).toISOString(),
          count: oneHour ? (index % 20 === 0 ? 1 : 0) : (index % 12 === 0 ? 2 : 0),
        })),
      });
    }

    if (url.pathname === "/api/exchange/corridors") {
      return json({
        terms_version: "2026-09-19",
        items: [{
          id: "00000000-0000-4000-8000-000000000010",
          source_country: "AM",
          source_currency: "AMD",
          target_country: "RU",
          target_currency: "RUB",
          min_amount_minor: 1000,
          max_amount_minor: null,
          daily_limit_minor: 5_000_000_000,
          metadata: {},
        }],
      });
    }
    if (url.pathname === "/api/banks") {
      const method = (method_id: string, display_name: string, country: string, currency: string, icon_url: string, currency_group: string, popular = false) => ({
        method_id, name: method_id, display_name, role: "both", country, currency, kind: "bank", color: "#171a17", initials: display_name.slice(0, 2).toUpperCase(), popular, icon_url, p2p_query: display_name, currency_group,
      });
      const wallet = (currency: string, display_name: string) => ({
        ...method(`global-${currency.toLowerCase()}`, display_name, "GLOBAL", currency, `/icons/assets/${currency.toLowerCase()}.webp`, ""),
        kind: "wallet",
        initials: currency,
        p2p_query: currency,
        currency_group: null,
      });
      const currency = (id: string, display_name: string, initials: string, color: string) => ({
        ...method(`currency-${id.toLowerCase()}`, display_name, "GLOBAL", id, "", ""),
        kind: "currency", initials, color, p2p_query: id, currency_group: null,
      });
      const items = [
        currency("AMD", "Armenian dram", "֏", "#6d2c91"), currency("RUB", "Russian ruble", "₽", "#21a038"),
        currency("USD", "US dollar", "$", "#168451"), currency("BYN", "Belarusian ruble", "Br", "#006b3f"), currency("KZT", "Kazakhstani tenge", "₸", "#168451"),
        { ...method("global-usd-cash", "Cash USD", "GLOBAL", "USD", "", "cash", true), kind: "cash", initials: "$", p2p_query: "Cash" },
        { ...method("am-amd-cash", "Cash AMD", "AM", "AMD", "", "cash", true), kind: "cash", initials: "֏", p2p_query: "Cash" },
        { ...method("ru-rub-cash", "Cash RUB", "RU", "RUB", "", "cash", true), kind: "cash", initials: "₽", p2p_query: "Cash" },
        { ...method("by-byn-cash", "Cash BYN", "BY", "BYN", "", "cash", true), kind: "cash", initials: "Br", p2p_query: "Cash" },
        method("am-ameriabank", "Ameriabank", "AM", "AMD", "/icons/assets/ameriabank-green.png", "ameriabank", true),
        method("am-ameriabank-usd-account", "Ameriabank", "AM", "USD", "/icons/assets/ameriabank-green.png", "ameriabank", true),
        method("am-idbank", "IDBank", "AM", "AMD", "/icons/assets/idbank.png", "idbank", true),
        method("am-idbank-usd-account", "IDBank", "AM", "USD", "/icons/assets/idbank.png", "idbank", true),
        method("am-acba", "ACBA Bank", "AM", "AMD", "/icons/assets/acba.png", "acba", true),
        method("am-ardshinbank", "Ardshinbank", "AM", "AMD", "/icons/assets/ardshinbank.png", "ardshinbank"),
        method("am-inecobank", "Inecobank", "AM", "AMD", "/icons/assets/inecobank.png", "inecobank"),
        method("am-evocabank", "Evocabank", "AM", "AMD", "/icons/assets/evocabank.png", "evocabank"),
        method("ru-sberbank", "Sberbank", "RU", "RUB", "/icons/assets/sberbank.webp", "sberbank", true),
        method("ru-tbank", "T-Bank", "RU", "RUB", "/icons/assets/tbank.webp", "tbank", true),
        method("ru-tbank-usd-account", "T-Bank", "RU", "USD", "/icons/assets/tbank.webp", "tbank", true),
        method("ru-alfabank", "Alfa-Bank", "RU", "RUB", "/icons/assets/alfabank.webp", "alfabank", true),
        method("kz-kaspi", "Kaspi.kz", "KZ", "KZT", "/icons/assets/kz-kaspi.png", "kaspi", true),
        wallet("USDT", "Tether"), wallet("USDC", "USD Coin"), wallet("BTC", "Bitcoin"), wallet("ETH", "Ethereum"),
        ...(options.includeEverscale ? [{ ...wallet("EVER", "Everscale"), icon_url: "/icons/assets/ever.svg" }] : []),
      ];
      return json({ items, total: items.length, limit: 100, offset: 0 });
    }
    if (url.pathname === "/api/networks") {
      return json([
        { id: "ethereum", name: "Ethereum (ERC-20)", currencies: ["ETH", "USDT", "USDC"] },
        { id: "base", name: "Base", currencies: ["ETH", "USDC"] },
        { id: "tron", name: "TRON (TRC-20)", currencies: ["TRX", "USDT"] },
        { id: "ton", name: "TON", currencies: ["TON", "USDT"] },
        ...(options.includeEverscale ? [{ id: "everscale", name: "Everscale", currencies: ["EVER"] }] : []),
        { id: "bitcoin", name: "Bitcoin", currencies: ["BTC"] },
        { id: "near", name: "NEAR", currencies: ["BTC", "USDT"] },
        ...(options.guideVenue ? [{ id: "solana", name: "Solana", currencies: ["USDT", "USDC", "SOL"] }, { id: "polygon", name: "Polygon", currencies: ["USDT", "USDC"] }] : []),
      ]);
    }
    if (url.pathname === "/api/providers") {
      const providers: Array<Record<string, unknown>> = [
        { slug: "binance", name: "Binance", side: "sell", source_url: "https://p2p.binance.com", currencies: ["AMD", "RUB"], banks: [], searchable: true },
        { slug: "bybit", name: "Bybit", side: "sell", source_url: "https://www.bybit.com/fiat/trade/otc", currencies: ["AMD", "RUB"], banks: [], searchable: true },
        { slug: "cifra-broker", name: "Cifra Markets Sell", side: "sell", source_url: "https://cifra.by/", currencies: ["BYN", "RUB", "USD"], banks: [], searchable: true },
        { slug: "whitebird", name: "Whitebird Sell", side: "sell", source_url: "https://whitebird.io", currencies: ["BYN", "USD", "EUR", "RUB"], banks: [], searchable: false },
        { slug: "bestchange", name: "BestChange Sell", side: "sell", source_url: "https://bestchange.app/?lang=en", currencies: ["BYN", "EUR", "RUB", "USD"], banks: [], searchable: false },
        { slug: "dzengi", name: "Dzengi Sell", side: "sell", source_url: "https://dzengi.com/ru/kalkulyator-kriptovalyut", currencies: ["BYN", "EUR", "RUB", "USD"], banks: [], searchable: false },
        { slug: "cow-swap", name: "CoW Protocol Live Sell", side: "sell", source_url: "https://swap.cow.fi", currencies: ["USDC", "USDT"], banks: [], searchable: true, search_mode: "selectable" },
        { slug: "near-intents", name: "NEAR 1Click Sell", side: "sell", source_url: "https://1click.chaindefuser.com", currencies: ["BTC", "USDT"], banks: [], searchable: true, search_mode: "selectable" },
        { slug: "id-pay", name: "ID Pay Live Sell", side: "sell", source_url: "https://id-pay.ru/", currencies: ["AMD", "RUB"], banks: [], searchable: true, search_mode: "selectable" },
      ];
      if (options.reviews) {
        for (const provider of providers) {
          if (["cow-swap", "near-intents"].includes(String(provider.slug))) {
            provider.guidance = { description: "Review the provider before exchanging.", steps: [], links: [], review_sources: [{ kind: "trustscores", url: `https://trustscores.org/companies/${provider.slug}` }] };
          }
        }
      }
      if (options.includeNewProviders) {
        providers.push(
          { slug: "bitcoin-center", name: "Bitcoin Center Buy", side: "buy", source_url: "https://www.bitcoincenter.am", currencies: ["AMD"], banks: ["Bank Transfer"], searchable: true, search_mode: "selectable" },
          { slug: "bncex", name: "bncex Buy", side: "buy", source_url: "https://www.bncex.com/en", currencies: ["AMD"], banks: [], searchable: true, search_mode: "selectable" },
          { slug: "skylabs", name: "SkyLabs Buy", side: "buy", source_url: "https://skylabs.world", currencies: ["AMD"], banks: [], searchable: true, search_mode: "selectable" },
          { slug: "symbiosis", name: "Symbiosis Buy", side: "buy", source_url: "https://api.symbiosis.finance", currencies: [], banks: [], searchable: true, search_mode: "selectable" },
        );
      }
      if (options.guideVenue && options.guideVenue !== "cifra-broker") providers.push({ slug: options.guideVenue, name: ({ "bitcoin-center": "Bitcoin Center", "cow-swap": "CoW Swap", bestchange: "BestChange", symbiosis: "Symbiosis", dzengi: "Dzengi", bncex: "bncex", binance: "Binance", bybit: "Bybit", mexc: "MEXC", bitget: "Bitget", whitebird: "Whitebird" } as Record<string, string>)[options.guideVenue], side: "sell", source_url: "https://example.com/", currencies: ["AMD", "RUB"], banks: [], searchable: true });
      if (options.spotGuidance) for (const provider of providers) {
        if (["binance", "bybit", "mexc", "bitget", "whitebird"].includes(String(provider.slug))) provider.guidance = { description: "P2P only", steps: ["Open the P2P advertiser profile."], links: [{ label: "Open P2P", url: "https://p2p.binance.com" }] };
      }
      if (options.mexc) providers.push({ slug: "mexc", name: "MEXC", side: "sell", source_url: "https://www.mexc.com/buy-crypto/p2p", currencies: ["AMD", "RUB"], banks: [], searchable: true });
      return json(providers);
    }
    if (url.pathname === "/api/service-executions/open" && method === "POST") {
      return json({
        execution_id: "00000000-0000-4000-8000-000000000301",
        newly_recorded: true,
        redirect_url: "about:blank",
        service: { id: "00000000-0000-4000-8000-000000000202", slug: "bybit", display_name: "Bybit", executions_total: 6201, likes_total: 850, dislikes_total: 40 },
      });
    }
    if (url.pathname === "/api/routes/route-2/vote" && method === "PUT") {
      return json({ likes_total: 1, dislikes_total: 0, viewer_vote: "like" });
    }
    if (url.pathname === "/api/p2p/routes") {
      const offer = (source: string, adId: string, fiat: string, asset: string) => ({
        source,
        ad_id: adId,
        fiat,
        asset,
        network: source === "bitcoin-center" ? "solana" : source === "bncex" ? "tron" : null,
        price: "1",
        available_asset: "1000000",
        min_fiat: "1000",
        max_fiat: "10000000",
        payment_methods: ["Bank transfer"],
        pay_time_limit_minutes: 15,
        advertiser: {
          id: source === "bybit" ? `masked-${adId}` : null,
          nickname: source === "bestchange" ? "ChangerBiz" : `${source}-merchant`,
          user_type: (["bncex", "bitcoin-center", "dzengi", "bestchange", "cifra-broker"].includes(source) ? "service" : "merchant") as string | null,
          is_merchant: true,
          is_verified: true,
          completed_orders_30d: 300 as number | null,
          completion_rate_30d: 0.99 as number | null,
        },
        source_url: `https://example.com/${adId}`,
        advertiser_profile_url: source === "bybit" ? `https://www.bybit.com/en/p2p/profile/masked-${adId}/${asset}/${fiat}/item` : source === "bestchange" ? "https://www.bestchange.pro/changerbiz-exchanger.html#pay3flow-id=125" : source === "binance" && options.guideVenue ? "https://c2c.binance.com/ru/advertiserDetail?advertiserNo=s6b4151aab3223e9a87d491ca4256411b" : source === "mexc" ? "https://www.mexc.com/buy-crypto/merchant/af2545ef4b6c470ebd516aa794bb7a03" : options.reviews ? `https://c2c.binance.com/en/advertiserDetail?advertiserNo=${adId}` : null,
      });
      if (options.cifra && url.searchParams.get("source_fiat") === "USD") {
        return json({ search_id: "00000000-0000-4000-8000-000000000103", routes_found: 1, searched_at: "2026-10-10T12:00:00Z", source_fiat: "USD", target_fiat: "RUB", source_amount: "100000.00", assets_searched: ["USDT"], can_exchange_to_target: true, routes: [{
          route_id: "cifra-usd-rub", rank: 1, asset: "USDT", entry_network: "tron", source_fiat: "USD", source_amount: "100000.00", acquired_asset_amount: "99000.00", target_fiat: "RUB", target_amount: "7000000.00", effective_rate: "70.00", same_venue: true, requires_asset_transfer: false, transfer_fee_included: false, route_kind: "fiat_to_fiat", payment_methods_verified: false,
          entry_offer: offer("cifra-broker", "cifra-entry", "USD", "USDT"), exit_offer: offer("cifra-broker", "cifra-exit", "RUB", "USDT"), warnings: ["Search estimate only."], services: [], reputation: { executions_average: 0, likes_average: 0, dislikes_average: 0 },
        }] });
      }
      if (url.searchParams.get("source_fiat") === "AMD" && url.searchParams.get("target_fiat") === "AMD") {
        expect(url.searchParams.get("source_payment_method")).toBe("Ameriabank");
        expect(url.searchParams.get("target_payment_method")).toBe("IDBank");
        return json({
          search_id: "00000000-0000-4000-8000-000000000110",
          routes_found: 1,
          searched_at: "2026-10-02T10:00:00Z",
          source_fiat: "AMD",
          target_fiat: "AMD",
          source_amount: "10000.00",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-amd-usdt-amd",
            rank: 1,
            asset: "USDT",
            entry_network: "tron",
            source_network: null,
            target_network: null,
            source_fiat: "AMD",
            source_amount: "10000.00",
            acquired_asset_amount: "27.70000000",
            target_fiat: "AMD",
            target_amount: "9900.00",
            effective_rate: "0.99000000",
            same_venue: false,
            requires_asset_transfer: true,
            transfer_fee_included: false,
            route_kind: "crypto_cycle",
            profitability: {
              status: "unconfirmed",
              gross_profit_minor: -10000,
              gross_profit_bps: -100,
              missing_costs: ["network_fee"],
            },
            payment_methods_verified: true,
            entry_offer: offer("binance", "entry-amd-cycle", "AMD", "USDT"),
            exit_offer: offer("bybit", "exit-amd-cycle", "AMD", "USDT"),
            warnings: ["Network fee must be confirmed before execution."],
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "USD" && url.searchParams.get("target_fiat") === "AMD") {
        expect(url.searchParams.get("source_payment_method")).toBe("Cash");
        expect(url.searchParams.get("target_payment_method")).toBe("Ameriabank");
        const entry = offer("skylabs", "usd-cash-entry", "USD", "USDT");
        entry.payment_methods = ["SkyLabs ATM"];
        const exit = offer("skylabs", "amd-bank-exit", "AMD", "USDT");
        exit.payment_methods = ["Ameriabank"];
        return json({
          search_id: "00000000-0000-4000-8000-000000000108",
          routes_found: 1,
          searched_at: "2026-09-27T10:00:00Z",
          source_fiat: "USD",
          target_fiat: "AMD",
          source_amount: "12000.00",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-usd-cash-amd",
            rank: 1,
            asset: "USDT",
            entry_network: null,
            source_network: null,
            target_network: null,
            source_fiat: "USD",
            source_amount: "12000.00",
            acquired_asset_amount: "12000.00",
            target_fiat: "AMD",
            target_amount: "4620000.00",
            effective_rate: "385.00000000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "fiat_to_fiat",
            payment_methods_verified: true,
            entry_offer: entry,
            exit_offer: exit,
            warnings: ["Cash exchange limits and identity checks are set by the provider."],
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "USD" && url.searchParams.get("target_fiat") === "USD") {
        const sourcePaymentMethod = url.searchParams.get("source_payment_method");
        const isReverseBankRoute = sourcePaymentMethod === "Ameriabank";
        const targetPaymentMethod = url.searchParams.get("target_payment_method");
        expect(sourcePaymentMethod).toBe(isReverseBankRoute ? "Ameriabank" : "T-Bank");
        expect(targetPaymentMethod).toBe(isReverseBankRoute ? "T-Bank" : "Ameriabank");
        const entry = offer(isReverseBankRoute ? "okx" : "skylabs", "usd-bank-entry", "USD", "USDT");
        entry.payment_methods = [sourcePaymentMethod ?? "Bank Transfer"];
        const exit = offer(isReverseBankRoute ? "skylabs" : "okx", "usd-bank-exit", "USD", "USDT");
        exit.payment_methods = [targetPaymentMethod ?? "Bank Transfer"];
        return json({
          search_id: "00000000-0000-4000-8000-000000000109",
          routes_found: 1,
          searched_at: "2026-09-27T10:00:00Z",
          source_fiat: "USD",
          target_fiat: "USD",
          source_amount: "12000.00",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-usd-bank-usdt-usd-bank",
            rank: 1,
            asset: "USDT",
            entry_network: "tron",
            source_network: null,
            target_network: null,
            source_fiat: "USD",
            source_amount: "12000.00",
            acquired_asset_amount: "11881.18811881",
            target_fiat: "USD",
            target_amount: "11643.56",
            effective_rate: "0.97029667",
            same_venue: false,
            requires_asset_transfer: true,
            transfer_fee_included: true,
            route_kind: "fiat_to_fiat",
            payment_methods_verified: true,
            entry_offer: entry,
            exit_offer: exit,
            warnings: ["Bank-account eligibility and transfer limits must be confirmed with the provider."],
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "RUB" && url.searchParams.get("target_fiat") === "RUB") {
        const sbpOffer = (source: string, adId: string, fiat: string) => ({
          ...offer(source, adId, fiat, "USDT"),
          payment_methods: ["СБП"],
        });
        expect(url.searchParams.get("source_payment_method")).toBe("Sberbank");
        expect(url.searchParams.get("target_payment_method")).toBe("Alfa-Bank");
        return json({
          search_id: "00000000-0000-4000-8000-000000000107",
          routes_found: 1,
          searched_at: "2026-09-25T10:00:00Z",
          source_fiat: "RUB",
          target_fiat: "RUB",
          source_amount: "10000.00",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-rub-sber-alfa-sbp",
            rank: 1,
            asset: "USDT",
            entry_network: null,
            source_network: null,
            target_network: null,
            source_fiat: "RUB",
            source_amount: "10000.00",
            acquired_asset_amount: "100.00000000",
            target_fiat: "RUB",
            target_amount: "9900.00",
            effective_rate: "0.99000000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "fiat_to_fiat",
            payment_methods_verified: true,
            entry_offer: sbpOffer("binance", "entry-rub-sbp", "RUB"),
            exit_offer: sbpOffer("binance", "exit-rub-sbp", "RUB"),
            warnings: ["Search estimate only."],
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "USDC") {
        expect(url.searchParams.get("source_network")).toBe("ethereum");
        const whitebird = offer("whitebird", "whitebird-sell-RUB-USDC", "RUB", "USDC");
        whitebird.price = "83.4295";
        whitebird.payment_methods = [];
        whitebird.advertiser = {
          ...whitebird.advertiser,
          id: null,
          nickname: "Whitebird",
          user_type: "service",
          completed_orders_30d: null,
          completion_rate_30d: null,
        };
        whitebird.source_url = "https://whitebird.io/";
        return json({
          search_id: "00000000-0000-4000-8000-000000000105",
          routes_found: 1,
          searched_at: "2026-09-23T10:00:00Z",
          source_fiat: "USDC",
          target_fiat: "RUB",
          source_amount: "100.00",
          assets_searched: ["USDC"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-usdc-whitebird-rub",
            rank: 1,
            asset: "USDC",
            entry_network: "ethereum",
            source_network: "ethereum",
            target_network: null,
            source_fiat: "USDC",
            source_amount: "100.00000000",
            acquired_asset_amount: "100.00000000",
            target_fiat: "RUB",
            target_amount: "8342.95",
            effective_rate: "83.42950000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "crypto_to_fiat",
            payment_methods_verified: false,
            entry_offer: null,
            exit_offer: whitebird,
            warnings: ["Search estimate only."],
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "USDT" && url.searchParams.get("target_fiat") === "USDC") {
        expect(url.searchParams.get("source_network")).toBe("ethereum");
        expect(url.searchParams.get("target_network")).toBe("ethereum");
        const directRoute = (provider: string, amount: string, rank: number) => ({
          route_id: `route-usdt-usdc-${provider}`,
          rank,
          asset: "USDC",
          entry_network: "ethereum",
          source_network: "ethereum",
          target_network: "ethereum",
          source_fiat: "USDT",
          source_amount: "100",
          acquired_asset_amount: amount,
          target_fiat: "USDC",
          target_amount: amount,
          effective_rate: (Number(amount) / 100).toFixed(12),
          same_venue: false,
          requires_asset_transfer: true,
          transfer_fee_included: true,
          route_kind: "crypto_to_crypto",
          route_provider: provider,
          route_path: ["USDT@ethereum", "USDC@ethereum"],
          route_fees: [{ asset: "USDT@ethereum", amount: provider === "cow-swap" ? "2.5" : "0.25" }],
          quote_expires_at: "2030-03-17T17:46:40Z",
          execution: {
            provider,
            from_asset: "USDT@ethereum",
            to_asset: "USDC@ethereum",
            input_amount: "100",
            expires_at: "2030-03-17T17:46:40Z",
            token: `signed-${provider}-route-token`,
          },
          payment_methods_verified: true,
          entry_offer: null,
          exit_offer: null,
          warnings: [`Live dry quote from ${provider}.`],
        });
        return json({
          search_id: "00000000-0000-4000-8000-000000000107",
          routes_found: 2,
          searched_at: "2026-09-26T10:00:00Z",
          source_fiat: "USDT",
          target_fiat: "USDC",
          source_amount: "100",
          assets_searched: [],
          can_exchange_to_target: true,
          routes: [directRoute("near-intents", "99.6", 1), directRoute("cow-swap", "97.3", 2)],
        });
      }
      if (url.searchParams.get("source_fiat") === "USDT" && url.searchParams.get("target_fiat") === "USDT") {
        if (url.searchParams.get("source_network") === "ethereum" && url.searchParams.get("target_network") === "ethereum") {
          const cycleLegs = options.spotCycle ? [
            { provider: options.spotVenue ?? "bybit", market_pair: "ETHUSDT", description: "Spot ETHUSDT", from_asset: "USDT@ethereum", to_asset: "ETH", input_amount: "100", output_amount: "1" },
            { provider: options.spotVenue ?? "bybit", market_pair: "BTCETH", description: "Spot BTCETH", from_asset: "ETH", to_asset: "BTC", input_amount: "1", output_amount: "0.5" },
            { provider: options.spotVenue ?? "bybit", market_pair: "BTCUSDT", description: "Spot BTCUSDT", from_asset: "BTC", to_asset: "USDT@ethereum", input_amount: "0.5", output_amount: "101" },
          ] : [
            { provider: "cow-swap", from_asset: "USDT@ethereum", to_asset: "USDC@ethereum", input_amount: "100", output_amount: "99", source_url: "https://swap.cow.fi" },
            { provider: "near-intents", from_asset: "USDC@ethereum", to_asset: "USDT@ethereum", input_amount: "99", output_amount: "101", source_url: "https://1click.chaindefuser.com" },
          ];
          return json({
            search_id: "00000000-0000-4000-8000-000000000109", routes_found: 1,
            routes_exhaustive: false, searched_at: "2026-10-07T10:00:00Z",
            source_fiat: "USDT", target_fiat: "USDT", source_amount: "100",
            assets_searched: [], can_exchange_to_target: true,
            routes: [{
              route_id: "crypto-cycle", rank: 1, asset: "USDC",
              source_fiat: "USDT", target_fiat: "USDT", source_amount: "100",
              target_amount: "101", acquired_asset_amount: "99", effective_rate: "1.01",
              source_network: "ethereum", target_network: "ethereum", entry_network: "ethereum",
              same_venue: false, requires_asset_transfer: true, transfer_fee_included: false,
              route_kind: "crypto_cycle", profitability_decimals: 8,
              profitability: { status: "unconfirmed", gross_profit_minor: 100000000, gross_profit_bps: 100, missing_costs: ["network_fee"] },
              route_path: [cycleLegs[0].from_asset, ...cycleLegs.map((leg) => leg.to_asset)],
              cycle_legs: cycleLegs,
              payment_methods_verified: true, warnings: [],
            }],
          });
        }
        expect(url.searchParams.get("source_network")).toBe("tron");
        expect(url.searchParams.get("target_network")).toBe("ton");
        return json({ error: "No live bridge provider is configured for USDT: TRON (TRC-20) → TON" }, 400);
      }
      if (url.searchParams.get("source_fiat") === "USDT") {
        expect(url.searchParams.get("source_network")).toBe(options.usdtFiatNetwork ?? "ethereum");
        expect(url.searchParams.has("source_payment_method")).toBe(false);
        return json({
          search_id: "00000000-0000-4000-8000-000000000101",
          routes_found: 1,
          searched_at: "2026-09-19T10:00:00Z",
          source_fiat: "USDT",
          target_fiat: "RUB",
          source_amount: "125.00",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-usdt-rub",
            rank: 1,
            asset: "USDT",
            entry_network: options.usdtFiatNetwork ?? "ethereum",
            source_network: options.usdtFiatNetwork ?? "ethereum",
            target_network: null,
            source_fiat: "USDT",
            source_amount: "125.00000000",
            acquired_asset_amount: "125.00000000",
            target_fiat: "RUB",
            target_amount: "11250.00",
            effective_rate: "90.00000000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "crypto_to_fiat",
            payment_methods_verified: true,
            entry_offer: null,
            exit_offer: offer("binance", "exit-erc20", "RUB", "USDT"),
            warnings: ["Search estimate only."],
            services: [{ id: "00000000-0000-4000-8000-000000000201", slug: "binance", display_name: "Binance", executions_total: 12400, likes_total: 1800, dislikes_total: 74 }],
            reputation: { executions_average: 12400, likes_average: 1800, dislikes_average: 74 },
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "ETH") {
        expect(url.searchParams.get("source_network")).toBe("base");
        expect(url.searchParams.get("target_network")).toBe(options.guideVenue ? "polygon" : "ton");
        return json({
          search_id: "00000000-0000-4000-8000-000000000102",
          routes_found: 1,
          searched_at: "2026-09-19T10:00:00Z",
          source_fiat: "ETH",
          target_fiat: "USDT",
          source_amount: "0.03",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-eth-usdt",
            rank: 1,
            asset: "USDT",
            entry_network: "base",
            source_network: "base",
            target_network: options.guideVenue ? "polygon" : "ton",
            source_fiat: "ETH",
            source_amount: "0.030000000000",
            acquired_asset_amount: "80.100000000000",
            target_fiat: "USDT",
            target_amount: "80.100000000000",
            effective_rate: "2670.000000000000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: false,
            route_kind: "crypto_to_crypto",
            bridge_currency: null,
            route_provider: ["symbiosis", "cow-swap"].includes(options.guideVenue ?? "") ? options.guideVenue : null,
            route_provider_url: options.guideVenue === "symbiosis" ? "https://app.symbiosis.finance/swap" : "https://swap.cow.fi/",
            market_path: ["symbiosis", "cow-swap"].includes(options.guideVenue ?? "") ? null : {
              venue: options.guideVenue ?? "binance",
              source_pair: "ETHUSDT",
              target_pair: "ETHUSDT",
              source_rate: "2670.000000000000",
              target_rate: "1.000000000000",
              intermediary_amount: "80.100000000000",
            },
            payment_methods_verified: true,
            entry_offer: null,
            exit_offer: null,
            warnings: ["Network availability is not verified."],
            services: [{ id: "00000000-0000-4000-8000-000000000201", slug: "binance", display_name: "Binance", executions_total: 12400, likes_total: 1800, dislikes_total: 74 }],
            reputation: { executions_average: 12400, likes_average: 1800, dislikes_average: 74 },
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "BTC") {
        expect(url.searchParams.get("source_network")).toBe("bitcoin");
        expect(url.searchParams.get("target_network")).toBe("ton");
        return json({
          search_id: "00000000-0000-4000-8000-000000000104",
          routes_found: 1,
          searched_at: "2026-09-19T10:00:00Z",
          source_fiat: "BTC",
          target_fiat: "USDT",
          source_amount: "0.002",
          assets_searched: ["USDC"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-btc-usdc-usdt",
            rank: 1,
            asset: "USDC",
            entry_network: "bitcoin",
            source_network: "bitcoin",
            target_network: "ton",
            source_fiat: "BTC",
            source_amount: "0.002000000000",
            acquired_asset_amount: "126.000000000000",
            target_fiat: "USDT",
            target_amount: "125.800000000000",
            effective_rate: "62900.000000000000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: false,
            route_kind: "crypto_to_crypto",
            bridge_currency: "USDC",
            market_path: {
              venue: "binance",
              source_pair: "BTCUSDC",
              target_pair: "USDCUSDT",
              source_rate: "63000.000000000000",
              target_rate: "0.998400000000",
              intermediary_amount: "126.000000000000",
            },
            payment_methods_verified: true,
            entry_offer: null,
            exit_offer: null,
            warnings: ["Network availability is not verified."],
            services: [{ id: "00000000-0000-4000-8000-000000000201", slug: "binance", display_name: "Binance", executions_total: 12400, likes_total: 1800, dislikes_total: 74 }],
            reputation: { executions_average: 12400, likes_average: 1800, dislikes_average: 74 },
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "AMD" && url.searchParams.get("target_fiat") === "BTC") {
        expect(url.searchParams.get("source_amount")).toBe("10000");
        expect(url.searchParams.get("target_network")).toBe("near");
        expect(url.searchParams.get("source_payment_method")).toBe("Ameriabank");
        return json({
          search_id: "00000000-0000-4000-8000-000000000106",
          routes_found: 1,
          searched_at: "2026-09-25T10:00:00Z",
          source_fiat: "AMD",
          target_fiat: "BTC",
          source_amount: "10000.00",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-amd-usdt-btc-near",
            rank: 1,
            asset: "USDT",
            entry_network: "optimism",
            source_network: null,
            target_network: "near",
            source_fiat: "AMD",
            source_amount: "10000.00",
            acquired_asset_amount: "2.77777778",
            target_fiat: "BTC",
            target_amount: "0.00032478",
            effective_rate: "0.000000032478",
            same_venue: false,
            requires_asset_transfer: true,
            transfer_fee_included: true,
            route_kind: "fiat_to_crypto",
            route_provider: "near-intents",
            route_path: ["AMD", "USDT@optimism", "BTC@near"],
            execution: {
              provider: "near-intents",
              from_asset: "USDT@optimism",
              to_asset: "BTC@near",
              input_amount: "2.77777778",
              expires_at: "2030-03-17T17:46:40Z",
              token: "signed-near-route-token",
            },
            payment_methods_verified: true,
            entry_offer: offer("bybit", "entry-amd-usdt", "AMD", "USDT"),
            exit_offer: null,
            warnings: ["Live dry quote from near-intents; execution and wallet compatibility are not verified."],
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "AMD" && url.searchParams.get("target_fiat") === "RUB" && ["10000", "42269"].includes(url.searchParams.get("source_amount") ?? "")) {
        const isTargetAmountProbe = url.searchParams.get("source_amount") === "10000";
        const sourceAmount = isTargetAmountProbe ? "10000.00" : "42269.00";
        const targetAmount = isTargetAmountProbe ? "2365.80" : "10000.00";
        const idPay = offer("id-pay", "indicative-amd-rub", "AMD", "RUB");
        idPay.price = "4.2269";
        idPay.payment_methods = ["IDBank", "Alfa-Bank"];
        idPay.advertiser = {
          ...idPay.advertiser,
          id: null,
          nickname: "ID Pay",
          user_type: "service",
          completed_orders_30d: null,
          completion_rate_30d: null,
        };
        idPay.source_url = "https://id-pay.ru/";
        return json({
          search_id: "00000000-0000-4000-8000-000000000108",
          routes_found: 1,
          searched_at: "2026-09-26T11:00:00Z",
          source_fiat: "AMD",
          target_fiat: "RUB",
          source_amount: sourceAmount,
          assets_searched: [],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-amd-rub-id-pay",
            rank: 1,
            asset: "RUB",
            source_fiat: "AMD",
            source_amount: sourceAmount,
            acquired_asset_amount: targetAmount,
            target_fiat: "RUB",
            target_amount: targetAmount,
            effective_rate: "0.23657900",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "fiat_to_fiat",
            payment_methods_verified: false,
            entry_offer: idPay,
            exit_offer: null,
            warnings: ["Indicative direct-transfer quote."],
          }],
        });
      }
      expect(url.searchParams.get("source_payment_method")).toBe("IDBank");
      expect(url.searchParams.get("target_payment_method")).toBe("Alfa-Bank");
      expect(url.searchParams.get("source_fiat")).toBe("AMD");
      expect(url.searchParams.get("target_fiat")).toBe("RUB");
      expect(url.searchParams.get("allow_cross_venue")).toBe("true");
      const routes = Array.from({ length: options.routeCount ?? 12 }, (_, index) => {
        const best = index === 0;
        const asset = index % 2 === 0 ? "USDT" : "USDC";
        const venue = options.guideVenue ?? (options.mexc ? "mexc" : index % 2 === 0 ? "binance" : "bybit");
        const exitVenue = ["bncex", "bitcoin-center", "dzengi"].includes(venue) || index === 2 ? "bybit" : venue;
        return {
          route_id: `route-${index + 1}`,
          rank: index + 1,
          asset,
          source_fiat: "AMD",
          source_amount: "100000.00",
          acquired_asset_amount: best ? "253.16455696" : "252.52525252",
          target_fiat: "RUB",
          target_amount: best ? "20350.00" : (20120 - index * 20).toFixed(2),
          effective_rate: best ? "0.20350000" : "0.20100000",
          entry_network: index === 2 ? "ethereum" : undefined,
          same_venue: index !== 2,
          requires_asset_transfer: index === 2,
          transfer_fee_included: true,
          payment_methods_verified: best,
          entry_offer: offer(venue, `entry-${index + 1}`, "AMD", asset),
          exit_offer: offer(exitVenue, `exit-${index + 1}`, "RUB", asset),
          warnings: ["Search estimate only."],
          services: [{ id: venue === "binance" ? "00000000-0000-4000-8000-000000000201" : "00000000-0000-4000-8000-000000000202", slug: venue, display_name: venue === "mexc" ? "MEXC" : venue === "binance" ? "Binance" : "Bybit", executions_total: best ? 12400 : 6200, likes_total: best ? 1800 : 850, dislikes_total: best ? 74 : 40 }],
          reputation: { executions_average: best ? 12400 : 6200, likes_average: best ? 1800 : 850, dislikes_average: best ? 74 : 40 },
          service_links: index === 1 ? [
            { service_id: "00000000-0000-4000-8000-000000000202", service_slug: "bybit", kind: "entry", tracking_token: "entry-token" },
            { service_id: "00000000-0000-4000-8000-000000000202", service_slug: "bybit", kind: "exit", tracking_token: "exit-token" },
          ] : [],
        };
      });
      return json({
        search_id: "00000000-0000-4000-8000-000000000103",
        routes_found: options.routeCount ?? 24,
        searched_at: "2026-09-19T10:00:00Z",
        source_fiat: "AMD",
        target_fiat: "RUB",
        source_amount: "100000.00",
        assets_searched: ["USDT", "USDC", "BTC", "ETH"],
        can_exchange_to_target: true,
        routes,
      });
    }
    return json({ error: `unmocked ${method} ${url.pathname}` }, 500);
  });
}

test("public P2P route search → open step-by-step instructions", async ({ page, isMobile }) => {
  await mockBackend(page, { routeCount: 101 });
  const fixturePath = "/#/swap/AMD/RUB?from=am-ameriabank&to=ru-sberbank";
  await openApp(page, fixturePath);

  const backgroundPattern = await expect
    .poll(() => page.locator(".appShell").evaluate((element) => getComputedStyle(element, "::before").backgroundImage))
    .not.toBe("none")
    .then(() =>
      page.locator(".appShell").evaluate((element) => getComputedStyle(element, "::before").backgroundImage),
    );
  await page.evaluate(path => history.replaceState(history.state, "", path), fixturePath);
  await page.reload();
  await expect
    .poll(async () => {
      const nextPattern = await page
        .locator(".appShell")
        .evaluate((element) => getComputedStyle(element, "::before").backgroundImage);
      return nextPattern !== "none" && nextPattern !== backgroundPattern;
    })
    .toBe(true);

  await expect(page.getByTestId("auth-form")).toHaveCount(0);
  await expect(page.getByLabel("Amount to send")).toHaveValue("0");

  const amountInput = page.getByLabel("Amount to send");
  await amountInput.fill("123");
  await expect(amountInput).toHaveValue("123");
  await amountInput.fill("12б5");
  await expect(amountInput).toHaveValue("12,5");
  await amountInput.fill("12ю5");
  await expect(amountInput).toHaveValue("12.5");
  await amountInput.fill("0");

  const swapDirection = page.getByRole("button", { name: "Swap sender and recipient" });
  await swapDirection.click();
  await expect(page.getByRole("button", { name: "Select sending bank: Sberbank" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Select recipient bank: Ameriabank" })).toBeVisible();
  await swapDirection.click();
  await expect(page.getByRole("button", { name: "Select sending bank: Ameriabank" })).toBeVisible();

  await page.getByRole("button", { name: "Route refresh settings" }).click();
  const refreshSettings = page.getByRole("dialog", { name: "Refresh settings" });
  if ((page.viewportSize()?.width ?? 0) > 640) await expect(refreshSettings).toHaveCSS("width", "310px");
  await refreshSettings.getByRole("button", { name: "5m" }).click();
  await expect.poll(() => page.evaluate(() => localStorage.getItem("pay3flow.exchange.refresh-seconds"))).toBe("300");

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await expect(sourcePicker).toBeVisible();
  await sourcePicker.getByRole("option", { name: /^AMD / }).click();
  await sourcePicker.getByLabel("Search banks and payment methods").fill("IDBank");
  await sourcePicker.getByRole("option", { name: /IDBank/ }).click();
  await expect(page.getByRole("button", { name: "Select sending bank: IDBank" })).toBeVisible();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await expect(targetPicker).toBeVisible();
  await targetPicker.getByRole("option", { name: /^RUB / }).click();
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();
  await expect(page.getByRole("button", { name: "Select recipient bank: Alfa-Bank" })).toBeVisible();

  await page.evaluate(() => {
    const browserWindow = window as Window & { __routeRenderSamples?: Array<{ count: number; at: number }> };
    browserWindow.__routeRenderSamples = [];
    let previousCount = 0;
    new MutationObserver(() => {
      const count = document.querySelectorAll('[data-testid="complete-route"]').length;
      if (count > 0 && count !== previousCount) {
        browserWindow.__routeRenderSamples?.push({ count, at: performance.now() });
        previousCount = count;
      }
    }).observe(document.body, { childList: true, subtree: true });
  });
  await amountInput.fill("100000");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(101);
  await expect(page.getByTestId("start-search")).toHaveText("Go ↗");
  await expect(page.getByTestId("start-search")).toHaveAttribute("aria-label", "Open route instructions");
  await expect(page.getByText("101 routes found")).toBeVisible();
  const routeCounter = page.locator(".resultSummary small[aria-live='polite']");
  const toggleLanguage = async () => {
    const current = await page.locator("html").getAttribute("lang") as "en" | "ru" | "hy";
    await selectLanguage(page, ({ en: "ru", ru: "hy", hy: "en" } as const)[current]);
  };
  await toggleLanguage();
  await expect(routeCounter).toHaveText("101 маршрут найден");
  await toggleLanguage();
  await toggleLanguage();
  await expect(routeCounter).toHaveText("101 routes found");
  const routeRenderSamples = await page.evaluate(() =>
    (window as Window & { __routeRenderSamples?: Array<{ count: number; at: number }> }).__routeRenderSamples ?? [],
  );
  expect(routeRenderSamples.map((sample) => sample.count)).toEqual([100, 101]);
  expect(routeRenderSamples[1].at - routeRenderSamples[0].at).toBeGreaterThanOrEqual(5);
  await expect(page.getByTestId("complete-route").first()).not.toContainText("Used");
  await expect(page.getByTestId("complete-route").first()).toContainText("20350 RUB");
  const bestRoute = page.getByTestId("complete-route").first();
  const alternativeRoute = page.getByTestId("complete-route").nth(1);
  await alternativeRoute.locator(".routeRank").click();
  await expect(alternativeRoute).toHaveClass(/selected/);
  await expect(bestRoute).not.toHaveClass(/selected/);
  await expect(alternativeRoute.getByRole("button", { name: /Select route 2:/ })).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("complete-route").first().locator(".routeWorkflowButton")).toHaveAttribute(
    "aria-label",
    "AMD · IDBank → USDT (Binance) → RUB · Alfa-Bank (Binance)",
  );
  const routeGroups = page.getByTestId("route-groups");
  const scrollMetrics = await routeGroups.evaluate((element) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
    overflowY: getComputedStyle(element).overflowY,
  }));
  expect(scrollMetrics.overflowY).toBe("auto");
  expect(scrollMetrics.scrollHeight).toBeGreaterThan(scrollMetrics.clientHeight);
  await expect(routeGroups).toHaveCSS("scrollbar-width", "auto");
  await routeGroups.hover();
  await page.mouse.wheel(0, 360);
  await expect.poll(() => routeGroups.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  if ((page.viewportSize()?.width ?? 0) > 980) {
    await page.getByRole("button", { name: "Show search activity" }).click();
    const expandedScrollMetrics = await routeGroups.evaluate((element) => ({ clientHeight: element.clientHeight, scrollHeight: element.scrollHeight }));
    expect(expandedScrollMetrics.scrollHeight).toBeGreaterThan(expandedScrollMetrics.clientHeight);
    await routeGroups.evaluate((element) => element.scrollTo({ top: 0 }));
    await routeGroups.hover();
    await page.mouse.wheel(0, 360);
    await expect.poll(() => routeGroups.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    await page.getByRole("button", { name: "Hide search activity" }).click();
  }
  await routeGroups.evaluate((element) => element.scrollTo({ top: element.scrollHeight }));
  await expect(page.getByTestId("complete-route").last()).toBeVisible();
  await routeGroups.evaluate((element) => element.scrollTo({ top: 0 }));
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();
  const instructions = page.getByTestId("route-guide");
  await expect(instructions).toBeVisible();
  await expect(instructions.getByRole("list", { name: "Exchange steps" })).toBeVisible();
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await instructions.getByTestId("start-guide").click();
  await expect(instructions.getByRole("heading", { name: "Buy USDT for 100,000 AMD" })).toBeVisible();
  await expect(instructions.locator(".frameList")).toContainText("Open More details");
  await instructions.getByTestId("confirm-instruction-step").click();
  await expect(instructions.locator(".frameList")).toContainText("find an active advertisement to sell USDT");
  await expect(instructions.locator(".realAction")).toHaveCount(0);
  await instructions.locator(".guideToolbar").getByRole("button", { name: /Back to routes/ }).click();
  await expect(instructions).toBeHidden();
  await page.getByTestId("start-search").click();
  await expect(instructions).toBeVisible();
  await instructions.locator(".guideToolbar").getByRole("button", { name: /Back to routes/ }).click();

  let finishRefresh!: () => void;
  const heldRefresh = new Promise<void>((resolve) => { finishRefresh = resolve; });
  await page.route("**/api/p2p/routes**", async (route) => {
    await heldRefresh;
    await route.fallback();
  });
  await page.getByRole("button", { name: "Refresh routes now" }).click();
  await expect(page.getByTestId("start-search")).toHaveText("Go ↗");
  await expect(page.getByTestId("start-search")).toBeEnabled();
  await page.getByTestId("start-search").click();
  await expect(instructions).toBeVisible();
  finishRefresh();
  await expect(instructions).toBeVisible();
  await instructions.locator(".guideToolbar").getByRole("button", { name: /Back to routes/ }).click();
  await expect(page.getByTestId("start-search")).toBeEnabled();

  await swapDirection.click();
  await expect(amountInput).toHaveValue("20350");
});

test("editing the receive amount updates the send amount", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  const sendAmount = page.getByLabel("Amount to send");
  const receiveAmount = page.getByLabel("Amount to receive");
  await receiveAmount.fill("10000");
  await expect(sendAmount).toHaveValue("");
  await expect(sendAmount).toHaveValue("42269");
  await expect(receiveAmount).toHaveValue("10000");
  await expect(page.getByTestId("complete-route")).toHaveCount(1);

  const quotedSendAmount = await sendAmount.inputValue();
  await sendAmount.fill(quotedSendAmount);
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await expect(receiveAmount).toHaveValue("10000");
});

test("RUB to RUB bank routes require checking the order payment method", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByRole("option", { name: /^RUB\b/ }).click();
  await sourcePicker.getByLabel("Search banks and payment methods").fill("Sberbank");
  await sourcePicker.getByRole("option", { name: /Sberbank/ }).click();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByRole("option", { name: /^RUB\b/ }).click();
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();

  await page.getByLabel("Amount to send").fill("10000");
  await page.getByTestId("start-search").click();
  const route = page.getByTestId("complete-route").first();
  await expect(route).toBeVisible();
  await expect(route.locator(".workflowPayment")).toHaveCount(2);
  await expect(route.locator(".workflowPayment").first()).toHaveAttribute("title", "Sberbank");
  await expect(route.locator(".workflowPayment").last()).toHaveAttribute("title", "Alfa-Bank");
  await expect(route.locator(".workflowPayment img").first()).toHaveAttribute("src", "/icons/assets/sberbank.webp");
  await expect(route.locator(".workflowPayment img").last()).toHaveAttribute("src", "/icons/assets/alfabank.webp");
  await route.locator(".routeAmount").click();

  const instructions = page.getByTestId("route-guide");
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await instructions.getByTestId("start-guide").click();
  await expect(instructions.getByRole("heading", { name: "Buy USDT for 10,000 RUB" })).toBeVisible();
  await expect(instructions.locator(".frameList")).toContainText("find an active advertisement to buy USDT");
  await instructions.getByTestId("confirm-instruction-step").click();
  await expect(instructions.locator(".frameList")).toContainText("find an active advertisement to sell USDT");
});

test("AMD cycle keeps the best route visible when profit is unconfirmed", async ({ page }) => {
  await mockBackend(page);
  await page.addInitScript(() => localStorage.setItem("pay3flow.exchange.target-method", "am-idbank"));
  await openApp(page);

  await page.getByLabel("Amount to send").fill("10000");
  await page.getByTestId("start-search").click();

  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await expect(page.getByTestId("complete-route").first()).toContainText("-100 AMD (-1.00%)");
  const payments = page.getByTestId("complete-route").locator(".workflowPayment");
  await expect(payments).toHaveCount(2);
  await expect(payments.first()).toHaveAttribute("title", "Ameriabank");
  await expect(payments.last()).toHaveAttribute("title", "IDBank");
  await expect(page.getByTestId("complete-route").getByTestId("best-route-badge")).toHaveText("Best router");
  await expect(page.getByTestId("no-profitable-routes")).toHaveCount(0);
});

for (const source of ["binance", "bitcoin-center", "bestchange"] as const) {
test(`cross-venue instructions animate a contextual transfer from ${source}`, async ({ page }, testInfo) => {
  await mockBackend(page, { guideVenue: source });
  if (source === "bestchange") await page.emulateMedia({ reducedMotion: "reduce" });
  await openApp(page);

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByRole("option", { name: /^AMD / }).click();
  await sourcePicker.getByLabel("Search banks and payment methods").fill("IDBank");
  await sourcePicker.getByRole("option", { name: /IDBank/ }).click();
  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByRole("option", { name: /^RUB / }).click();
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();
  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(12);

  await page.getByTestId("complete-route").nth(2).locator(".routeAmount").click();
  const instructions = page.getByTestId("route-guide");
  await expectNumberedTimeline(instructions, ["1", "2", "3"]);
  await instructions.getByTestId("start-guide").click();
  await instructions.getByTestId("confirm-instruction-step").click();
  await expect(instructions.getByRole("heading", { name: "Transfer USDT to Bybit" })).toBeVisible();
  await expect(instructions.getByText("Choose the exact Ethereum (ERC-20) network on both platforms", { exact: false })).toBeVisible();
  await expect(instructions.locator(".frameList").getByText("Wait until Bybit shows the deposit as received before continuing.")).toBeVisible();
  const animation = instructions.getByTestId("transfer-animation");
  await expect(animation).toBeVisible();
  await expect(animation.getByTestId("transfer-source")).toContainText(({ binance: "Binance", "bitcoin-center": "Bitcoin Center", bestchange: "ChangerBiz" })[source]);
  await expect(animation.getByTestId("transfer-destination")).toContainText("Bybit");
  await expect(animation.getByTestId("transfer-source").locator("img")).toHaveAttribute("src", source === "bestchange" ? "/icons/venues/generic.svg" : source === "binance" ? "/icons/venues/binance.png" : "/icons/venues/bitcoin-center.svg");
  await expect(animation.getByTestId("transfer-destination").locator("img")).toHaveAttribute("src", "/icons/venues/bybit.png");
  await expect(animation).toContainText("Ethereum (ERC-20)");
  await expect(instructions.locator(".browser, [data-testid=spot-terminal]")).toHaveCount(0);
  await expect(instructions.getByTestId("p2p-profile-link")).toHaveCount(0);
  await instructions.getByRole("button", { name: "Scene 2: Check the network", exact: true }).click();
  await expect(animation).toHaveAttribute("data-phase", "verify");
  await instructions.getByRole("button", { name: "Scene 3: Confirm the transfer", exact: true }).click();
  await expect(animation).toHaveAttribute("data-phase", "act");
  const token = animation.getByTestId("transfer-token");
  await expect(token).toBeVisible();
  if (source === "binance") {
    await instructions.getByTestId("instruction-scene").scrollIntoViewIfNeeded();
    await instructions.getByTestId("playback-toggle").click();
    await expect.poll(() => token.evaluate(element => parseFloat(element.style.left))).toBeGreaterThan(8);
    await instructions.getByTestId("playback-toggle").click();
    const frozen = await token.getAttribute("style");
    await page.waitForTimeout(250);
    await expect(token).toHaveAttribute("style", frozen!);
  }
  await instructions.getByRole("button", { name: "Scene 4: Wait for the deposit", exact: true }).click();
  await expect(animation).toHaveAttribute("data-phase", "receive");
  await expect.poll(() => token.evaluate(element => element.style.left)).toBe("72%");
  await instructions.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath(`transfer-${source}.png`) });
  await instructions.getByTestId("confirm-instruction-step").click();
  await expect(instructions.getByTestId("transfer-animation")).toHaveCount(0);
  await expect(instructions.getByTestId("p2p-profile-link")).toHaveAttribute("href", "https://www.bybit.com/en/p2p/profile/masked-exit-3/USDT/RUB/item");
});
}

test("selected bank currencies override the reversed corridor", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Swap sender and recipient" }).click();
  await page.getByRole("button", { name: "Select sending bank: Sberbank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByLabel("Search banks and payment methods").fill("IDBank");
  await sourcePicker.getByRole("option", { name: /IDBank/ }).click();

  await page.getByRole("button", { name: "Select recipient bank: Ameriabank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();

  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("start-search").click();

  await expect(page.getByLabel("Amount to receive")).toBeVisible();
  await expect(page.locator(".moneyPanelTarget .currencyHint")).toHaveCount(0);
  await expect(page).toHaveURL(/#\/swap\/AMD\/RUB\?amount=100000$/);
});

test("catalog and direct quote providers are separately selectable", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Choose exchanges" }).click();
  await expect(page.getByRole("dialog", { name: "Exchange settings" })).toBeVisible();
  const whitebird = page.getByRole("button", { name: "Whitebird" });
  const cifra = page.getByRole("button", { name: "Cifra Markets" });
  const bestchange = page.getByRole("button", { name: "BestChange" });
  const dzengi = page.getByRole("button", { name: "Dzengi" });
  const cow = page.getByRole("button", { name: "CoW Protocol Live" });
  const near = page.getByRole("button", { name: "NEAR 1Click" });
  const idPay = page.getByRole("button", { name: "ID Pay Live" });

  await expect(cifra).toBeEnabled();
  await expect(cifra).toHaveAttribute("aria-pressed", "true");
  await expect(cifra.locator("img")).toHaveAttribute("src", "/icons/venues/cifra-broker.png");
  await expect(whitebird).toBeEnabled();
  await whitebird.click();
  await expect(whitebird).toHaveAttribute("aria-pressed", "true");
  for (const [button, icon] of [[bestchange, "/icons/venues/bestchange.svg"], [dzengi, "/icons/venues/dzengi.svg"]] as const) {
    await expect(button).toBeEnabled();
    await expect(button).toHaveAttribute("aria-pressed", "false");
    await expect(button.locator("img")).toHaveAttribute("src", icon);
    await button.click();
    await expect(button).toHaveAttribute("aria-pressed", "true");
  }
  await expect(cow).toBeEnabled();
  await expect(cow).toHaveAttribute("aria-pressed", "true");
  await expect(cow.locator("img")).toHaveAttribute("src", "/icons/venues/cow-swap-favicon.svg");
  await expect(near).toBeEnabled();
  await expect(near).toHaveAttribute("aria-pressed", "true");
  await expect(near.locator("img")).toHaveAttribute("src", "/icons/assets/near.webp");
  await expect(idPay).toBeEnabled();
  await expect(idPay).toHaveAttribute("aria-pressed", "true");
  await expect(idPay.locator("img")).toHaveAttribute("src", "/icons/venues/id-pay.svg");
});

test("exchange methods allow one or both choices and persist the selection", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Route refresh settings" }).click();
  let settings = page.getByRole("dialog", { name: "Refresh settings" });
  const methods = settings.getByLabel("Exchange methods");
  const p2p = methods.getByRole("button", { name: "P2P" });
  const exchangers = methods.getByRole("button", { name: "Exchangers" });
  await expect(p2p).toHaveAttribute("aria-pressed", "true");
  await expect(exchangers).toHaveAttribute("aria-pressed", "true");

  await p2p.click();
  await expect(p2p).toHaveAttribute("aria-pressed", "false");
  await exchangers.click();
  await expect(exchangers).toHaveAttribute("aria-pressed", "true");
  await expect.poll(() => page.evaluate(() => localStorage.getItem("pay3flow.exchange.methods"))).toBe("exchanger");

  await page.keyboard.press("Escape");
  await page.reload();
  await page.getByRole("button", { name: "Route refresh settings" }).click();
  settings = page.getByRole("dialog", { name: "Refresh settings" });
  await expect(settings.getByRole("button", { name: "P2P" })).toHaveAttribute("aria-pressed", "false");
  await expect(settings.getByRole("button", { name: "Exchangers" })).toHaveAttribute("aria-pressed", "true");

  await settings.getByRole("button", { name: "P2P" }).click();
  await settings.getByRole("button", { name: "Exchangers" }).click();
  await expect.poll(() => page.evaluate(() => localStorage.getItem("pay3flow.exchange.methods"))).toBe("p2p");

  await page.keyboard.press("Escape");
  const filteredRequest = page.waitForRequest((request) => {
    const url = new URL(request.url());
    return url.pathname === "/api/p2p/routes" && url.searchParams.get("exchange_mode") === "p2p";
  });
  await page.getByLabel("Amount to send").fill("100000");
  await filteredRequest;
});

test("saved provider choices adopt new providers and retain later deselections", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow.exchange.p2p-sources", "cow-swap");
  });
  await mockBackend(page, { includeNewProviders: true });
  await openApp(page);

  await page.getByRole("button", { name: "Choose exchanges" }).click();
  const picker = page.getByRole("dialog", { name: "Exchange settings" });
  for (const name of ["Bitcoin Center", "bncex", "SkyLabs", "Symbiosis"]) {
    await expect(picker.getByRole("button", { name })).toHaveAttribute("aria-pressed", "true");
  }
  const symbiosis = picker.getByRole("button", { name: "Symbiosis" });
  await expect(symbiosis.locator("img")).toHaveAttribute("src", "/icons/venues/symbiosis.png");
  await symbiosis.click();
  await expect(symbiosis).toHaveAttribute("aria-pressed", "false");

  const providersReloaded = page.waitForResponse((response) =>
    response.url().endsWith("/api/providers") && response.ok(),
  );
  await page.reload();
  await providersReloaded;
  await page.getByRole("button", { name: "Choose exchanges" }).click();
  await expect(page.getByRole("dialog", { name: "Exchange settings" }).getByRole("button", { name: "Symbiosis" }))
    .toHaveAttribute("aria-pressed", "false");
});

test("search venues only show providers with matching routes", async ({ page }) => {
  await mockBackend(page);

  let sendFirstRoute: (() => void) | undefined;
  let sendSecondRoute: (() => void) | undefined;
  let finishSearch: (() => void) | undefined;
  let socketConnections = 0;
  await page.routeWebSocket(/\/ws\/p2p\/routes(?:\?|$)/, (socket) => {
    socketConnections += 1;
    socket.onMessage((message) => {
      const request = JSON.parse(String(message));
      expect(request.query.sources).toBe("binance,bybit,cifra-broker,cow-swap,id-pay,near-intents,whitebird");
      expect(request.query.exchange_mode).toBe("all");

      const offer = (source: string, adId: string, fiat: string) => ({
        source,
        ad_id: adId,
        fiat,
        asset: "USDT",
        network: source === "bitcoin-center" ? "solana" : source === "bncex" ? "tron" : null,
        price: "1",
        available_asset: "1000000",
        min_fiat: "1000",
        max_fiat: "10000000",
        payment_methods: ["Bank transfer"],
        pay_time_limit_minutes: 15,
        advertiser: {
          id: `masked-${adId}`,
          nickname: source === "bestchange" ? "ChangerBiz" : `${source}-merchant`,
          user_type: "merchant",
          is_merchant: true,
          is_verified: true,
          completed_orders_30d: 300,
          completion_rate_30d: 0.99,
        },
        source_url: `https://example.com/${adId}`,
      });
      const firstResponse = {
        search_id: "00000000-0000-4000-8000-000000000106",
        routes_found: 1,
        searched_at: "2026-09-23T10:00:00Z",
        source_fiat: "AMD",
        target_fiat: "RUB",
        source_amount: "100000.00",
        assets_searched: ["USDT"],
        can_exchange_to_target: true,
        asset_statuses: [{
          asset: "USDT",
          entry_offers: 4,
          exit_offers: 2,
          routes_built: 1,
          can_exchange_to_target: true,
          entry_sources: [
            { source: "bybit", ok: true, latency_ms: 10, offers_found: 1, error: null },
            { source: "skylabs", ok: true, latency_ms: 15, offers_found: 2, error: null },
            { source: "bncex", ok: false, latency_ms: 18, offers_found: 1, error: "Provider temporarily unavailable" },
            { source: "bitcoin-center", ok: true, latency_ms: 19, offers_found: 1, error: null },
            { source: "binance", ok: true, latency_ms: 20, offers_found: 0, error: null },
          ],
          exit_sources: [{ source: "okx", ok: true, latency_ms: 12, offers_found: 0, error: null }],
        }],
        routes: [{
          route_id: "route-streamed-first",
          rank: 1,
          asset: "USDT",
          entry_network: "internal",
          source_fiat: "AMD",
          source_amount: "100000.00",
          acquired_asset_amount: "253.16",
          target_fiat: "RUB",
          target_amount: "20350.00",
          effective_rate: "0.2035",
          same_venue: true,
          requires_asset_transfer: false,
          transfer_fee_included: true,
          route_kind: "fiat_to_fiat",
          payment_methods_verified: true,
          entry_offer: offer("bybit", "entry-live", "AMD"),
          exit_offer: offer("bybit", "exit-live", "RUB"),
          warnings: [],
        }],
      };
      const finalResponse = {
        ...firstResponse,
        routes_found: 2,
        routes: [{
          ...firstResponse.routes[0],
          route_id: "route-streamed-second",
          rank: 1,
          target_amount: "20420.00",
          effective_rate: "0.2042",
          entry_offer: offer("whitebird", "entry-live-2", "AMD"),
          exit_offer: offer("whitebird", "exit-live-2", "RUB"),
        }, { ...firstResponse.routes[0], rank: 2 }],
      };

      socket.send(JSON.stringify({ type: "search_started", search_id: firstResponse.search_id, routes_found: 0 }));
      sendFirstRoute = () => socket.send(JSON.stringify({ type: "routes_updated", ...firstResponse }));
      sendSecondRoute = () => socket.send(JSON.stringify({ type: "routes_updated", ...finalResponse }));
      finishSearch = () => socket.send(JSON.stringify({ type: "search_finished", ...finalResponse }));
    });
  });

  await openApp(page);
  const exchangesButton = page.getByRole("button", { name: "Choose exchanges" });
  await exchangesButton.click();
  await page.getByRole("button", { name: "Whitebird" }).click();
  if ((page.viewportSize()?.width ?? 0) <= 640) {
    await page.locator(".settingsBackdrop").dispatchEvent("mousedown");
  } else {
    await exchangesButton.click();
  }
  await expect(page.getByRole("dialog", { name: "Exchange settings" })).toHaveCount(0);
  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("start-search").click();

  await expect.poll(() => Boolean(sendFirstRoute)).toBe(true);
  await page.waitForTimeout(750);
  expect(socketConnections).toBe(1);
  const panelTop = page.locator("#routes .panelTop");
  const searchingVenues = panelTop.getByTestId("searching-venue");
  await expect(searchingVenues).toHaveCount(5);
  await expect(searchingVenues.nth(0)).toHaveAttribute("title", "Searching Binance");
  await expect(searchingVenues.nth(1)).toHaveAttribute("title", "Searching Bybit");
  await expect(searchingVenues.nth(2)).toHaveAttribute("title", "Searching Cifra Markets");
  await expect(searchingVenues.nth(3)).toHaveAttribute("title", "Searching CoW Protocol Live");
  await expect(searchingVenues.nth(4)).toHaveAttribute("title", "Searching ID Pay Live");
  await expect(panelTop.getByTestId("searching-venues-overflow")).toHaveText("...");
  await expect(searchingVenues.nth(0)).toHaveCSS("width", "32px");
  await expect(searchingVenues.nth(0)).toHaveCSS("animation-delay", "0s");
  await expect(searchingVenues.nth(1)).toHaveCSS("animation-delay", "0.13s");

  const refreshButton = page.getByRole("button", { name: "Refresh routes now" });
  await expect(refreshButton.locator("img")).toHaveClass(/refreshSpin/);
  await expect(page.getByTestId("start-search")).toBeDisabled();
  sendFirstRoute?.();
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await expect(page.getByTestId("start-search")).toBeEnabled();
  await expect(page.getByTestId("complete-route").first()).toHaveClass(/selected/);
  const foundVenues = panelTop.locator(".resultSummary").getByTestId("found-venue");
  await expect(foundVenues).toHaveCount(1);
  await expect(foundVenues.first()).toHaveAttribute("title", "Found on Bybit");
  await expect(panelTop.locator(".resultSummary")).not.toContainText("venue in routes");
  await expect(searchingVenues).toHaveCount(5);
  await expect(searchingVenues.nth(0)).toHaveAttribute("title", "Searching Cifra Markets");
  await expect(searchingVenues.nth(4)).toHaveAttribute("title", "Searching Whitebird");
  await expect(panelTop.getByTestId("searching-venues-overflow")).toHaveCount(0);
  await expect(refreshButton).toBeDisabled();
  await expect(refreshButton.locator("img")).not.toHaveClass(/refreshSpin/);

  sendSecondRoute?.();
  await expect(page.getByTestId("complete-route")).toHaveCount(2);
  await expect(page.getByTestId("complete-route").first()).toContainText("20420 RUB");
  await expect(page.getByTestId("complete-route").first()).toHaveClass(/selected/);
  await expect(foundVenues).toHaveCount(2);
  for (const title of ["Whitebird", "Bybit"]) {
    await expect(panelTop.locator(`[data-testid="found-venue"][title="Found on ${title}"]`)).toHaveCount(1);
  }
  await page.getByRole("button", { name: "Show Whitebird routes" }).click();
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await expect(page.getByTestId("complete-route").first()).toContainText("Whitebird");
  await page.getByRole("button", { name: "Show Whitebird routes" }).click();
  await expect(page.getByTestId("complete-route")).toHaveCount(2);
  await expect(searchingVenues).toHaveCount(4);

  await page.getByTestId("complete-route").nth(1).locator(".routeRank").click();
  await expect(page.getByTestId("complete-route").nth(1)).toHaveClass(/selected/);
  await expect(refreshButton).toBeDisabled();
  await page.getByTestId("start-search").click();
  const guide = page.getByTestId("route-guide");
  await expect(guide).toBeVisible();
  await guide.getByTestId("start-guide").click();
  await expect(guide.locator(".headingVenue")).toContainText("Bybit");
  finishSearch?.();
  await expect(guide.locator(".headingVenue")).toContainText("Bybit");
  await guide.locator(".guideToolbar").getByRole("button", { name: /Back to routes/ }).click();
  await expect(refreshButton).toBeEnabled();
  await expect(searchingVenues).toHaveCount(0);
  await expect(page.getByTestId("complete-route").nth(1)).toHaveClass(/selected/);
});

test("reordered progressive snapshots do not restart card rendering at 100", async ({ page }) => {
  await mockBackend(page);

  let sendFirstSnapshot: (() => void) | undefined;
  let sendReorderedSnapshot: (() => void) | undefined;
  let finishSearch: (() => void) | undefined;
  await page.routeWebSocket(/\/ws\/p2p\/routes(?:\?|$)/, (socket) => {
    socket.onMessage(() => {
      const offer = (adId: string) => ({
        source: "bybit",
        ad_id: adId,
        fiat: "AMD",
        asset: "USDT",
        network: source === "bitcoin-center" ? "solana" : source === "bncex" ? "tron" : null,
        price: "1",
        available_asset: "1000000",
        min_fiat: "1000",
        max_fiat: "10000000",
        payment_methods: ["Bank transfer"],
        pay_time_limit_minutes: 15,
        advertiser: {
          id: `masked-${adId}`,
          nickname: "bybit-merchant",
          user_type: "merchant",
          is_merchant: true,
          is_verified: true,
          completed_orders_30d: 300,
          completion_rate_30d: 0.99,
        },
        source_url: `https://example.com/${adId}`,
      });
      const route = (index: number) => ({
        route_id: `route-progressive-${index}`,
        rank: index + 1,
        asset: "USDT",
        entry_network: "internal",
        source_fiat: "AMD",
        source_amount: "100000.00",
        acquired_asset_amount: "253.16",
        target_fiat: "RUB",
        target_amount: String(20_500 - index),
        effective_rate: "0.205",
        same_venue: true,
        requires_asset_transfer: false,
        transfer_fee_included: true,
        route_kind: "fiat_to_fiat",
        payment_methods_verified: true,
        entry_offer: offer(`entry-${index}`),
        exit_offer: { ...offer(`exit-${index}`), fiat: "RUB" },
        warnings: [],
      });
      const firstRoutes = Array.from({ length: 101 }, (_, index) => route(index));
      const reorderedRoutes = [
        ...firstRoutes.slice().reverse(),
        ...Array.from({ length: 100 }, (_, index) => route(index + firstRoutes.length)),
      ];
      const response = (routes: ReturnType<typeof route>[], routesFound: number) => ({
        search_id: "00000000-0000-4000-8000-000000000108",
        routes_found: routesFound,
        searched_at: "2026-09-26T10:00:00Z",
        source_fiat: "AMD",
        target_fiat: "RUB",
        source_amount: "100000.00",
        assets_searched: ["USDT"],
        can_exchange_to_target: true,
        routes,
      });
      const firstResponse = response(firstRoutes, 250);
      const reorderedResponse = response(reorderedRoutes, 201);

      socket.send(JSON.stringify({ type: "search_started", search_id: firstResponse.search_id, routes_found: 0 }));
      sendFirstSnapshot = () => socket.send(JSON.stringify({ type: "routes_updated", ...firstResponse }));
      sendReorderedSnapshot = () => socket.send(JSON.stringify({ type: "routes_updated", ...reorderedResponse }));
      finishSearch = () => socket.send(JSON.stringify({ type: "search_finished", ...reorderedResponse }));
    });
  });

  await openApp(page);
  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("start-search").click();
  await expect.poll(() => Boolean(sendFirstSnapshot)).toBe(true);

  sendFirstSnapshot?.();
  await expect(page.getByTestId("complete-route")).toHaveCount(101);
  await expect(page.getByText("250 routes found")).toBeVisible();
  await page.evaluate(() => {
    const browserWindow = window as Window & { __routeCountSamples?: number[] };
    browserWindow.__routeCountSamples = [];
    new MutationObserver(() => {
      browserWindow.__routeCountSamples?.push(document.querySelectorAll('[data-testid="complete-route"]').length);
    }).observe(document.querySelector("#routes")!, { childList: true, subtree: true });
  });

  sendReorderedSnapshot?.();
  await expect(page.getByTestId("complete-route")).toHaveCount(201);
  await expect(page.getByText("250 routes found")).toBeVisible();
  const counts = await page.evaluate(() =>
    (window as Window & { __routeCountSamples?: number[] }).__routeCountSamples ?? [],
  );
  expect(counts.length).toBeGreaterThan(0);
  expect(Math.min(...counts)).toBeGreaterThanOrEqual(101);

  finishSearch?.();
});

test("currency control only lists currencies supported by the selected payment method", async ({ page }) => {
  await mockBackend(page);
  let marketPriceRequests = 0;
  page.on("request", (request) => {
    if (new URL(request.url()).pathname === "/api/market-prices") marketPriceRequests += 1;
  });
  await openApp(page);

  await expect(page.getByRole("button", { name: "Select sending currency: AMD" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Select recipient currency: RUB" })).toBeVisible();

  await page.getByRole("button", { name: "Select sending currency: AMD" }).click();
  const currencyPicker = page.getByRole("dialog", { name: "Choose currency" });
  await expect(currencyPicker.getByRole("option")).toHaveCount(2);
  await expect(currencyPicker.getByRole("option", { name: /^AMD\b/ })).toBeVisible();
  await expect(currencyPicker.getByRole("option", { name: /^USD\b/ })).toBeVisible();
  await expect(currencyPicker.getByRole("option", { name: /^RUB\b/ })).toHaveCount(0);
  await expect(currencyPicker.getByRole("option", { name: /^USDT\b/ })).toHaveCount(0);
  await currencyPicker.getByRole("option", { name: /^USD\b/ }).click();
  await expect(page.getByRole("button", { name: "Select sending currency: USD" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Select sending bank: Ameriabank" })).toBeVisible();

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const methodPicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await methodPicker.getByRole("option", { name: /^USD\b/ }).click();
  await methodPicker.getByLabel("Search banks and payment methods").fill("Ameriabank");
  await expect(methodPicker.getByRole("option", { name: /Ameriabank/ })).toHaveCount(1);
  await methodPicker.getByLabel("Search banks and payment methods").fill("IDBank");
  await expect(methodPicker.getByRole("option", { name: /^IDBank Bank transfer · USD/ })).toHaveCount(1);
  await methodPicker.getByRole("option", { name: /^IDBank Bank transfer · USD/ }).click();
  await expect(page.getByRole("button", { name: "Select sending bank: IDBank" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Select sending currency: USD" })).toBeVisible();
  await expect(page.locator(".moneyPanelSource .methodControls > .methodTrigger + .networkControl")).toHaveCount(1);
  await expect(page.locator(".moneyPanelSource .networkButton .networkCopy")).toHaveText("USD");

  await page.getByRole("button", { name: "Select sending bank: IDBank" }).click();
  const reopenedMethodPicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await reopenedMethodPicker.getByRole("option", { name: /^USDT\b/ }).click();
  await reopenedMethodPicker.getByRole("textbox", { name: "Blockchains" }).fill("ERC20");
  await expect(reopenedMethodPicker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDT/ })).toHaveCount(1);
  await reopenedMethodPicker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDT/ }).click();
  const sendingAsset = page.getByRole("button", { name: "Select sending asset: USDT" });
  await expect(sendingAsset).toBeVisible();
  await expect(sendingAsset.locator(".methodText")).toHaveText("USDT");
  await page.getByLabel("Amount to send").fill("2");
  await expect(page.locator(".moneyPanelSource .marketValue")).toHaveText("$2.00");
  await page.getByLabel("Amount to send").fill("3");
  await expect(page.locator(".moneyPanelSource .marketValue")).toHaveText("$3.00");
  expect(marketPriceRequests).toBe(1);
  await page.getByLabel("Amount to send").fill("0");
  await expect(page.locator(".moneyPanelSource .marketValue")).toHaveCount(0);
  await expect(page.locator(".moneyPanelSource .currencyHint")).toHaveCount(0);
  const sendingNetwork = page.getByRole("button", { name: "Select sending network: Ethereum (ERC-20)" });
  await expect(sendingNetwork).toBeVisible();
  await expect(sendingNetwork.locator(".networkCopy")).toHaveCount(0);
  await sendingNetwork.click();
  const networkPicker = page.getByRole("dialog", { name: "Choose network" });
  await expect(networkPicker.locator(".optionMeta")).toHaveCount(0);
  await networkPicker.getByRole("option", { name: "Ethereum (ERC-20)" }).click();
  for (const language of ["ru", "hy", "en"] as const) {
    await selectLanguage(page, language);
    await expect(page.locator(".moneyPanelSource .methodTrigger .methodText")).toHaveText("USDT");
  }

  await page.getByRole("button", { name: "Select recipient currency: RUB" }).click();
  const recipientCurrencies = page.getByRole("dialog", { name: "Choose currency" });
  await expect(recipientCurrencies.getByRole("option")).toHaveCount(1);
  await expect(recipientCurrencies.getByRole("option", { name: /^RUB\b/ })).toBeVisible();
  await expect(recipientCurrencies.getByRole("option", { name: /^(AMD|USD)\b/ })).toHaveCount(0);
  await recipientCurrencies.getByRole("option", { name: /^RUB\b/ }).click();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const recipientMethods = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await recipientMethods.getByRole("option", { name: /^AMD\b/ }).click();
  await recipientMethods.getByLabel("Search banks and payment methods").fill("Ameriabank");
  await recipientMethods.getByRole("option", { name: /^Ameriabank Bank transfer · AMD/ }).click();
  await page.getByRole("button", { name: "Select recipient currency: AMD" }).click();
  const ameriaCurrencies = page.getByRole("dialog", { name: "Choose currency" });
  await expect(ameriaCurrencies.getByRole("option", { name: /^AMD\b/ })).toBeVisible();
  await expect(ameriaCurrencies.getByRole("option", { name: /^USD\b/ })).toBeVisible();
  await expect(ameriaCurrencies.getByRole("option", { name: /^RUB\b/ })).toHaveCount(0);
});

test("cash is available in each country currency without replacing default banks", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await expect(page.getByRole("button", { name: "Select sending bank: Ameriabank" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Select recipient bank: Sberbank" })).toBeVisible();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetMethodPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetMethodPicker.getByRole("option", { name: /^RUB\b/ }).click();
  await expect(targetMethodPicker.getByRole("option", { name: /^Cash RUB Cash settlement · RUB/ })).toHaveCount(1);
  await targetMethodPicker.getByRole("option", { name: /Cash RUB/ }).click();
  await expect(page.getByRole("button", { name: "Select recipient payment method: Cash RUB" })).toBeVisible();

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourceMethodPicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourceMethodPicker.getByRole("option", { name: /^AMD\b/ }).click();
  await sourceMethodPicker.getByRole("option", { name: /Cash AMD/ }).click();
  await expect(page.getByRole("button", { name: "Select sending payment method: Cash AMD" })).toBeVisible();

  await page.getByRole("button", { name: "Select sending currency: AMD" }).click();
  const currencies = page.getByRole("dialog", { name: "Choose currency" });
  for (const code of ["AMD", "BYN", "RUB", "USD"]) await expect(currencies.getByRole("option", { name: new RegExp(`^${code}\\b`) })).toBeVisible();
  await currencies.getByRole("option", { name: /^BYN\b/ }).click();
  await expect(page.getByRole("button", { name: "Select sending payment method: Cash BYN" })).toBeVisible();
});

test("USD supports cash and Armenian bank currencies", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending currency: AMD" }).click();
  const sourceCurrencyPicker = page.getByRole("dialog", { name: "Choose currency" });
  await expect(sourceCurrencyPicker.getByRole("option", { name: /^USD\b/ })).toBeVisible();
  await sourceCurrencyPicker.getByRole("option", { name: /^USD\b/ }).click();

  const ameria = page.getByRole("button", { name: "Select sending bank: Ameriabank" });
  await expect(ameria).toBeVisible();
  await ameria.click();
  const methodPicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await methodPicker.getByRole("option", { name: /^USD\b/ }).click();
  await expect(methodPicker.getByRole("option", { name: /Cash USD/ })).toBeVisible();
  await methodPicker.getByLabel("Search banks and payment methods").fill("Ameriabank");
  await expect(methodPicker.getByRole("option", { name: /^Ameriabank Bank transfer · USD/ })).toHaveCount(1);
  await methodPicker.getByLabel("Search banks and payment methods").fill("");
  await methodPicker.getByRole("option", { name: /Cash USD/ }).click();
  await expect(page.getByRole("button", { name: "Select sending payment method: Cash USD" })).toBeVisible();
  await expect(page.locator(".moneyPanelSource .currencyHint")).toHaveCount(0);

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetMethodPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetMethodPicker.getByRole("option", { name: /^AMD\b/ }).click();
  await targetMethodPicker.getByLabel("Search banks and payment methods").fill("Ameriabank");
  await targetMethodPicker.getByRole("option", { name: /^Ameriabank Bank transfer · AMD/ }).click();
  await expect(page.getByRole("button", { name: "Select recipient bank: Ameriabank" })).toBeVisible();

  await page.getByLabel("Amount to send").fill("12000");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await expect(page.getByText("4620000 AMD")).toBeVisible();
  await expect(page).toHaveURL(/#\/swap\/USD\/AMD\?amount=12000$/);
});

test("T-Bank USD routes through crypto to Ameriabank USD", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending currency: AMD" }).click();
  await page.getByRole("dialog", { name: "Choose currency" }).getByRole("option", { name: /^USD\b/ }).click();

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourceMethodPicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourceMethodPicker.getByLabel("Search banks and payment methods").fill("T-Bank");
  await sourceMethodPicker.getByRole("option", { name: /^T-Bank Bank transfer · USD/ }).click();
  await expect(page.getByRole("button", { name: "Select sending bank: T-Bank" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Select sending currency: USD" })).toBeVisible();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetMethodPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetMethodPicker.getByLabel("Search banks and payment methods").fill("Ameriabank");
  await targetMethodPicker.getByRole("option", { name: /^Ameriabank Bank transfer · AMD/ }).click();
  await page.getByRole("button", { name: "Select recipient currency: AMD" }).click();
  await page.getByRole("dialog", { name: "Choose currency" }).getByRole("option", { name: /^USD\b/ }).click();
  await expect(page.getByRole("button", { name: "Select recipient bank: Ameriabank" })).toBeVisible();

  await page.getByLabel("Amount to send").fill("12000");
  await page.getByTestId("start-search").click();

  const route = page.getByTestId("complete-route");
  await expect(route).toHaveCount(1);
  await expect(route).toContainText("USDT");
  await expect(page.getByText("11643.56 USD")).toBeVisible();
  await expect(page).toHaveURL(/#\/swap\/USD\/USD\?amount=12000$/);
});

test("Ameriabank USD routes through crypto to T-Bank USD", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending currency: AMD" }).click();
  await page.getByRole("dialog", { name: "Choose currency" }).getByRole("option", { name: /^USD\b/ }).click();

  await expect(page.getByRole("button", { name: "Select sending bank: Ameriabank" })).toBeVisible();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetMethodPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetMethodPicker.getByLabel("Search banks and payment methods").fill("T-Bank");
  await targetMethodPicker.getByRole("option", { name: /^T-Bank Bank transfer · RUB/ }).click();
  await page.getByRole("button", { name: "Select recipient currency: RUB" }).click();
  await page.getByRole("dialog", { name: "Choose currency" }).getByRole("option", { name: /^USD\b/ }).click();
  await expect(page.getByRole("button", { name: "Select recipient bank: T-Bank" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Select recipient currency: USD" })).toBeVisible();

  await page.getByLabel("Amount to send").fill("12000");
  await page.getByTestId("start-search").click();

  const route = page.getByTestId("complete-route");
  await expect(route).toHaveCount(1);
  await expect(route).toContainText("USDT");
  await expect(route).toContainText(/okx/i);
  await expect(route).toContainText(/skylabs/i);
  await expect(page.getByText("11643.56 USD")).toBeVisible();
  await expect(page).toHaveURL(/#\/swap\/USD\/USD\?amount=12000$/);
});

test("cryptocurrency search binds the selected asset to its network", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  const picker = await chooseCrypto(page, "sending", "USDT ERC20");

  const ethereumUsdt = picker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDT/ });
  await expect(ethereumUsdt).toHaveCount(1);
  await ethereumUsdt.click();

  const selectedNetwork = page.getByRole("button", { name: /Select sending network: Ethereum \(ERC-20\)/ });
  await expect(selectedNetwork).toHaveAttribute("title", "Ethereum (ERC-20)");

  await page.getByLabel("Amount to send").fill("125");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await expect(page.getByTestId("complete-route").locator(".routeWorkflowButton")).toHaveAttribute(
    "aria-label",
    /USDT · ERC 20 \(Binance\) → RUB/,
  );
});

test("direct provider quotes keep their API names and independent prices", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  const sourcePicker = await chooseCrypto(page, "sending", "USDT ERC20");
  await sourcePicker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDT/ }).click();

  const targetPicker = await chooseCrypto(page, "recipient", "USDC ERC20");
  await targetPicker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDC/ }).click();

  await page.getByLabel("Amount to send").fill("100");
  await page.getByTestId("start-search").click();

  const cards = page.getByTestId("complete-route");
  await expect(cards).toHaveCount(2);
  await expect(cards.nth(0)).toContainText("99.6 USDC");
  await expect(cards.nth(0)).not.toContainText("Quote by");
  await expect(cards.nth(1)).toContainText("97.3 USDC");
  await expect(cards.nth(1)).not.toContainText("Quote by");
  await expect(cards.nth(0).locator(".routeWorkflowButton")).toHaveAttribute(
    "aria-label", /USDT.*ERC-20.*USDC.*ERC-20.*NEAR 1Click/,
  );
  await expect(cards.nth(1).locator(".routeWorkflowButton")).toHaveAttribute(
    "aria-label", /USDT.*ERC-20.*USDC.*ERC-20.*CoW Protocol Live/,
  );
  await cards.nth(0).locator(".routeAmount").click();
  let instructions = page.getByTestId("route-guide");
  await instructions.getByTestId("start-guide").click();
  await expect(instructions.getByTestId("route-wallet-execution")).toBeVisible();
  await expect(instructions.getByRole("button", { name: "Connect ethereum wallet" })).toBeVisible();
  await instructions.locator(".guideToolbar").getByRole("button", { name: /Back to routes/ }).click();
  await cards.nth(1).locator(".routeAmount").click();
  instructions = page.getByTestId("route-guide");
  await instructions.getByTestId("start-guide").click();
  await expect(instructions.getByTestId("route-wallet-execution")).toBeVisible();
});

test("ID Pay provides a direct AMD to RUB route with its API name", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByLabel("Amount to send").fill("42269");
  await expect(page.getByTestId("complete-route")).toBeVisible();

  const route = page.getByTestId("complete-route");
  await expect(route).toHaveCount(1);
  await expect(route).toContainText(/10,?000 RUB/);
  await expect(route).not.toContainText("Quote by");
  await expect(route.locator(".routeWorkflowButton")).toHaveAttribute(
    "aria-label",
    "AMD · Ameriabank → RUB · Sberbank (ID Pay Live)",
  );

  await route.locator(".routeAmount").click();
  const instructions = page.getByTestId("route-guide");
  await instructions.getByTestId("start-guide").click();
  await expect(instructions.getByRole("heading", { name: "Transfer AMD to RUB via ID Pay Live" })).toBeVisible();
  await expect(instructions.locator(".realAction")).toHaveCount(0);
});

test("small AMD to BTC@near routes keep crypto precision", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  const targetPicker = await chooseCrypto(page, "recipient", "BTC NEAR");
  await targetPicker.getByRole("option", { name: /NEAR.*BTC/ }).click();

  await page.getByLabel("Amount to send").fill("10000");
  await page.getByTestId("start-search").click();

  const route = page.getByTestId("complete-route").first();
  await expect(route).toBeVisible();
  await expect(route.locator(".routeAmount")).toHaveText("0.00032478 BTC");
  await expect(page.locator("#exchange-output")).toHaveValue("0.00032478");
  await expect(route.locator(".routeWorkflowButton")).toHaveAttribute(
    "aria-label", /AMD.*USDT.*Bybit.*BTC.*NEAR/,
  );
  await route.locator(".routeAmount").click();
  const instructions = page.getByTestId("route-guide");
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await instructions.getByTestId("start-guide").click();
  await expect(instructions.getByRole("heading", { name: "Buy USDT for 10,000 AMD" })).toBeVisible();
  await instructions.getByTestId("confirm-instruction-step").click();
  await expect(instructions.getByRole("heading", { name: "Swap USDT for BTC via NEAR 1Click" })).toBeVisible();
  await expect(instructions.getByRole("button", { name: "Connect optimism wallet" })).toBeVisible();
});

test("direct Whitebird exchange uses its swap card, provider wording and local venue icons", async ({ page }, testInfo) => {
  await mockBackend(page);
  await openApp(page);

  const picker = await chooseCrypto(page, "sending", "USDC ERC20");
  await picker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDC/ }).click();

  await page.getByLabel("Amount to send").fill("100");
  await page.getByTestId("start-search").click();
  const route = page.getByTestId("complete-route");
  await expect(route).toHaveCount(1);
  await expect(route.locator(".routeWorkflowButton")).toHaveAttribute(
    "aria-label",
    "USDC USD Coin · ERC 20 (Whitebird) → RUB · Sberbank",
  );
  await expect(route.locator(".workflowVenueIcon img")).toHaveAttribute(
    "src",
    "/icons/venues/whitebird.png",
  );

  await route.locator(".routeAmount").click();
  const instructions = page.getByTestId("route-guide");
  await instructions.getByTestId("start-guide").click();
  await expect(instructions.locator(".realAction, .adHint, .counterpartyAvatar")).toHaveCount(0);
  const scene = instructions.getByTestId("instruction-scene");
  const swap = scene.getByTestId("whitebird-swap-card");
  await expect(swap).toBeVisible();
  await expect(scene.locator(".browserChrome, .platform, .offerIdentity")).toHaveCount(0);
  await expect(swap.getByTestId("whitebird-send-amount")).toHaveText("100");
  await expect(swap.getByTestId("whitebird-receive-amount")).toHaveText("8,342.95");
  await expect(swap.locator('[data-side="send"] .currencyText')).toContainText("USDC");
  await expect(swap.locator('[data-side="receive"] .currencyText')).toContainText("RUB");
  await expect(swap.locator(".exchangeAction")).toHaveText("Exchange");
  await expect(swap.locator(".fees")).toContainText("Check on Whitebird");
  await scene.scrollIntoViewIfNeeded();
  await instructions.getByRole("button", { name: "Scene 2: Check the exchange", exact: true }).click();
  await expect(swap).toHaveAttribute("data-frame", "1");
  await expect(swap.locator(".fieldControl.highlight")).toHaveCount(2);
  await expect(swap).toHaveClass(/paused/);
  const dimensions = await swap.evaluate(element => ({ width: element.clientWidth, contentWidth: element.scrollWidth }));
  expect(dimensions.contentWidth).toBeLessThanOrEqual(dimensions.width + 2);
  await page.mouse.move(0, 0);
  await scene.screenshot({ animations: "disabled", path: testInfo.outputPath("whitebird-swap.png") });
  await instructions.getByRole("button", { name: "Next scene", exact: true }).click();
  await expect(swap).toHaveAttribute("data-frame", "2");
  await expect(swap).toHaveAttribute("data-frame-kind", "review");
  await expect(instructions.getByRole("button", { name: "Scene 3: Check the amount you receive", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(scene.locator(".sceneAnnotation strong")).toHaveText("Check the amount you receive");
  await expect(swap.locator('[data-side="receive"] .fieldControl')).toHaveClass(/highlight/);
  await expect(swap.locator('[data-side="send"] .fieldControl')).not.toHaveClass(/highlight/);
  await expect(swap.locator(".exchangeAction")).not.toHaveClass(/actionHighlight/);
  await instructions.getByRole("button", { name: "Next scene", exact: true }).click();
  await expect(swap).toHaveAttribute("data-frame", "3");
  await expect(swap).toHaveAttribute("data-frame-kind", "act");
  await expect(instructions.getByRole("button", { name: "Scene 4: Press Exchange", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(scene.locator(".sceneAnnotation strong")).toHaveText("Press Exchange");
  await expect(swap.locator(".exchangeAction")).toHaveClass(/actionHighlight/);
  await expect(swap.locator(".fieldControl.highlight")).toHaveCount(0);
  await instructions.getByRole("button", { name: "Previous scene", exact: true }).click();
  await expect(swap).toHaveAttribute("data-frame-kind", "review");
  await expect(swap.locator(".exchangeAction")).not.toHaveClass(/actionHighlight/);
});

test("Bybit P2P walkthrough reviews the profile and selects the route's buy or sell advertisement", async ({ page }, testInfo) => {
  await mockBackend(page);
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
    localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
  });
  await openApp(page);
  await page.getByLabel("Amount to send").fill("100000");
  await expect(page.getByTestId("complete-route")).toHaveCount(12);
  await page.getByTestId("complete-route").nth(1).locator(".routeAmount").click();
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const scene = guide.getByTestId("instruction-scene");
  const profile = scene.getByTestId("bybit-profile-card");
  await scene.scrollIntoViewIfNeeded();
  await expect(profile).toBeVisible();
  await expect(profile.getByTestId("bybit-advertiser")).toHaveText("bybit-merchant");
  await expect(guide.getByTestId("p2p-profile-link")).toHaveAttribute("href", "https://www.bybit.com/en/p2p/profile/masked-entry-2/USDC/AMD/item");
  await expect(guide.getByTestId("p2p-profile-link")).toContainText("Open bybit-merchant profile");
  await expect(guide.getByTestId("p2p-profile-link")).toHaveAttribute("target", "_blank");
  await expect(profile).toHaveAttribute("data-side", "buy");
  await expect(profile.getByTestId("bybit-trade-action")).toHaveText("Buy USDC");
  await expect(profile.locator(".profileIcon")).toBeVisible();
  await expect(profile.locator(".bybitNav small, .adsBottom small")).toHaveCount(0);
  await expect(profile.locator(".adPrice")).toContainText("AMD");
  await expect(guide.locator(".playerTimeline button")).toHaveCount(4);

  // Sample rendered positions to catch the original 10 fps cursor stutter.
  const cursorMotion = await profile.locator(".cursorTrack").evaluate(async element => {
    const positions = new Set<string>();
    const start = performance.now();
    let samples = 0;
    await new Promise<void>(resolve => {
      function sample(now: number) {
        positions.add(getComputedStyle(element).transform);
        samples += 1;
        if (now - start >= 600) resolve();
        else requestAnimationFrame(sample);
      }
      requestAnimationFrame(sample);
    });
    return { changes: positions.size, samples };
  });
  expect(cursorMotion.changes).toBeGreaterThan(10);
  expect(cursorMotion.changes / cursorMotion.samples).toBeGreaterThan(.6);

  // The first scene really scrolls the profile, and pausing freezes its clock.
  await expect.poll(() => profile.evaluate(element => Number.parseFloat(getComputedStyle(element).getPropertyValue("--scroll")))).toBeGreaterThan(.05);
  await guide.getByTestId("playback-toggle").click();
  const pausedStyle = await profile.getAttribute("style");
  await page.waitForTimeout(300);
  await expect(profile).toHaveAttribute("style", pausedStyle!);

  await guide.getByRole("button", { name: "Scene 2: Read the reviews", exact: true }).click();
  await expect(profile.getByTestId("bybit-review-panel")).toBeVisible();
  await expect(profile.getByTestId("bybit-ads-panel")).toHaveCount(0);
  await expect(profile.locator(".profileTabs > .active")).toHaveText("Reviews");
  await expect(profile.getByTestId("bybit-review-panel")).toContainText("Illustrative reviews");
  await page.mouse.move(0, 0);
  await scene.screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("bybit-reviews.png") });

  await guide.getByRole("button", { name: "Scene 3: Return to Ads", exact: true }).click();
  await expect(profile.getByTestId("bybit-ads-panel")).toBeVisible();
  await expect(profile.locator(".profileTabs > .active")).toHaveText("Ads");
  await expect(guide.locator(".frameList")).toContainText("buy USDC with AMD");
  await guide.getByRole("button", { name: "Scene 4: Press Buy USDC", exact: true }).click();
  await expect(profile.getByTestId("bybit-trade-action")).toHaveClass(/clicked/);
  await expect(scene.locator(".paymentPreview")).toHaveCount(0);
  await scene.screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("bybit-buy.png") });

  await guide.getByTestId("confirm-instruction-step").click();
  await scene.scrollIntoViewIfNeeded();
  await expect(profile).toHaveAttribute("data-side", "sell");
  await expect(guide.getByTestId("p2p-profile-link")).toHaveAttribute("href", "https://www.bybit.com/en/p2p/profile/masked-exit-2/USDC/RUB/item");
  await expect(profile.locator(".adPrice")).toContainText("RUB");
  await guide.getByRole("button", { name: "Scene 3: Return to Ads", exact: true }).click();
  await expect(guide.locator(".frameList")).toContainText("sell USDC for RUB");
  await guide.getByRole("button", { name: "Scene 4: Press Sell USDC", exact: true }).click();
  await expect(profile.getByTestId("bybit-trade-action")).toHaveText("Sell USDC");
  await expect(profile.getByTestId("bybit-trade-action")).toHaveClass(/sellAction.*clicked/);
  const dimensions = await profile.evaluate(element => ({ width: element.clientWidth, content: element.scrollWidth }));
  expect(dimensions.content).toBeLessThanOrEqual(dimensions.width + 1);
  await page.mouse.move(0, 0);
  await scene.screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("bybit-sell.png") });
});

test("MEXC P2P walkthrough reviews the merchant and chooses the correct buy or sell section", async ({ page }, testInfo) => {
  await mockBackend(page, { mexc: true });
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
    localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
  });
  await openApp(page);
  await page.getByLabel("Amount to send").fill("100000");
  await expect(page.getByTestId("complete-route").first()).toBeVisible();
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const scene = guide.getByTestId("instruction-scene");
  const profile = scene.getByTestId("mexc-profile-card");
  await scene.scrollIntoViewIfNeeded();
  await expect(profile).toBeVisible();
  await expect(profile).toHaveAttribute("data-side", "buy");
  await expect(profile.getByTestId("mexc-advertiser")).toHaveText("mexc-merchant");
  await expect(profile.locator(".wordmark")).toHaveAttribute("src", "/icons/venues/mexc-wordmark.svg");
  await expect.poll(() => profile.locator(".wordmark").evaluate((image: HTMLImageElement) => image.complete && image.naturalWidth > 0)).toBe(true);
  await expect(profile.locator(".selected h3")).toHaveText("Buy from the User");
  await expect(guide.locator(".realAction")).toHaveCount(0);
  // The accelerated first scene advances automatically, then clicks Reviews.
  await expect(profile).toHaveAttribute("data-frame", "1", { timeout: 6000 });
  await expect(profile.getByTestId("mexc-review-panel")).toBeVisible({ timeout: 2000 });
  await guide.getByTestId("playback-toggle").click();
  const pausedStyle = await profile.getAttribute("style");
  await page.waitForTimeout(250);
  await expect(profile).toHaveAttribute("style", pausedStyle!);
  await expect(profile.locator(".profileTabs > .active")).toHaveText("Reviews");

  await guide.getByRole("button", { name: "Scene 3: Return to Ads", exact: true }).click();
  await expect(profile.locator(".selected h3")).toHaveText("Buy from the User");
  await expect(profile.locator(".selected .adPrice")).toContainText("AMD");
  await expect(guide.locator(".frameList")).toContainText('use “Buy from the User” to buy USDT with AMD');
  await guide.getByRole("button", { name: "Scene 4: Press Buy USDT", exact: true }).click();
  await expect(profile.getByTestId("mexc-trade-action")).toHaveText("Buy USDT");
  await expect(profile.getByTestId("mexc-trade-action")).toHaveClass(/clicked/);
  await scene.scrollIntoViewIfNeeded();
  await scene.screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("mexc-buy.png") });

  await guide.getByTestId("confirm-instruction-step").click();
  await guide.getByRole("button", { name: "Scene 3: Return to Ads", exact: true }).click();
  await scene.scrollIntoViewIfNeeded();
  await expect(profile).toHaveAttribute("data-side", "sell");
  await expect(profile.locator(".selected h3")).toHaveText("Sell to the User");
  await expect(profile.getByTestId("mexc-trade-action")).toBeInViewport();
  await expect(profile.locator(".selected .adPrice")).toContainText("RUB");
  await expect(guide.locator(".frameList")).toContainText('use “Sell to the User” to sell USDT for RUB');
  await guide.getByRole("button", { name: "Scene 4: Press Sell USDT", exact: true }).click();
  await expect(profile.getByTestId("mexc-trade-action")).toHaveText("Sell USDT");
  await expect(profile.getByTestId("mexc-trade-action")).toHaveClass(/sellAction.*clicked/);
  await expect(scene.locator(".paymentPreview")).toHaveCount(0);
  const dimensions = await profile.evaluate(element => ({ width: element.clientWidth, content: element.scrollWidth }));
  expect(dimensions.content).toBeLessThanOrEqual(dimensions.width + 1);
  await scene.screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("mexc-sell.png") });
  await guide.getByRole("button", { name: "Scene 2: Read the reviews", exact: true }).click();
  await expect(profile.getByTestId("mexc-review-panel")).toBeVisible();
  await scene.screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("mexc-reviews.png") });
});

test("MEXC profile walkthrough localizes route actions and supports reduced motion", async ({ page }) => {
  await mockBackend(page, { mexc: true });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow-locale", "ru");
    localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
    localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
  });
  await openApp(page);
  await page.locator("#exchange-amount").fill("100000");
  await expect(page.getByTestId("complete-route").first()).toBeVisible();
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const profile = guide.getByTestId("mexc-profile-card");
  await expect(profile).toHaveClass(/paused/);
  await expect(guide.locator(".frameList")).toContainText("на MEXC");
  await expect(profile.getByTestId("mexc-trade-action")).toHaveText("Купить USDT");
  // Both advertisements exist and fit the first scene, before opening Reviews.
  await expect(profile.locator(".tradeAction")).toHaveText(["Купить USDT", "Продать USDT"]);
  for (const action of await profile.locator(".tradeAction").all()) await expect(action).toBeInViewport();
  const initialBuyAds = await profile.getByTestId("mexc-ads-panel").innerText();
  await guide.getByRole("button", { name: "Сцена 3: Вернитесь к Ads", exact: true }).click();
  await expect(profile.getByTestId("mexc-ads-panel")).toHaveText(initialBuyAds, { useInnerText: true });
  await guide.getByRole("button", { name: "Сцена 2: Посмотрите отзывы", exact: true }).click();
  await expect(profile.getByTestId("mexc-review-panel")).toContainText("Пример отзывов");
  await guide.getByTestId("confirm-instruction-step").click();
  await expect(profile.locator(".tradeAction")).toHaveText(["Купить USDT", "Продать USDT"]);
  for (const action of await profile.locator(".tradeAction").all()) await expect(action).toBeInViewport();
  const initialSellAds = await profile.getByTestId("mexc-ads-panel").innerText();
  await guide.getByRole("button", { name: "Сцена 3: Вернитесь к Ads", exact: true }).click();
  await expect(profile.getByTestId("mexc-ads-panel")).toHaveText(initialSellAds, { useInnerText: true });
  await guide.getByRole("button", { name: "Сцена 4: Нажмите «Продать USDT»", exact: true }).click();
  await expect(profile.getByTestId("mexc-trade-action")).toHaveText("Продать USDT");
});

test("Bybit profile walkthrough localizes route actions and supports reduced motion", async ({ page }) => {
  await mockBackend(page);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow-locale", "ru");
    localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
    localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
  });
  await openApp(page);
  await page.locator("#exchange-amount").fill("100000");
  await expect(page.getByTestId("complete-route")).toHaveCount(12);
  await page.getByTestId("complete-route").nth(1).locator(".routeAmount").click();
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const profile = guide.getByTestId("bybit-profile-card");
  await expect(profile).toHaveClass(/paused/);
  await expect(profile.getByTestId("bybit-trade-action")).toHaveText("Купить USDC");
  await guide.getByRole("button", { name: "Сцена 2: Посмотрите отзывы", exact: true }).click();
  await expect(profile.getByTestId("bybit-review-panel")).toContainText("Пример отзывов");
  await guide.getByTestId("confirm-instruction-step").click();
  await guide.getByRole("button", { name: "Сцена 4: Нажмите «Продать USDC»", exact: true }).click();
  await expect(profile.getByTestId("bybit-trade-action")).toHaveText("Продать USDC");
});

test("Armenian bank picker uses the downloaded local icons", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const picker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await picker.getByRole("option", { name: /^AMD / }).click();
  const icons = [
    ["Ameriabank", "/icons/assets/ameriabank-green.png"],
    ["IDBank", "/icons/assets/idbank.png"],
    ["ACBA Bank", "/icons/assets/acba.png"],
    ["Ardshinbank", "/icons/assets/ardshinbank.png"],
    ["Inecobank", "/icons/assets/inecobank.png"],
    ["Evocabank", "/icons/assets/evocabank.png"],
  ] as const;

  for (const [name, src] of icons) {
    await picker.getByLabel("Search banks and payment methods").fill(name);
    await expect(picker.getByRole("option", { name: new RegExp(name) }).locator("img")).toHaveAttribute("src", src);
  }
});

test("anonymous vote reveals service reputation", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);
  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByLabel("Search banks and payment methods").fill("IDBank");
  await sourcePicker.getByRole("option", { name: /IDBank/ }).click();
  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();
  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(12);

  const routeCard = page.getByTestId("complete-route").nth(1);
  const actionRow = routeCard.locator(".routeActionRow");
  await expect(actionRow).toHaveCSS("display", "flex");
  await expect(actionRow.locator(".routeWorkflowButton + .routeFeedback")).toHaveCount(1);
  await expect(routeCard.getByRole("button", { name: "Like this route", exact: true })).toBeVisible();
  await expect(routeCard.getByRole("button", { name: "Dislike this route", exact: true })).toBeVisible();
  await expect(routeCard.getByLabel("850 likes")).toHaveCount(0);
  await expect(routeCard.getByLabel("40 dislikes")).toHaveCount(0);
  await page.evaluate(() => {
    document.documentElement.dataset.theme = "dark";
  });
  await expect(routeCard).toHaveCSS(
    "background-color",
    "rgb(32, 32, 32)",
  );
  await expect(routeCard.locator(".routeFeedback")).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
  await routeCard.getByRole("button", { name: "Like this route", exact: true }).click();
  await expect(routeCard.getByRole("button", { name: "Like this route", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(routeCard.getByRole("button", { name: "Like this route", exact: true })).toHaveCSS("color", "rgb(197, 255, 34)");
  await expect(routeCard.getByLabel("1 likes")).toBeVisible();
});

test("crypto route keeps distinct source and target networks", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  const sourcePicker = await chooseCrypto(page, "sending", "ETH Base");
  await sourcePicker.getByRole("option", { name: /Base.*ETH/ }).click();

  const targetPicker = await chooseCrypto(page, "recipient", "USDT TON");
  await targetPicker.getByRole("option", { name: /TON.*USDT/ }).click();

  await page.getByLabel("Amount to send").fill("0.03");
  await page.getByTestId("start-search").click();

  await expect(page.getByTestId("complete-route").locator(".routeWorkflowButton")).toHaveAttribute(
    "aria-label", /ETH Ether.*Base.*USDT.*TON.*Binance/,
  );

  await page.getByTestId("complete-route").locator(".routeAmount").click();
  const instructions = page.getByTestId("route-guide");
  await expectNumberedTimeline(instructions, ["1"]);
  await instructions.getByTestId("start-guide").click();
  await expect(instructions.getByRole("heading", { name: "Convert ETH to USDT" })).toBeVisible();
  await expect(instructions.getByTestId("spot-terminal")).toHaveAttribute("data-order-side", "sell");
  await expect(instructions.getByTestId("spot-market-link")).toHaveAttribute("href", "https://www.binance.com/en/trade/ETH_USDT?type=spot");
  await expect(instructions.getByText(/select Sell: you spend ETH to receive USDT/)).toBeVisible();
});

for (const venue of ["binance", "bybit", "mexc", "bitget", "whitebird"]) {
  test(`${venue} crypto guide opens Spot and excludes P2P provider instructions`, async ({ page }, testInfo) => {
    await mockBackend(page, { guideVenue: venue, spotGuidance: true });
    await openApp(page);
    const source = await chooseCrypto(page, "sending", "ETH Base");
    await source.getByRole("option", { name: /Base.*ETH/ }).click();
    const target = await chooseCrypto(page, "recipient", "USDT Polygon");
    await target.getByRole("option", { name: /Polygon.*USDT/ }).click();
    await page.getByLabel("Amount to send").fill("0.03");
    await page.getByTestId("start-search").click();
    await page.getByTestId("complete-route").locator(".routeAmount").click();
    const guide = page.getByTestId("route-guide");
    await guide.getByTestId("start-guide").click();
    await expect(guide.getByTestId("spot-terminal")).toHaveAttribute("data-order-side", "sell");
    await expect(guide.getByTestId("spot-market-pair")).toContainText("ETH/USDT");

    await expect(guide.locator(".playerTimeline button")).toHaveCount(5);
    await expect(guide).not.toContainText("Open the P2P advertiser profile.");
    await expect(guide.locator('a[href*="p2p"], a[href*="fiat/trade"], a[href*="advertiser"]')).toHaveCount(0);
    const urls: Record<string, string> = { binance: "https://www.binance.com/en/trade/ETH_USDT?type=spot", bybit: "https://www.bybit.com/trade/spot/ETH/USDT", mexc: "https://www.mexc.com/exchange/ETH_USDT", bitget: "https://www.bitget.com/spot/ETHUSDT", whitebird: "https://whitebird.io/spot/ETH/USDT" };
    await expect(guide.getByTestId("spot-market-link")).toHaveAttribute("href", urls[venue]);
    await guide.getByRole("button", { name: /Scene 3:/ }).click();
    await expect(guide.getByTestId("spot-terminal")).toHaveAttribute("data-frame-kind", "review");
    const cardBounds = await guide.getByTestId("spot-terminal").boundingBox();
    await expect(guide.getByTestId("spot-input-amount")).toContainText("0.03");
    const stageBounds = await guide.getByTestId("instruction-scene").boundingBox();
    expect(cardBounds!.y + cardBounds!.height).toBeLessThanOrEqual(stageBounds!.y + stageBounds!.height);
    const content = await guide.getByTestId("spot-terminal").evaluate(element => ({ width: element.clientWidth, content: element.scrollWidth }));
    expect(content.content).toBeLessThanOrEqual(content.width + 1);
    await guide.getByTestId("instruction-scene").screenshot({ path: testInfo.outputPath(`${venue}-spot.png`) });
    await selectLanguage(page, "ru");
    await expect(guide.locator(".explanation").getByText("Выберите тип ордера и сумму", { exact: true })).toBeVisible();
    await expect(guide.getByTestId("spot-market-link")).toHaveAttribute("href", urls[venue]);
  });
}

test("bridged spot instructions split both market trades into separate steps", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  const sourcePicker = await chooseCrypto(page, "sending", "BTC Bitcoin");
  await sourcePicker.getByRole("option", { name: /Bitcoin.*BTC/ }).click();

  const targetPicker = await chooseCrypto(page, "recipient", "USDT TON");
  await targetPicker.getByRole("option", { name: /TON.*USDT/ }).click();

  await page.getByLabel("Amount to send").fill("0.002");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await page.reload();
  await expect(page.getByRole("button", { name: /Select sending network: Bitcoin/ })).toHaveAttribute("title", "Bitcoin");
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await page.getByTestId("complete-route").locator(".routeAmount").click();

  const instructions = page.getByTestId("route-guide");
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await instructions.getByTestId("start-guide").click();
  await expect(instructions.getByRole("heading", { name: "Convert BTC to USDC" })).toBeVisible();
  await expect(instructions.getByTestId("spot-market-pair")).toContainText("BTC/USDC");
  await expect(instructions.getByTestId("spot-market-link")).toHaveAttribute("href", "https://www.binance.com/en/trade/BTC_USDC?type=spot");
  await instructions.getByTestId("confirm-instruction-step").click();
  await expect(instructions.getByRole("heading", { name: "Convert USDC to USDT" })).toBeVisible();
  await expect(instructions.getByTestId("spot-market-pair")).toContainText("USDC/USDT");
  await expect(instructions.getByTestId("spot-market-link")).toHaveAttribute("href", "https://www.binance.com/en/trade/USDC_USDT?type=spot");
});

test("same asset on different networks reports unavailable bridge provider", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  const sourcePicker = await chooseCrypto(page, "sending", "USDT TRC20");
  await sourcePicker.getByRole("option", { name: /TRON \(TRC-20\).*USDT/ }).click();

  const targetPicker = await chooseCrypto(page, "recipient", "USDT TON");
  await targetPicker.getByRole("option", { name: /TON.*USDT/ }).click();

  await page.getByLabel("Amount to send").fill("125");
  await page.getByTestId("start-search").click();

  await expect(page.getByRole("alert")).toContainText(
    "No live bridge provider is configured for USDT: TRON (TRC-20) → TON",
  );
});


test("same asset on the same network searches a cycle and displays both swap steps", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);
  for (const side of ["sending", "recipient"] as const) {
    const picker = await chooseCrypto(page, side, "USDT ERC20");
    await picker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDT/ }).click();
  }
  await page.getByLabel("Amount to send").fill("100");
  await page.getByTestId("start-search").click();
  const card = page.locator(".routeCard").first();
  await expect(card).toBeVisible();
  await expect(card).toContainText("+1 USDT (+1.00%)");
  await expect(card.getByTestId("best-route-badge")).toHaveText("Best router");
  await expect(card.locator('.routeBadges .bestBadge:not([data-testid="best-route-badge"])')).toHaveText("+1 USDT (+1.00%)");
  await expect(card.locator(".routeBadges")).not.toContainText("Est.");
  await expect(card).toContainText("101 USDT");
  await expect(card).toContainText("USDC");
  await expect(card).not.toContainText("AMD");
  await card.locator(".workflow").click();
  const instructions = page.getByTestId("route-guide");
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await instructions.getByTestId("start-guide").click();
  await expect(instructions.getByRole("heading", { name: "Convert USDT in Ethereum (ERC-20) to USDC in Ethereum (ERC-20)" })).toBeVisible();
  await expect(instructions.getByTestId("cow-swap-swap-card")).toBeVisible();
  await instructions.getByTestId("confirm-instruction-step").click();
  await expect(instructions.getByRole("heading", { name: "Convert USDC in Ethereum (ERC-20) to USDT in Ethereum (ERC-20)" })).toBeVisible();
  await expect(instructions.locator(".realAction")).toHaveCount(0);
});

test("spot crypto cycles show three trades and wallet deposit and withdrawal instructions", async ({ page }) => {
  await mockBackend(page, { spotCycle: true });
  await openApp(page);
  for (const side of ["sending", "recipient"] as const) {
    const picker = await chooseCrypto(page, side, "USDT ERC20");
    await picker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDT/ }).click();
  }
  await page.getByLabel("Amount to send").fill("100");
  await page.getByTestId("start-search").click();
  const card = page.locator(".routeCard").first();
  await expect(card).toContainText("+1 USDT (+1.00%)");
  await expect(card.getByTestId("best-route-badge")).toHaveText("Best router");
  await expect(card.locator('.routeBadges .bestBadge:not([data-testid="best-route-badge"])')).toHaveText("+1 USDT (+1.00%)");
  await expect(card.locator(".routeBadges")).not.toContainText("Est.");
  await card.locator(".workflow").click();
  const instructions = page.getByTestId("route-guide");
  await expectNumberedTimeline(instructions, ["1", "2", "3"]);
  await instructions.getByTestId("start-guide").click();
  await expect(instructions.getByTestId("spot-market-pair")).toContainText("ETH/USDT");
  await expect(instructions.getByTestId("spot-terminal")).toHaveAttribute("data-order-side", "buy");
  await expect(instructions.getByTestId("spot-market-link")).toHaveAttribute("href", "https://www.bybit.com/trade/spot/ETH/USDT");
  await expect(instructions.locator(".realAction")).toHaveCount(0);
  await instructions.getByTestId("confirm-instruction-step").click();
  await expect(instructions.getByTestId("spot-market-pair")).toContainText("BTC/ETH");
  await expect(instructions.getByTestId("spot-terminal")).toHaveAttribute("data-order-side", "buy");
  await expect(instructions.getByTestId("spot-market-link")).toHaveAttribute("href", "https://www.bybit.com/trade/spot/BTC/ETH");
  await instructions.getByTestId("confirm-instruction-step").click();
  await expect(instructions.getByTestId("spot-terminal")).toHaveAttribute("data-order-side", "sell");
  await expect(instructions.getByTestId("spot-market-link")).toHaveAttribute("href", "https://www.bybit.com/trade/spot/BTC/USDT");
  await expect(instructions.locator(".realAction")).toHaveCount(0);
  await expect(instructions).not.toContainText("Bybit P2P results");
});

test("header and footer align with the workspace and mobile controls remain reachable", async ({ page, isMobile }) => {
  await mockBackend(page);
  await openApp(page);
  await settleEntrance(page);
  for (const width of isMobile ? [320, 412, 768] : [1024, 1440]) {
    await page.setViewportSize({ width, height: 900 });
    const layout = await page.evaluate(() => {
      const rect = (selector: string) => document.querySelector(selector)!.getBoundingClientRect();
      const header = rect(".header .inner"), workspace = rect(".workspace"), footer = rect(".siteFooter");
      return { headerLeft: header.left, headerRight: header.right, workspaceLeft: workspace.left, workspaceRight: workspace.right, footerLeft: footer.left, footerRight: footer.right, brandRight: rect(".wordmark").right, walletLeft: rect(".walletToggle").left };
    });
    expect(Math.abs(layout.headerLeft - layout.workspaceLeft)).toBeLessThan(1);
    expect(Math.abs(layout.headerRight - layout.workspaceRight)).toBeLessThan(1);
    expect(Math.abs(layout.footerLeft - layout.workspaceLeft)).toBeLessThan(1);
    expect(Math.abs(layout.footerRight - layout.workspaceRight)).toBeLessThan(1);
    expect(layout.brandRight).toBeLessThanOrEqual(layout.walletLeft);
    await expect(page.locator(".header .inner")).toHaveCSS("box-shadow", "none");
    await expect(page.locator(".card")).toHaveCSS("box-shadow", "none");
    if (isMobile) {
      const smallTargets = await page.locator("button, a[aria-label]").evaluateAll((elements) => elements.filter((element) => {
        const rect = element.getBoundingClientRect();
        return element.checkVisibility({ checkVisibilityCSS: true }) && (rect.width < 43.9 || rect.height < 43.9);
      }).map((element) => element.outerHTML));
      expect(smallTargets).toEqual([]);
    }
  }
});

test("header context menu stays by its button and supports keyboard navigation", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);
  await settleEntrance(page);
  const toggle = page.getByRole("button", { name: "Open menu" });
  const menu = page.getByRole("menu", { name: "Menu", exact: true });
  for (const width of [1280, 393, 320]) {
    await page.setViewportSize({ width, height: 900 });
    const before = await page.locator(".brand").boundingBox();
    await toggle.click();
    await expect(menu).toBeVisible();
    await menu.evaluate(async node => {
      await Promise.all(node.getAnimations().map(animation => animation.finished));
    });
    const toggleBox = await toggle.boundingBox(), menuBox = await menu.boundingBox();
    expect(menuBox!.x).toBeCloseTo(toggleBox!.x, 0);
    expect(menuBox!.y).toBeGreaterThan(toggleBox!.y + toggleBox!.height);
    expect(menuBox!.x + menuBox!.width).toBeLessThanOrEqual(width);
    expect(menuBox!.height).toBeLessThan(300);
    expect(await page.locator(".brand").boundingBox()).toEqual(before);
    await expect(menu.getByRole("menuitem")).toHaveCount(4);
    await expect(menu.getByRole("menuitem").first()).toBeFocused();
    await expect(page.locator(".appShell")).not.toHaveAttribute("inert", "");
    await expect(page.locator(".actionsBackdrop")).toHaveCount(0);
    await page.keyboard.press("ArrowUp");
    await expect(menu.getByRole("menuitem", { name: "Open Pay3Flow on GitHub" })).toBeFocused();
    await page.keyboard.press("Home");
    await expect(menu.getByRole("menuitem").first()).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);
    await expect(toggle).toBeFocused();
    await toggle.click();
    await toggle.click();
    await expect(menu).toHaveCount(0);
    await toggle.click();
    await page.getByRole("button", { name: "Switch theme" }).click();
    await expect(menu).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Switch theme" })).toBeFocused();
    await toggle.focus();
    await page.keyboard.press("ArrowUp");
    await expect(menu.getByRole("menuitem").last()).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(menu).toHaveCount(0);
    await expect(page.locator(".brand")).toBeFocused();
  }
});

test("icon tooltips work on hover and keyboard focus without clipping", async ({ page, isMobile }) => {
  test.skip(isMobile, "Hover and keyboard layout only");
  await mockBackend(page);
  await openApp(page);
  await settleEntrance(page);
  const button = page.getByRole("button", { name: "Route refresh settings" });
  const tooltip = page.locator("#icon-control-tooltip");
  await button.hover();
  await expect(tooltip).toHaveText("Route refresh settings");
  await expect(button).toHaveAttribute("aria-describedby", "icon-control-tooltip");
  const buttonBox = await button.boundingBox(), tipBox = await tooltip.boundingBox();
  expect(tipBox!.y).toBeGreaterThan(buttonBox!.y + buttonBox!.height);
  await tooltip.hover();
  await expect(tooltip).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(tooltip).toHaveCount(0);
  await expect(button).not.toHaveAttribute("aria-describedby");
  await page.mouse.move(0, 0);
  await page.keyboard.press("Tab");
  await button.focus();
  await expect(tooltip).toHaveText("Route refresh settings");
  await page.keyboard.press("Escape");
  const picker = await chooseCrypto(page, "sending", "USDT ERC20");
  await picker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDT/ }).click();
  const asset = page.getByRole("button", { name: "Select sending asset: USDT", exact: true });
  await asset.hover();
  await expect(tooltip).toContainText("USDT");
  await expect(tooltip).toContainText("Tether");
  await page.mouse.move(0, 0);
  await page.getByLabel("Amount to send").focus();
  await page.keyboard.press("Tab");
  await expect(asset).toBeFocused();
  await expect(tooltip).toContainText("USDT");
});

for (const theme of ["light", "dark"] as const) {
  test(`readable text and WCAG contrast in ${theme} theme across exchange dialogs`, async ({ page, isMobile }) => {
    const { default: AxeBuilder } = await import("@axe-core/playwright");
    await page.emulateMedia({ colorScheme: theme });
    await mockBackend(page);
    await page.addInitScript(() => {
      localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
      localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
    });
    await openApp(page);
    await settleEntrance(page);
    const audit = async () => {
      await page.locator('[role="dialog"], .side, .routeList > li').evaluateAll(async (elements) => {
        await Promise.all(elements.flatMap((element) => element.getAnimations().filter((animation) => animation.effect?.getTiming().iterations !== Infinity).map((animation) => animation.finished.catch(() => {}))));
      });
      const report = await new AxeBuilder({ page }).withRules(["color-contrast"]).analyze();
      expect(report.violations.map((violation) => ({ id: violation.id, nodes: violation.nodes.map((node) => ({ target: node.target, summary: node.failureSummary })) }))).toEqual([]);
      const smallText = await page.locator("body *").evaluateAll((elements) => elements.filter((element) =>
        element.checkVisibility() && [...element.childNodes].some((node) => node.nodeType === Node.TEXT_NODE && node.textContent?.trim()) && Number.parseFloat(getComputedStyle(element).fontSize) < 12
      ).map((element) => ({ text: element.textContent, size: getComputedStyle(element).fontSize })));
      expect(smallText).toEqual([]);
      if (isMobile) {
        const smallTargets = await page.locator('button, a[aria-label]').evaluateAll((elements) => elements.filter((element) => {
          const rect = element.getBoundingClientRect();
          return element.checkVisibility({ checkVisibilityCSS: true }) && (rect.width < 43.9 || rect.height < 43.9);
        }).map((element) => ({ label: element.getAttribute("aria-label") || element.textContent?.trim(), width: element.getBoundingClientRect().width, height: element.getBoundingClientRect().height })));
        expect(smallTargets).toEqual([]);
      }
    };
    await audit();
    const assetPicker = await openCryptoPicker(page, "sending");
    await assetPicker.getByRole("option", { name: /^USDT\b/ }).click();
    await audit();
    await assetPicker.getByRole("option", { name: "Ethereum (ERC-20) USDT · Tether" }).click();
    await page.getByRole("button", { name: "Select sending network: Ethereum (ERC-20)" }).click();
    await audit();
    await page.keyboard.press("Escape");
    const bankPicker = await openCryptoPicker(page, "sending");
    await bankPicker.getByRole("option", { name: /^AMD\b/ }).click();
    await bankPicker.getByRole("option", { name: /^IDBank Bank transfer · AMD/ }).click();
    await page.getByLabel("Amount to send").fill("100000");
    await expect(page.getByTestId("complete-route").first()).toBeVisible();
    await audit();
    await page.getByRole("button", { name: "Route refresh settings" }).click();
    await audit();
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "Choose exchanges" }).click();
    await audit();
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: /Select sending bank:/ }).click();
    await audit();
    await page.keyboard.press("Escape");
    await page.getByTestId("start-search").click();
    await expect(page.getByTestId("route-guide")).toBeVisible();
    await audit();
  });
}


test("route panel slides in both directions and respects reduced motion", async ({ page, isMobile }) => {
  await mockBackend(page);
  await openApp(page);
  await settleEntrance(page);
  const selector = isMobile ? ".mobileRoutesToggle" : ".routesToggle";
  for (const expanded of [false, true]) {
    const motion = await page.evaluate(async (selector) => {
      (document.querySelector(selector) as HTMLButtonElement).click();
      await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
      const panel = document.querySelector(".routesReveal") as HTMLElement;
      return {
        present: !!panel,
        inert: panel?.inert,
        keyframes: panel?.getAnimations().flatMap((animation) => (animation.effect as KeyframeEffect).getKeyframes()).map((frame) => frame.transform).filter(Boolean) ?? [],
        animations: panel?.getAnimations().filter((animation) => animation.playState === "running").length ?? 0
      };
    }, selector);
    expect(motion.present).toBe(true);
    expect(motion.inert).toBe(!expanded);
    expect(motion.animations).toBeGreaterThan(0);
    expect(motion.keyframes.some((transform) => transform !== "none")).toBe(true);
    if (expanded) await expect(page.locator("#routes")).toBeVisible();
    else await expect(page.locator("#routes")).toHaveCount(0);
    await page.locator(".routesReveal").evaluateAll(async (elements) => {
      await Promise.all(elements.flatMap((element) => element.getAnimations().map((animation) => animation.finished.catch(() => {}))));
    });
  }
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.getByRole("button", { name: "Hide routes" }).click();
  await expect(page.locator("#routes")).toHaveCount(0);
  await page.getByRole("button", { name: "Show routes" }).click();
  await expect(page.locator("#routes")).toBeVisible();
  await expect(page.locator(".routesReveal")).toHaveCSS("transform", "none");
});


test("header context menu animates and respects reduced motion", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);
  await settleEntrance(page);
  const toggle = page.getByRole("button", { name: "Open menu" });
  const menu = page.getByRole("menu", { name: "Menu", exact: true });
  await toggle.click();
  const closing = await menu.evaluate(async element => {
    (document.querySelector(".menuToggle") as HTMLButtonElement).click();
    await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    return { present: element.isConnected, animations: element.getAnimations().length };
  });
  expect(closing.present).toBe(true);
  expect(closing.animations).toBeGreaterThan(0);
  await expect(menu).toHaveCount(0);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await toggle.click();
  await expect(menu).toBeVisible();
  expect(await menu.evaluate(node => node.getAnimations().filter(animation => animation.playState === "running").length)).toBe(0);
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
});

test("route instructions load provider reviews for every cycle step", async ({ page }) => {
  await mockBackend(page, { reviews: true });
  const requests: string[] = [];
  await page.route("**/api/reviews/providers/*", async (request) => {
    const provider = new URL(request.request().url()).pathname.split("/").pop()!;
    requests.push(provider);
    await request.fulfill({ contentType: "application/json", body: JSON.stringify({
      source_url: `https://trustscores.org/companies/${provider}`,
      fetched_at: "2026-10-08T12:00:00Z",
      reviews: Array.from({ length: 7 }, (_, index) => ({
        id: `${provider}-${index}`, author: `Reviewer ${index + 1}`, text: `${provider} review ${index + 1}`,
        rating: index === 6 ? 2 : 5, created_at: "2026-10-01T12:00:00Z", url: `https://trustscores.org/companies/${provider}`,
      })),
    }) });
  });
  await openApp(page);
  for (const side of ["sending", "recipient"] as const) {
    const picker = await chooseCrypto(page, side, "USDT ERC20");
    await picker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDT/ }).click();
  }
  await page.getByLabel("Amount to send").fill("100");
  await expect(page.locator(".routeCard").first()).toBeVisible();
  await page.locator(".routeCard").first().locator(".routeWorkflowButton").click();
  const instructions = page.getByTestId("route-guide");
  await instructions.locator(".reviewsTab").click();
  const reviews = instructions.getByRole("region", { name: "Customer reviews" });
  await expect(reviews).toHaveCount(2);
  await expect(reviews.first()).toContainText("cow-swap review 1");
  await expect(reviews.first().locator(".review")).toHaveCount(7);
  await expect(reviews.first().locator(".stars")).toHaveCount(7);
  await expect(instructions.locator(".reviewSources, .reviewTools, .cardFoot")).toHaveCount(0);
  await expect(reviews.nth(1)).toContainText("near-intents review 1");
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ animations: "disabled", path: test.info().outputPath("guide-reviews.png") });
  expect(requests.sort()).toEqual(["cow-swap", "near-intents"]);
  await instructions.locator(".guideToolbar").getByRole("button", { name: /Back to routes/ }).click();
  await expect(instructions).toBeHidden();
});


test("P2P instructions restore written advertiser reviews", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
    localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
  });
  await mockBackend(page, { reviews: true });
  const profiles: string[] = [];
  await page.route("**/api/reviews/profile?*", async (request) => {
    const profile = new URL(request.request().url()).searchParams.get("url")!;
    profiles.push(profile);
    const id = new URL(profile).searchParams.get("advertiserNo")!;
    await request.fulfill({ contentType: "application/json", body: JSON.stringify({
      source_url: profile, fetched_at: "2026-10-08T12:00:00Z",
      reviews: [{ id, author: "Customer", text: `Written feedback for ${id}`, rating: 5, created_at: null, url: profile }],
    }) });
  });
  await openApp(page);
  await page.getByLabel("Amount to send").fill("100000");
  await expect(page.getByTestId("complete-route").first()).toBeVisible();
  await page.getByTestId("complete-route").first().locator(".routeWorkflowButton").click();
  const instructions = page.getByTestId("route-guide");
  await instructions.locator(".reviewsTab").click();
  const reviews = instructions.getByRole("region", { name: "Customer reviews" });
  await expect(reviews).toHaveCount(2);
  for (const section of await reviews.all()) await expect(section).toContainText("Written feedback for");
  await expect.poll(() => new Set(profiles).size).toBe(2);
});

test("animated guide has inline actions, scene controls and fresh progress on every opening", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
    localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
  });
  await mockBackend(page);
  await openApp(page);
  await page.getByLabel("Amount to send").fill("100000");
  await expect(page.getByTestId("complete-route").first()).toBeVisible();
  const trigger = page.getByTestId("complete-route").first().locator(".routeAmount");
  await trigger.click();
  const guide = page.getByTestId("route-guide");
  await expect(guide.getByRole("heading", { name: "A clear route. At your pace." })).toBeVisible();
  const box = await guide.boundingBox();
  expect(box!.width).toBeCloseTo(page.viewportSize()!.width, 1);
  expect(box!.height).toBeGreaterThan(400);
  await expect(page).toHaveURL(/#\/guide\/AMD\/RUB\?/);
  await expect(page.locator(".header .brand")).toBeVisible();
  await expect(page.locator(".workspace")).toBeHidden();
  await expect(guide.locator(".routePair img")).toHaveCount(2);
  const actions = guide.locator(".guideActions");
  await expect(guide.locator("footer")).toHaveCount(0);
  await expect(actions).toHaveCount(0);
  await expect(guide.locator(".estimate").getByTestId("start-guide")).toHaveCount(1);
  await expect(guide.locator(".visualCaption")).toHaveCount(0);
  await expect(guide.locator(".smallLogo")).toHaveAttribute("src", "/icons/assets/pay3flow_logo.svg");
  await expect(guide.locator(".sourceCoin img")).toHaveAttribute("src", "/icons/flags/am.svg");
  await expect(guide.locator(".targetCoin img")).toHaveAttribute("src", "/icons/flags/ru.svg");
  await expect(guide.locator(".overviewButton img")).toHaveAttribute("src", "/icons/ui/guide-before.svg");
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  expect(await page.evaluate(() => window.scrollY)).toBeGreaterThan(0);
  await guide.getByTestId("start-guide").scrollIntoViewIfNeeded();
  await expect(guide.getByTestId("start-guide")).toBeInViewport();
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ animations: "disabled", path: testInfo.outputPath("guide-overview.png") });
  const chapters = guide.getByTestId("instruction-step").getByRole("button");
  await expect(chapters.nth(1)).toBeDisabled();
  await guide.getByTestId("start-guide").click();
  await expect(guide.locator("h1")).toBeFocused();
  await expect(actions).toHaveCSS("position", "static");
  await expect(guide.locator(".guideMain > .guideActions")).toHaveCount(1);
  await expect(guide.locator(".footerDot")).toHaveCount(0);
  await expect(guide.getByTestId("confirm-instruction-step")).toBeVisible();
  await guide.locator(".videoStage").hover();
  const center = guide.getByTestId("center-playback");
  await expect(center).toHaveCSS("opacity", "1");
  await center.click();
  await page.mouse.move(0, 0);
  await expect(center).toHaveCSS("opacity", "1");
  await expect(center).toHaveAttribute("aria-label", "Play walkthrough");
  await expect(center.locator(".playIcon")).toBeVisible();
  await guide.getByRole("button", { name: "Next scene", exact: true }).click();
  await expect(guide.locator(".sceneCount")).toHaveText("02 / 04");
  await guide.getByRole("button", { name: "Previous scene", exact: true }).click();
  await expect(guide.locator(".sceneCount")).toHaveText("01 / 04");
  await guide.getByRole("button", { name: "Scene 3: Close details and find the advertisement" }).click();
  await expect(guide.locator(".sceneCount")).toHaveText("03 / 04");
  await expect(chapters.nth(1)).toBeDisabled();
  await guide.getByRole("button", { name: "Replay walkthrough" }).click();
  await expect(guide.locator(".sceneCount")).toHaveText("01 / 04");
  await page.evaluate(() => window.scrollTo({ top: 0 }));
  await page.mouse.move(0, 0);
  await page.screenshot({ animations: "disabled", path: testInfo.outputPath("guide-first-step.png") });
  await guide.getByTestId("confirm-instruction-step").click();
  await expect(guide.locator("h1")).toContainText("Sell USDT");
  await expect(chapters.nth(0).locator(".stepNumber")).toHaveText("✓");
  expect(await page.evaluate(() => Object.keys(sessionStorage).filter(key => key.startsWith("pay3flow.tutorial.v1.")))).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(guide).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => document.body.style.position)).toBe("");
  await trigger.click();
  await expect(guide.locator("h1")).toHaveText("A clear route. At your pace.");
  await expect(guide.locator(".chapterProgress strong")).toHaveText("0/2");
  await expect(chapters.nth(1)).toBeDisabled();
  await page.reload();
  await expect(guide.locator("h1")).toHaveText("A clear route. At your pace.");
  await expect(guide.locator(".chapterProgress strong")).toHaveText("0/2");
  await guide.getByTestId("start-guide").click();
  await guide.getByTestId("confirm-instruction-step").click();
  await expect(guide.locator("h1")).toContainText("Sell USDT");
  await guide.getByTestId("confirm-instruction-step").focus();
  await page.keyboard.press("Tab");
  await expect(guide.getByTestId("confirm-instruction-step")).not.toBeFocused();
  await guide.getByTestId("confirm-instruction-step").click();
  await expect(guide.getByRole("heading", { name: "Every step. Done." })).toBeVisible();
  await expect(guide.getByText("This checklist records your confirmations. It does not verify payments or balances.")).toBeVisible();
  await guide.locator(".guideActions").getByRole("button", { name: "Back to routes", exact: true }).click();
  await expect(guide).toHaveCount(0);
});

test("Russian guide supports reduced motion and accessible controls", async ({ page }, testInfo) => {
  await page.emulateMedia({ reducedMotion: "reduce", colorScheme: "dark" });
  await page.addInitScript(() => localStorage.setItem("pay3flow-locale", "ru"));
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
    localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
  });
  await mockBackend(page);
  await openApp(page);
  await page.locator("#exchange-amount").fill("100000");
  await expect(page.getByTestId("complete-route").first()).toBeVisible();
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();
  const guide = page.getByTestId("route-guide");
  await expect(guide.locator("h1")).toHaveText("Понятный маршрут. В вашем темпе.");
  await guide.getByTestId("start-guide").click();
  await expect(guide.getByTestId("center-playback")).toHaveAccessibleName("Продолжить демонстрацию");
  await guide.getByRole("button", { name: "Сцена 2: Откройте «Подробнее»" }).click();
  await expect(guide.locator(".sceneCount")).toHaveText("02 / 04");
  await expect(guide.getByTestId("confirm-instruction-step")).toHaveText("Сделал, дальше →");
  const overflow = await guide.evaluate((element) => element.scrollWidth > element.clientWidth);
  expect(overflow).toBe(false);
  const AxeBuilder = (await import("@axe-core/playwright")).default;
  const accessibility = await new AxeBuilder({ page }).include('[data-testid="route-guide"]').analyze();
  expect(accessibility.violations.map((item) => ({ id: item.id, nodes: item.nodes.map((node) => node.target) }))).toEqual([]);
  await page.evaluate(() => window.scrollTo({ top: 0 }));
  await page.mouse.move(0, 0);
  await page.screenshot({ animations: "disabled", path: testInfo.outputPath("guide-russian-dark.png") });
});


test("guide URL restores banks and selected operations in a fresh browser", async ({ page, browser }) => {
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
    localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
  });
  await mockBackend(page);
  await openApp(page);
  await page.getByLabel("Amount to send").fill("100000");
  await expect(page.getByTestId("complete-route").first()).toBeVisible();
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();
  const url = page.url();
  await page.evaluate(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (text: string) => sessionStorage.setItem("test.share-url", text) } }));
  await expect(page.getByTestId("route-guide").getByRole("button", { name: /Share guide/ }).locator(".shareIcon")).toBeVisible();
  await page.getByTestId("route-guide").getByRole("button", { name: /Share guide/ }).click();
  await expect(page.getByTestId("route-guide").getByRole("button", { name: /Link copied/ })).toBeVisible();
  const share = await page.evaluate(() => sessionStorage.getItem("test.share-url"));
  expect(share).toContain("/share/guide/AMD/RUB?");
  expect(url).toContain("from=am-idbank");
  expect(url).toContain("to=ru-alfabank");
  expect(url).not.toMatch(/tracking_token|execution_token/);
  const context = await browser.newContext();
  const fresh = await context.newPage();
  await mockBackend(fresh);
  await fresh.goto(share!);
  const guide = fresh.getByTestId("route-guide");
  await expect(guide.locator("h1")).toHaveText("A clear route. At your pace.");
  await guide.getByTestId("start-guide").click();
  await expect(guide.locator(".headingVenue")).toContainText("Binance");
  await guide.getByRole("button", { name: "Scene 3: Close details and find the advertisement", exact: true }).click();
  await expect(guide.getByTestId("binance-profile-card").locator(".adSection.highlight .payment")).toHaveText("Bank transfer");
  await fresh.reload();
  await expect(guide.locator("h1")).toHaveText("A clear route. At your pace.");
  await expect(guide.locator(".chapterProgress strong")).toHaveText("0/2");
  await guide.getByTestId("start-guide").click();
  await expect(guide.locator(".headingVenue")).toContainText("Binance");
  await guide.locator(".guideToolbar").getByRole("button", { name: /Back to routes/ }).click();
  await expect(fresh).toHaveURL(/#\/swap\/AMD\/RUB/);
  await expect(fresh.locator(".workspace")).toBeVisible();
  await expect(fresh.getByRole("button", { name: /Select sending bank: IDBank/ })).toBeVisible();
  await expect(fresh.getByRole("button", { name: /Select recipient bank: Alfa-Bank/ })).toBeVisible();
  await context.close();
  await page.goBack();
  await expect(page.getByTestId("route-guide")).toHaveCount(0);
  await page.goForward();
  await expect(page.getByTestId("route-guide")).toBeVisible();
});

test("exchange share exposes a bridge card to messengers without JavaScript", async ({ request, browser, baseURL }) => {
  const path = "/swap/AMD/RUB?amount=100000&receive=20350&from=am-idbank&to=ru-alfabank&sources=bybit&methods=p2p&lang=ru&fromName=IDBank&toName=Alfa-Bank&fromIcon=/icons/assets/idbank.png&toIcon=/icons/assets/alfabank.webp&execution_token=excluded";
  const response = await request.get(path);
  expect(response.ok()).toBe(true);
  const html = await response.text();
  expect(html).toContain("100000 AMD → 20350 RUB");
  expect(html).toContain('property="og:image"');
  expect(html).toContain('property="og:image:width" content="1200"');
  expect(html).toContain('name="twitter:card" content="summary_large_image"');
  expect(html).not.toContain("execution_token");
  const imageUrl = html.match(/property="og:image" content="([^"]+)"/)![1].replaceAll("&amp;", "&");
  expect(new URL(imageUrl).searchParams.get("fromName")).toBe("IDBank");
  expect(new URL(imageUrl).searchParams.get("toName")).toBe("Alfa-Bank");
  const image = await request.get(imageUrl);
  expect(image.headers()["content-type"]).toBe("image/png");
  const png = await image.body();
  const sharp = (await import("sharp")).default;
  expect(await sharp(png).metadata()).toMatchObject({ width: 1200, height: 630, format: "png" });
  const differentBank = new URL(imageUrl);
  differentBank.searchParams.set("toName", "Sberbank");
  expect((await (await request.get(differentBank.toString())).body()).equals(png)).toBe(false);
  const networkImage = new URL(imageUrl);
  networkImage.searchParams.set("fromNetworkName", "Ethereum (ERC-20)");
  expect((await (await request.get(networkImage.toString())).body()).equals(png)).toBe(false);
  const other = await request.get("/share-image.png?from=ETH&to=BYN&amount=1&receive=840.11");
  expect((await other.body()).equals(png)).toBe(false);
  const context = await browser.newContext({ javaScriptEnabled: false });
  try {
    const page = await context.newPage();
    await page.goto(new URL(path, baseURL).toString());
    await expect(page.getByRole("heading", { level: 1 })).toContainText("100000 AMD → 20350 RUB");
    await expect(page.getByRole("link", { name: "Открыть обмен" })).toHaveAttribute("href", /#\/swap\/AMD\/RUB\?amount=100000&from=am-idbank&to=ru-alfabank/);
  } finally { await context.close(); }
  const legacy = await request.get("/swap/USDT/KZT?amount=125&sm=global-usdt&tm=kz-kaspi&sn=tron&modes=p2p");
  expect(await legacy.text()).toContain("from=global-usdt");
});

test("guide share exposes server metadata and a route-specific PNG", async ({ request }) => {
  const response = await request.get("/share/guide/ETH/BYN?amount=1&from=crypto-eth&to=by-bank&venues=whitebird&lang=ru&execution_token=excluded");
  expect(response.ok()).toBe(true);
  const html = await response.text();
  expect(html).toContain('property="og:image"');
  expect(html).toContain('name="twitter:card" content="summary_large_image"');
  expect(html).toContain("ETH → BYN");
  expect(html).not.toContain("execution_token");
  const image = await request.get("/share/guide/ETH/BYN/preview.png?venues=whitebird&lang=ru");
  expect(image.ok()).toBe(true);
  expect(image.headers()["content-type"]).toBe("image/png");
  const bytes = await image.body();
  expect([...bytes.subarray(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
  const sharp = (await import("sharp")).default;
  const metadata = await sharp(bytes).metadata();
  expect([metadata.width, metadata.height]).toEqual([1200, 630]);
  const other = await request.get("/share/guide/BTC/USD/preview.png?venues=binance");
  expect((await other.body()).equals(bytes)).toBe(false);
});


test("a guide does not substitute a missing bank or an unavailable operation", async ({ page }) => {
  await mockBackend(page);
  await page.goto("/#/guide/AMD/RUB?amount=100000&from=missing-bank&to=ru-alfabank");
  await expect(page.getByRole("heading", { name: "This route is no longer available" })).toBeVisible();
  await expect(page.getByTestId("route-guide")).toHaveCount(0);
  await page.getByRole("button", { name: /Back to routes/ }).click();
  await expect(page.locator(".workspace")).toBeVisible();
  await page.goto('/#/guide/AMD/RUB?amount=100000&from=am-idbank&to=ru-alfabank&path=unavailable-operation');
  await expect(page.getByRole("heading", { name: "This route is no longer available" })).toBeVisible();
  await expect(page.getByTestId("route-guide")).toHaveCount(0);
});

test("converter walkthrough demonstrates both pickers without changing the exchange", async ({ page }) => {
  await mockBackend(page);
  await page.addInitScript(() => { localStorage.setItem("pay3flow.exchange.source-method", "am-idbank"); localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank"); });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openApp(page);
  await page.getByLabel("Amount to send").fill("2500");
  await expect(page.getByTestId("complete-route").first()).toBeVisible();
  await page.getByTestId("complete-route").nth(1).locator(".routeRank").click();
  const original = await page.evaluate(() => ({ hash: location.hash, storage: JSON.stringify(localStorage), amount: (document.querySelector("#exchange-amount") as HTMLInputElement).value, selected: document.querySelector(".routeCard.selected .routeRank")?.textContent }));
  let searches = 0;
  page.on("request", request => { if (/\/api\/p2p\/routes/.test(request.url())) searches++; });
  await page.getByTestId("start-converter-walkthrough").click();
  const guide = page.getByTestId("converter-walkthrough");
  await expect(guide).toBeVisible();
  await expect(guide.locator(".demoPicker .dialog")).toBeVisible({ timeout: 5000 });
  await expect(guide.locator(".demoStage .moneyPanelSource .methodText strong")).toHaveText("Ameriabank", { timeout: 6000 });
  await expect(guide).toHaveAttribute("data-step", "3", { timeout: 6000 });
  await expect(guide.locator(".demoPicker .dialog")).toBeVisible();
  await page.screenshot({ path: test.info().outputPath("demo-picker.png") });
  await expect(guide).toHaveAttribute("data-step", "5", { timeout: 7000 });
  await page.screenshot({ path: test.info().outputPath("demo-amount.png") });
  await expect(guide.locator(".demoStage .moneyPanelTarget .methodText strong")).toHaveText("Sberbank");
  await expect(guide.locator(".demoStage .moneyPanelSource .amountInput")).toHaveValue("1000");
  await expect(guide).toHaveCount(0, { timeout: 4000 });
  expect(await page.evaluate(() => ({ hash: location.hash, storage: JSON.stringify(localStorage), amount: (document.querySelector("#exchange-amount") as HTMLInputElement).value, selected: document.querySelector(".routeCard.selected .routeRank")?.textContent }))).toEqual(original);
  expect(searches).toBe(0);
  await expect(page.getByTestId("start-converter-walkthrough")).toBeFocused();
  expect(await page.locator("body").evaluate(node => node.style.position)).not.toBe("fixed");
  expect(await page.locator(".shell").evaluate(node => node.inert)).toBe(false);
});

test("converter walkthrough cancels from an open picker and restores scroll and focus", async ({ page }) => {
  await mockBackend(page);
  await page.addInitScript(() => { localStorage.setItem("pay3flow.exchange.source-method", "am-idbank"); localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank"); });
  await openApp(page);
  await page.getByTestId("start-converter-walkthrough").scrollIntoViewIfNeeded();
  const scroll = await page.evaluate(() => scrollY);
  await page.getByTestId("start-converter-walkthrough").click();
  const guide = page.getByTestId("converter-walkthrough");
  await expect(guide.locator(".demoPicker .dialog")).toBeVisible({ timeout: 6000 });
  await page.keyboard.press("Escape");
  await expect(guide).toHaveCount(0);
  await expect(page.getByTestId("start-converter-walkthrough")).toBeFocused();
  expect(await page.evaluate(() => scrollY)).toBe(scroll);
  expect(await page.locator("body").evaluate(node => node.style.position)).not.toBe("fixed");
  await page.getByTestId("start-converter-walkthrough").click();
  await expect(guide).toBeVisible();
  await guide.getByRole("button", { name: "Close guide", exact: true }).click();
  await expect(guide).toHaveCount(0);
  await page.getByLabel("Amount to send").fill("1000");
  await expect(page.getByTestId("complete-route").first()).toBeVisible();
});

test("route scrollbar follows ranking depth and overview includes bank routes", async ({ page }) => {
  await mockBackend(page, { routeCount: 101 });
  await page.addInitScript(() => { localStorage.setItem("pay3flow.exchange.source-method", "am-idbank"); localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank"); });
  await openApp(page);
  await expect(page.locator("#how-it-works")).toHaveCount(0);
  await expect(page.locator(".bankTags")).toContainText("Ameriabank");
  await expect(page.locator(".bankTags")).toContainText("Sberbank");
  await expect(page.locator(".examples article").first()).toContainText("Ameriabank");
  await expect(page.locator(".examples article").first()).toContainText("Sberbank");
  await page.getByLabel("Amount to send").fill("1000");
  await expect(page.getByTestId("complete-route").first()).toBeVisible();
  const group = page.getByTestId("route-groups");
  await expect(page.getByTestId("complete-route")).toHaveCount(101);
  await page.evaluate(() => document.fonts.ready);
  await expect(group).toHaveCSS("scrollbar-color", /rgb\(181, 245, 0\)/);
  for (const [fraction, color] of [[1 / 3, "rgb(250, 204, 21)"], [2 / 3, "rgb(249, 115, 22)"], [1, "rgb(239, 68, 68)"]] as const) {
    await expect.poll(async () => {
      await group.evaluate((node, fraction) => { node.scrollTop = (node.scrollHeight - node.clientHeight) * fraction; }, fraction);
      return group.evaluate(node => getComputedStyle(node).getPropertyValue("--route-scroll-color"));
    }).toBe(color);
  }
  await expect(page.getByTestId("complete-route").first().locator(".workflowAsset img").first()).toHaveAttribute("src", "/icons/flags/am.svg");
  await expect(page.getByTestId("complete-route").first().locator(".workflowAsset img").last()).toHaveAttribute("src", "/icons/flags/ru.svg");
  for (const popover of await page.locator(".foundVenuePopover").all()) await expect(popover).not.toContainText("Route found");
});


test("guide suspends offscreen animation while scrolling", async ({ page, isMobile }) => {
  await mockBackend(page);
  await page.addInitScript(() => { localStorage.setItem("pay3flow.exchange.source-method", "am-idbank"); localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank"); });
  await page.setViewportSize({ width: isMobile ? 393 : 1280, height: 400 });
  await openApp(page);
  await page.getByLabel("Amount to send").fill("1000");
  await expect(page.getByTestId("complete-route").first()).toBeVisible();
  await page.getByTestId("complete-route").first().locator(".routeWorkflowButton").click();
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  await guide.locator(".scene").scrollIntoViewIfNeeded();
  await expect(guide.locator(".scene")).not.toHaveClass(/paused/);
  await expect(guide.getByTestId("playback-toggle").locator("img")).toHaveAttribute("src", "/icons/ui/guide-pause.png");
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await expect(guide.locator(".scene")).toHaveClass(/paused/);
  const progress = await guide.locator(".playerTimeline button[aria-pressed=true] > span").getAttribute("style");
  // Exercise several playback ticks while its illustration is offscreen.
  await page.waitForTimeout(400);
  await expect(guide.locator(".playerTimeline button[aria-pressed=true] > span")).toHaveAttribute("style", progress!);
  await guide.locator(".scene").scrollIntoViewIfNeeded();
  await expect(guide.locator(".scene")).not.toHaveClass(/paused/);
  await expect(guide.locator(".checkpoint")).toHaveCount(0);
  await expect(page.locator(".appShell")).toHaveClass(/guideActive/);
});


test("Binance profile walkthrough opens More details, closes it and selects the route side", async ({ page }, testInfo) => {
  await mockBackend(page, { guideVenue: "binance" });
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
    localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
  });
  await openApp(page);
  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const profile = guide.getByTestId("binance-profile-card");
  await expect(profile).toHaveAttribute("data-side", "buy");
  await expect(guide.getByTestId("p2p-profile-link")).toHaveAttribute("href", "https://c2c.binance.com/ru/advertiserDetail?advertiserNo=s6b4151aab3223e9a87d491ca4256411b");
  await expect(guide.getByTestId("p2p-profile-link")).toHaveAttribute("target", "_blank");
  await guide.getByRole("button", { name: "Scene 2: Open More details", exact: true }).click();
  await expect(profile.getByTestId("binance-details")).toContainText("Trade information");
  await expect(profile.getByTestId("binance-details")).toContainText("99%");
  await expect(profile).not.toContainText("Illustrative reviews");
  const frozen = await profile.getAttribute("style");
  await page.waitForTimeout(200);
  await expect(profile).toHaveAttribute("style", frozen!);
  await guide.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("binance-details.png") });
  await guide.getByRole("button", { name: "Scene 3: Close details and find the advertisement", exact: true }).click();
  await expect(profile.getByTestId("binance-details")).toHaveCount(0);
  await expect(profile.getByTestId("binance-trade-action")).toBeInViewport();
  await expect(profile.locator(".tradeAction")).toHaveText(["Buy USDT", "Sell USDT"]);
  await guide.getByRole("button", { name: "Scene 4: Press Buy USDT", exact: true }).click();
  await expect(profile.getByTestId("binance-trade-action")).toHaveClass(/clicked/);
  await guide.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("binance-buy.png") });
  await guide.getByTestId("confirm-instruction-step").click();
  await guide.getByRole("button", { name: "Scene 4: Press Sell USDT", exact: true }).click();
  await expect(profile.getByTestId("binance-trade-action")).toHaveText("Sell USDT");
  await expect(profile.getByTestId("binance-trade-action")).toBeInViewport();
  await expect(guide.locator(".paymentPreview, .realAction")).toHaveCount(0);
});

test("BestChange walkthrough reads reviews and ends at the contextual exchanger handoff", async ({ page }, testInfo) => {
  await mockBackend(page, { guideVenue: "bestchange" });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow-locale", "ru");
    localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
    localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
  });
  await openApp(page);
  await page.locator("#exchange-amount").fill("100000");
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const card = guide.getByTestId("bestchange-exchanger-card");
  await expect(card).toContainText("ChangerBiz");
  await expect(card.getByTestId("bestchange-direction")).toHaveText("AMDUSDT");
  await guide.getByRole("button", { name: "Сцена 2: Посмотрите отзывы", exact: true }).click();
  await expect(card.getByTestId("bestchange-reviews")).toBeInViewport();
  await expect(card).toContainText("Прочитайте претензии и ответы обменника");
  await guide.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("bestchange-reviews.png") });
  await guide.getByRole("button", { name: "Сцена 3: Вернитесь к обменнику", exact: true }).click();
  await expect(card.getByTestId("bestchange-website-link")).toBeInViewport();
  await guide.getByRole("button", { name: "Сцена 4: Перейдите на сайт обменника", exact: true }).click();
  await expect(card).toHaveAttribute("data-handoff", "true");
  await expect(card.getByTestId("bestchange-website-link")).toHaveText("Перейти на сайт ChangerBiz ↗");
  await expect(guide.locator(".frameList")).toContainText("На этом переходе инструкция заканчивается");
  await expect(guide.locator(".frameList")).not.toContainText("После продажи");
  await expect(guide.locator(".paymentPreview, .realAction")).toHaveCount(0);
  await guide.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("bestchange-handoff.png") });
});

for (const provider of ["bncex", "bitcoin-center"]) {
  test(`${provider} venue walkthrough selects the fiat buy direction and correct network`, async ({ page }, testInfo) => {
    await mockBackend(page, { guideVenue: provider });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.addInitScript(() => {
      localStorage.setItem("pay3flow.exchange.source-method", "am-idbank");
      localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank");
    });
    await openApp(page);
    await page.getByLabel("Amount to send").fill("100000");
    await page.getByTestId("complete-route").first().locator(".routeAmount").click();
    const guide = page.getByTestId("route-guide");
    await guide.getByTestId("start-guide").click();
    const card = guide.getByTestId(`${provider}-swap-card`);
    await expect(card).toBeVisible();
    await expect(card.getByTestId(`${provider}-send-amount`)).toHaveText("100,000");
    await expect(card.locator('[data-side="send"]')).toContainText("AMD");
    await expect(card.locator('[data-side="receive"]')).toContainText("USDT");
    if (provider === "bitcoin-center") await expect(card.getByTestId(`${provider}-receive-network`)).toHaveText("Solana");
    else await expect(card.locator(".buySell .active")).toHaveText("Buy");
    await guide.getByRole("button", { name: "Scene 3: Check the amount you receive", exact: true }).click();
    await expect(card.locator('[data-side="receive"]')).toHaveClass(/amountHighlight/);
    await expect(card.getByTestId(`${provider}-receive-amount`)).toHaveText("—");
    const action = provider === "bncex" ? "Continue" : "Sign in to continue";
    await guide.getByRole("button", { name: `Scene 4: Press ${action}`, exact: true }).click();
    await expect(card.getByTestId(`${provider}-exchange-action`)).toHaveText(action);
    await expect(card.getByTestId(`${provider}-exchange-action`)).toHaveClass(/clicked/);
    const size = await card.evaluate(element => ({ width: element.clientWidth, content: element.scrollWidth }));
    expect(size.content).toBeLessThanOrEqual(size.width + 1);
    const receive = await card.locator('[data-side="receive"]').boundingBox();
    const actionBounds = await card.getByTestId(`${provider}-exchange-action`).boundingBox();
    expect(receive!.y + receive!.height).toBeLessThan(actionBounds!.y);
    await expect(guide.locator(".paymentPreview, .realAction")).toHaveCount(0);
    await guide.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath(`${provider}.png`) });
  });
}

for (const provider of ["symbiosis", "cow-swap", "dzengi"]) {
  test(`${provider} venue walkthrough preserves crypto amounts and source and destination context`, async ({ page }, testInfo) => {
    await mockBackend(page, { guideVenue: provider });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await openApp(page);
    const source = await chooseCrypto(page, "sending", "ETH Base");
    await source.getByRole("option", { name: /Base.*ETH/ }).click();
    const target = await chooseCrypto(page, "recipient", "USDT Polygon");
    await target.getByRole("option", { name: /Polygon.*USDT/ }).click();
    await page.getByLabel("Amount to send").fill("0.03");
    await page.getByTestId("start-search").click();
    await page.getByTestId("complete-route").first().locator(".routeAmount").click();
    const guide = page.getByTestId("route-guide");
    await guide.getByTestId("start-guide").click();
    const card = guide.getByTestId(`${provider}-swap-card`);
    await expect(card).toBeVisible();
    await expect(card.getByTestId(`${provider}-send-amount`)).toHaveText("0.03");
    await expect(card.getByTestId(`${provider}-receive-amount`)).toHaveText("80.1");
    if (provider !== "dzengi") {
      await expect(card.getByTestId(`${provider}-send-network`)).toHaveText("Base");
      await expect(card.getByTestId(`${provider}-receive-network`)).toHaveText("Polygon");
    } else {
      await expect(card.locator('[data-side="send"]')).toContainText("ETH");
      await expect(card.locator('[data-side="receive"]')).toContainText("USDT");
    }
    await guide.getByRole("button", { name: "Scene 3: Check the amount you receive", exact: true }).click();
    await expect(card.locator('[data-side="receive"]')).toHaveClass(/amountHighlight/);
    if (provider === "dzengi") await expect(guide.locator(".frameList")).toContainText("indicative quote");
    const action = provider === "symbiosis" ? "Swap" : provider === "cow-swap" ? "Review swap" : "Convert";
    await guide.getByRole("button", { name: `Scene 4: Press ${action}`, exact: true }).click();
    await expect(card.getByTestId(`${provider}-exchange-action`)).toHaveText(action);
    await expect(card.getByTestId(`${provider}-exchange-action`)).toHaveClass(/clicked/);
    await expect(guide.locator(".realAction")).toHaveCount(0);
    await expect(guide.getByTestId("route-wallet-execution")).toHaveCount(0);
    await expect(guide.locator(".walletButton")).toHaveCount(0);
    const size = await card.evaluate(element => ({ width: element.clientWidth, content: element.scrollWidth }));
    expect(size.content).toBeLessThanOrEqual(size.width + 1);
    await guide.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath(`${provider}.png`) });
  });
}

for (const provider of ["binance", "bybit", "mexc", "whitebird", "bitget"]) {
  test(`spot demonstration on ${provider} follows buy and sell sides and player controls`, async ({ page }, testInfo) => {
    await mockBackend(page, { spotCycle: true, spotVenue: provider });
    await openApp(page);
    for (const side of ["sending", "recipient"] as const) {
      const picker = await chooseCrypto(page, side, "USDT ERC20");
      await picker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDT/ }).click();
    }
    await page.getByLabel("Amount to send").fill("100");
    await page.getByTestId("start-search").click();
    await page.locator(".routeCard").first().locator(".workflow").click();
    const guide = page.getByTestId("route-guide");
    await guide.getByTestId("start-guide").click();
    const terminal = guide.getByTestId("spot-terminal");
    await expect(terminal).toHaveAttribute("data-provider", provider);
    await expect(terminal).toHaveAttribute("data-order-side", "buy");
    await expect(guide.getByTestId("spot-order-action")).toHaveText("Buy ETH");
    await guide.getByRole("button", { name: "Scene 3: Choose the order type and amount", exact: true }).click();
    await expect(terminal).toHaveAttribute("data-frame-kind", "review");
    await expect(guide.getByTestId("spot-input-amount")).toContainText("100");
    await expect(guide.getByTestId("spot-input-amount")).toContainText("USDT");
    await expect(guide.getByTestId("playback-toggle")).toHaveAttribute("aria-pressed", "false");
    await guide.getByRole("button", { name: "Next scene", exact: true }).click();
    await expect(terminal).toHaveAttribute("data-frame-kind", "act");
    await guide.getByRole("button", { name: "Previous scene", exact: true }).click();
    await expect(terminal).toHaveAttribute("data-frame-kind", "review");
    await guide.getByTestId("center-playback").click();
    await expect(guide.getByTestId("playback-toggle")).toHaveAttribute("aria-pressed", "true");
    await guide.getByTestId("center-playback").click();
    await expect(guide.getByTestId("playback-toggle")).toHaveAttribute("aria-pressed", "false");
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth);
    expect(overflow).toBeLessThanOrEqual(1);
    await guide.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath(`${provider}-spot.png`) });
    await guide.getByRole("button", { name: "Scene 5: Check Trade History and balance", exact: true }).click();
    await expect(guide.getByTestId("spot-order-history")).toHaveClass(/focus/);
    await guide.getByTestId("confirm-instruction-step").click();
    await expect(guide.getByTestId("spot-market-pair")).toContainText("BTC/ETH");
    await guide.getByTestId("confirm-instruction-step").click();
    await expect(terminal).toHaveAttribute("data-order-side", "sell");
    await expect(guide.getByTestId("spot-order-action")).toHaveText("Sell BTC");
    await expect(guide.getByTestId("spot-order-form").locator(".sideTabs .selected")).toHaveText("Sell");
  });
}

test("Russian spot demonstration stays readable at 320px and respects reduced motion", async ({ page }, testInfo) => {
  await mockBackend(page, { spotCycle: true, spotVenue: "bitget" });
  await openApp(page);
  for (const side of ["sending", "recipient"] as const) {
    const picker = await chooseCrypto(page, side, "USDT ERC20");
    await picker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDT/ }).click();
  }
  await page.getByLabel("Amount to send").fill("100");
  await page.getByTestId("start-search").click();
  await page.locator(".routeCard").first().locator(".workflow").click();
  await page.evaluate(() => {
    localStorage.setItem("pay3flow-locale", "ru");
    history.replaceState(null, "", location.href.replace(/([?&])lang=en\b/, "$1lang=ru"));
  });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 320, height: 900 });
  await page.reload();
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  await expect(guide.getByTestId("playback-toggle")).toHaveAttribute("aria-pressed", "false");
  await expect(guide.getByTestId("spot-terminal").locator(".cursor")).toBeHidden();
  await expect(guide.getByTestId("spot-order-action")).toHaveText("Купить ETH");
  await guide.getByRole("button", { name: "Сцена 3: Выберите тип ордера и сумму", exact: true }).click();
  await expect(guide.getByTestId("spot-input-amount")).toContainText("100");
  expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
  const field = await guide.getByTestId("spot-input-amount").boundingBox();
  const stage = await guide.getByTestId("instruction-scene").boundingBox();
  expect(field!.x + field!.width).toBeLessThanOrEqual(stage!.x + stage!.width);
  await guide.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("spot-russian-320.png") });
  const AxeBuilder = (await import("@axe-core/playwright")).default;
  const accessibility = await new AxeBuilder({ page }).include('[data-testid="route-guide"]').analyze();
  expect(accessibility.violations.map(item => ({ id: item.id, nodes: item.nodes.map(node => node.target) }))).toEqual([]);
});


test("Cifra Tradernet walkthrough prepares funds and changes from a buy to a sell order", async ({ page }, testInfo) => {
  await mockBackend(page, { cifra: true });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.addInitScript(() => localStorage.setItem("pay3flow.exchange.target-method", "ru-alfabank"));
  await openApp(page);
  await page.getByRole("button", { name: "Select sending currency: AMD" }).click();
  await page.getByRole("dialog", { name: "Choose currency" }).getByRole("option", { name: /^USD / }).click();
  await page.getByLabel("Amount to send").fill("100000");
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const card = guide.getByTestId("cifra-tradernet-card");
  await expect(card).toBeVisible();
  await expect(card).toHaveAttribute("data-side", "buy");
  await expect(guide.locator(".playerTimeline button")).toHaveCount(6);
  await expect(guide.getByTestId("cifra-terminal-link")).toHaveAttribute("href", "https://tradernet.by/terminal");
  await expect(guide.getByRole("link", { name: "Cifra official video guides" })).toHaveAttribute("href", "https://cifra.by/knowledge-base");
  await expect(guide.getByTestId("p2p-profile-link")).toHaveCount(0);
  await guide.getByRole("button", { name: "Scene 2: Prepare the trading balance", exact: true }).click();
  await expect(card).toContainText("USD");
  await expect(guide.locator(".frameList")).toContainText("advanced verification");
  await guide.getByRole("button", { name: "Scene 3: Find USDT-USD.IMEX in Trade", exact: true }).click();
  await expect(card.getByTestId("cifra-ticker")).toHaveText("USDT-USD.IMEX");
  await guide.getByRole("button", { name: "Scene 4: Set up a market order", exact: true }).click();
  await expect(card.getByTestId("cifra-order-amount")).toContainText("100,000");
  await guide.getByRole("button", { name: "Scene 5: Review and submit the order", exact: true }).click();
  await expect(card.getByTestId("cifra-order-action")).toHaveText("Place a buy order");
  const actionBounds = await card.getByTestId("cifra-order-action").boundingBox(), bounds = await card.boundingBox();
  expect(actionBounds!.y + actionBounds!.height + 6).toBeLessThanOrEqual(bounds!.y + bounds!.height);
  await guide.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("cifra-buy.png") });
  await guide.getByRole("button", { name: "Scene 6: Check execution and the balance", exact: true }).click();
  await expect(card.locator(".balance")).toContainText("USDT");
  await guide.getByTestId("confirm-instruction-step").click();
  await expect(card).toHaveAttribute("data-side", "sell");
  await guide.getByRole("button", { name: "Scene 3: Find USDT-RUB.IMEX in Trade", exact: true }).click();
  await expect(card.getByTestId("cifra-ticker")).toHaveText("USDT-RUB.IMEX");
  await guide.getByRole("button", { name: "Scene 5: Review and submit the order", exact: true }).click();
  await expect(card.getByTestId("cifra-order-action")).toHaveText("Place a sell order");
  await guide.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("cifra-sell.png") });
});

test("Cifra crypto walkthrough preserves the market direction and localizes the real controls", async ({ page }, testInfo) => {
  await mockBackend(page, { guideVenue: "cifra-broker" });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openApp(page);
  const source = await chooseCrypto(page, "sending", "ETH Base");
  await source.getByRole("option", { name: /Base.*ETH/ }).click();
  const target = await chooseCrypto(page, "recipient", "USDT Polygon");
  await target.getByRole("option", { name: /Polygon.*USDT/ }).click();
  await page.getByLabel("Amount to send").fill("0.03");
  await page.getByTestId("start-search").click();
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();
  await selectLanguage(page, "ru");
  const guide = page.getByTestId("route-guide");
  await guide.getByTestId("start-guide").click();
  const card = guide.getByTestId("cifra-tradernet-card");
  await expect(card).toHaveAttribute("data-side", "sell");
  await expect(card).toHaveClass(/paused/);
  await guide.getByRole("button", { name: "Сцена 2: Подготовьте торговый баланс", exact: true }).click();
  await expect(card).toContainText("Base");
  await guide.getByRole("button", { name: "Сцена 4: Настройте рыночный приказ", exact: true }).click();
  await expect(card.getByTestId("cifra-ticker")).toHaveText("ETH-USDT.IMEX");
  await expect(card.getByTestId("cifra-order-amount")).toContainText("0.03");
  await expect(card.getByTestId("cifra-order-action")).toHaveText("Выставить приказ на продажу");
  const actionBounds = await card.getByTestId("cifra-order-action").boundingBox(), bounds = await card.boundingBox();
  expect(actionBounds!.y + actionBounds!.height + 6).toBeLessThanOrEqual(bounds!.y + bounds!.height);
  await guide.getByTestId("instruction-scene").screenshot({ style: ".sceneOverlay { visibility: hidden; }", path: testInfo.outputPath("cifra-crypto-ru.png") });
});

test("compact selection search filters currencies and banks and supports keyboard selection", async ({ page }, testInfo) => {
  await mockBackend(page);
  await openApp(page);
  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const picker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await picker.screenshot({ path: testInfo.outputPath("compact-currency-search.png") });
  const input = picker.getByRole("textbox", { name: "Currencies and digital assets" });
  await input.fill("AMD");
  await expect(picker.getByRole("option")).toHaveCount(1);
  await input.press("ArrowDown");
  await expect(picker.getByRole("option")).toBeFocused();
  await page.keyboard.press("Enter");
  await picker.getByLabel("Search banks and payment methods").fill("IDBank");
  await expect(picker.getByRole("option")).toHaveCount(1);
  await picker.screenshot({ path: testInfo.outputPath("compact-bank-search.png") });
  await picker.getByLabel("Search banks and payment methods").press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(picker).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Select sending bank: IDBank" })).toBeVisible();
  await page.getByRole("button", { name: "Select sending currency: AMD" }).click();
  const currencies = page.getByRole("dialog", { name: "Choose currency" });
  await currencies.getByRole("textbox", { name: "Currencies and digital assets" }).fill("USD");
  await expect(currencies.getByRole("option")).toHaveCount(1);
  await currencies.getByRole("textbox", { name: "Currencies and digital assets" }).press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("button", { name: "Select sending currency: USD" })).toBeVisible();
});

test("compact selection search filters a network and handles empty results and Escape", async ({ page }, testInfo) => {
  await mockBackend(page);
  await openApp(page);
  const picker = await chooseCrypto(page, "sending", "USDT ERC20");
  await picker.getByRole("option", { name: /Ethereum.*USDT/ }).click();
  await page.getByRole("button", { name: /Select sending network: Ethereum/ }).click();
  const networks = page.getByRole("dialog", { name: "Choose network" });
  await networks.getByLabel("Blockchains").fill("TRC-20");
  await expect(networks.getByRole("option")).toHaveCount(1);
  await expect(networks.getByRole("option")).toContainText("TRON");
  await networks.screenshot({ path: testInfo.outputPath("compact-network-search.png") });
  await networks.getByLabel("Blockchains").fill("unavailable-network");
  await expect(networks.getByRole("option")).toHaveCount(0);
  await expect(networks.getByText("No compatible blockchains found")).toBeVisible();
  await networks.getByLabel("Blockchains").press("Escape");
  await expect(networks).toHaveCount(0);
  await expect(page.getByRole("button", { name: /Select sending network: Ethereum/ })).toBeVisible();
});
