import { expect, test, type Locator, type Page } from "@playwright/test";

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

async function openApp(page: Page, waitForIntro = true) {
  await page.goto("/");
  await expect
    .poll(() => page.locator(".appShell").evaluate((element) => getComputedStyle(element, "::before").backgroundImage))
    .not.toBe("none");
  if (waitForIntro) await expect(page.locator(".introOverlay")).toHaveCount(0, { timeout: 6000 });
}

async function expectPeriodButtonBesidePair(chart: Locator) {
  await expect(chart.locator(".pairCurrency")).toHaveCount(2);
  await expect(chart.locator(".pairCurrency img, .pairCurrency .pairFlag")).toHaveCount(2);
  const pair = await chart.locator(".pair").boundingBox();
  const button = await chart.getByRole("button", { name: "Chart time range" }).boundingBox();
  expect(pair).not.toBeNull();
  expect(button).not.toBeNull();
  expect(button!.x).toBeGreaterThanOrEqual(pair!.x + pair!.width - 1);
  expect(Math.abs(button!.y + button!.height / 2 - pair!.y - pair!.height / 2)).toBeLessThan(5);
  await expect(chart.locator(".periodButton img")).toHaveCSS("filter", "none");
}

test("system theme follows the browser until the user chooses a theme", async ({ page, isMobile }) => {
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

  if (isMobile) await page.locator(".menuToggle").click();
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
  await expect(page.locator(".introOverlay")).toHaveCount(0, { timeout: 6000 });
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
  await page.route("http://localhost:8080/api/p2p/routes**", async (route) => {
    await holdSearch;
    await route.abort();
  });
  await page.routeWebSocket(/\/ws\/p2p\/routes$/, (socket) => {
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

test("search activity range menu filters and remembers the selected period", async ({ page, isMobile }) => {
  await mockBackend(page);
  await openApp(page);
  await page.getByRole("button", { name: isMobile ? "Open search activity graph" : "Show search activity" }).click();
  const chart = isMobile ? page.getByRole("dialog", { name: "Searches for this exchange" }).getByTestId("search-activity") : page.getByTestId("search-activity");
  await expect(chart.locator(".activityStats strong")).toHaveText("28");
  const rangeButton = chart.getByRole("button", { name: "Chart time range" });
  await expect(rangeButton.locator("img")).toHaveAttribute("src", "/icons/ui/chart-period.png");
  await rangeButton.click();
  const menu = isMobile ? page.getByRole("dialog", { name: "Chart time range" }) : chart.getByRole("menu", { name: "Chart time range" });
  const selected = isMobile ? menu.locator('.periodOptions button[aria-pressed="true"]') : menu.getByRole("menuitemradio", { checked: true });
  await expect(isMobile ? menu.locator(".periodOptions button") : menu.getByRole("menuitemradio")).toHaveCount(8);
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
  await (isMobile ? menu.getByRole("button", { name: "1 hour" }) : menu.getByRole("menuitemradio", { name: "1 hour" })).click();
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
  await expect(isMobile ? page.getByRole("dialog", { name: "Chart time range" }).getByRole("button", { name: "1 hour", pressed: true }) : restoredChart.getByRole("menuitemradio", { name: "1 hour", checked: true })).toHaveCSS("background-color", accent);
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
  await expectFullViewport(".actionsBackdrop.menuOpen");
  await expect(page.locator(".actions a, .actions button")).toHaveCount(5);
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Route refresh settings" }).click();
  await expectFullViewport(".settingsBackdrop");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Choose exchanges" }).click();
  await expectFullViewport(".settingsBackdrop");
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Open search activity graph" }).click();
  const graph = page.getByRole("dialog", { name: "Searches for this exchange" });
  await expect(graph.locator(".sheetHandle")).toBeVisible();
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

  const telegram = page.getByRole("link", { name: "Open Pay3Flow Telegram channel" });
  await expect(telegram).toHaveAttribute("href", "https://t.me/+-lq4m5E_aT4xM2Y6");
  await expect(telegram.locator("img")).toHaveJSProperty("naturalWidth", 1024);

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

test("route instructions lock the page until closed", async ({ page }) => {
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

  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expect(instructions).toBeVisible();
  await expect(page.locator("body")).toHaveCSS("position", "fixed");
  await instructions.getByRole("button", { name: "Close instructions", exact: true }).click();
  await expect(instructions).toBeHidden();
  await expect(page.locator("body")).not.toHaveCSS("position", "fixed");
});

test("headline introduces the bridge and resizing does not replay the entrance", async ({ page }) => {
  await mockBackend(page);
  await openApp(page, false);
  const intro = page.locator(".introOverlay");
  const workspace = page.locator(".workspace");
  await expect(intro).toBeVisible();
  await expect(workspace).toBeHidden();
  await expect(intro.locator(".introWordMove")).toHaveCSS("opacity", "1");
  await expect(intro.locator(".introWordMoney")).toHaveCSS("opacity", "1");
  await expect(intro.locator(".introWordKeep")).toHaveCSS("opacity", "1");
  await expect(intro.locator(".introWordMore")).toHaveCSS("opacity", "1");
  await expect(intro.locator(".introLastPunctuation")).toHaveCSS("opacity", "1");
  await expect(intro).toHaveCount(0, { timeout: 6000 });
  await expect(workspace).toBeVisible();
  await expect(page.locator(".hero h1")).toContainText("Move money. Keep more.");
  await page.waitForTimeout(800);

  const animationTime = () => workspace.evaluate((element) => {
    const animation = element.getAnimations()[0];
    return Number(animation?.currentTime ?? 0);
  });
  const completedAt = await animationTime();
  expect(completedAt).toBeGreaterThanOrEqual(600);

  const viewport = page.viewportSize();
  expect(viewport).not.toBeNull();
  await page.setViewportSize({ width: viewport!.width - 40, height: viewport!.height });
  await page.waitForTimeout(250);

  expect(await animationTime()).toBeGreaterThanOrEqual(600);
  await expect(intro).toHaveCount(0);
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

async function mockBackend(page: Page, options: { includeNewProviders?: boolean; routeCount?: number } = {}) {
  await page.route("http://localhost:8080/api/**", async (route) => {
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
        currency("USD", "US dollar", "$", "#168451"), currency("BYN", "Belarusian ruble", "Br", "#006b3f"),
        { ...method("global-usd-cash", "Cash USD", "GLOBAL", "USD", "", "cash", true), kind: "cash", initials: "$", p2p_query: "Cash" },
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
        wallet("USDT", "Tether"), wallet("USDC", "USD Coin"), wallet("BTC", "Bitcoin"), wallet("ETH", "Ethereum"),
      ];
      return json({ items, total: items.length, limit: 100, offset: 0 });
    }
    if (url.pathname === "/api/networks") {
      return json([
        { id: "ethereum", name: "Ethereum (ERC-20)", currencies: ["ETH", "USDT", "USDC"] },
        { id: "base", name: "Base", currencies: ["ETH", "USDC"] },
        { id: "tron", name: "TRON (TRC-20)", currencies: ["TRX", "USDT"] },
        { id: "ton", name: "TON", currencies: ["TON", "USDT"] },
        { id: "bitcoin", name: "Bitcoin", currencies: ["BTC"] },
        { id: "near", name: "NEAR", currencies: ["BTC", "USDT"] },
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
      if (options.includeNewProviders) {
        providers.push(
          { slug: "bitcoin-center", name: "Bitcoin Center Buy", side: "buy", source_url: "https://www.bitcoincenter.am", currencies: ["AMD"], banks: ["Bank Transfer"], searchable: true, search_mode: "selectable" },
          { slug: "bncex", name: "bncex Buy", side: "buy", source_url: "https://www.bncex.com/en", currencies: ["AMD"], banks: [], searchable: true, search_mode: "selectable" },
          { slug: "skylabs", name: "SkyLabs Buy", side: "buy", source_url: "https://skylabs.world", currencies: ["AMD"], banks: [], searchable: true, search_mode: "selectable" },
          { slug: "symbiosis", name: "Symbiosis Buy", side: "buy", source_url: "https://api.symbiosis.finance", currencies: [], banks: [], searchable: true, search_mode: "selectable" },
        );
      }
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
        price: "1",
        available_asset: "1000000",
        min_fiat: "1000",
        max_fiat: "10000000",
        payment_methods: ["Bank transfer"],
        pay_time_limit_minutes: 15,
        advertiser: {
          id: source === "bybit" ? `masked-${adId}` : null,
          nickname: `${source}-merchant`,
          user_type: "merchant" as string | null,
          is_merchant: true,
          is_verified: true,
          completed_orders_30d: 300 as number | null,
          completion_rate_30d: 0.99 as number | null,
        },
        source_url: `https://example.com/${adId}`,
      });
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
        expect(url.searchParams.get("source_network")).toBe("tron");
        expect(url.searchParams.get("target_network")).toBe("ton");
        return json({ error: "No live bridge provider is configured for USDT: TRON (TRC-20) → TON" }, 400);
      }
      if (url.searchParams.get("source_fiat") === "USDT") {
        expect(url.searchParams.get("source_network")).toBe("ethereum");
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
            entry_network: "ethereum",
            source_network: "ethereum",
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
        expect(url.searchParams.get("target_network")).toBe("ton");
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
            target_network: "ton",
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
            market_path: {
              venue: "binance",
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
        const venue = index % 2 === 0 ? "binance" : "bybit";
        const exitVenue = index === 2 ? "bybit" : venue;
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
          services: [{ id: venue === "binance" ? "00000000-0000-4000-8000-000000000201" : "00000000-0000-4000-8000-000000000202", slug: venue, display_name: venue === "binance" ? "Binance" : "Bybit", executions_total: best ? 12400 : 6200, likes_total: best ? 1800 : 850, dislikes_total: best ? 74 : 40 }],
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
  await openApp(page);

  const backgroundPattern = await expect
    .poll(() => page.locator(".appShell").evaluate((element) => getComputedStyle(element, "::before").backgroundImage))
    .not.toBe("none")
    .then(() =>
      page.locator(".appShell").evaluate((element) => getComputedStyle(element, "::before").backgroundImage),
    );
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
  const languageToggle = page.locator(".languageToggle");
  const toggleLanguage = async () => {
    if (isMobile) await page.locator(".menuToggle").click();
    await languageToggle.click();
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
    "AMD → USDT (Binance) → RUB (Binance)",
  );
  const routeGroups = page.getByTestId("route-groups");
  const scrollMetrics = await routeGroups.evaluate((element) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
    overflowY: getComputedStyle(element).overflowY,
  }));
  expect(scrollMetrics.overflowY).toBe("auto");
  expect(scrollMetrics.scrollHeight).toBeGreaterThan(scrollMetrics.clientHeight);
  await expect(routeGroups).toHaveCSS("scrollbar-width", "none");
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
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expect(instructions).toBeVisible();
  await expect(instructions.getByRole("list", { name: "Exchange steps" })).toBeVisible();
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await expect(instructions.getByText("Buy USDT for 100,000 AMD")).toBeVisible();
  await expect(instructions.getByText("Before creating the order, compare the nickname and advertisement ID.")).toHaveCount(2);
  await expect(instructions.getByText("Release the asset only after you personally see the payment in your bank or payment account.")).toBeVisible();
  const offerLinks = instructions.getByRole("link", { name: /Open Binance P2P and find binance-merchant/ });
  await expect(offerLinks.first()).toHaveAttribute(
    "href",
    "https://example.com/entry-1",
  );
  await expect(offerLinks).toHaveCount(2);
  await instructions.getByRole("button", { name: "Close instructions", exact: true }).click();
  await expect(instructions).toBeHidden();
  await page.getByTestId("start-search").click();
  await expect(instructions).toBeVisible();
  await instructions.getByRole("button", { name: "Close instructions", exact: true }).click();

  let finishRefresh!: () => void;
  const heldRefresh = new Promise<void>((resolve) => { finishRefresh = resolve; });
  await page.route("http://localhost:8080/api/p2p/routes**", async (route) => {
    await heldRefresh;
    await route.fallback();
  });
  await page.getByRole("button", { name: "Refresh routes now" }).click();
  await expect(page.getByTestId("start-search")).toHaveText("Go ↗");
  await expect(page.getByTestId("start-search")).toBeDisabled();
  await expect(page.getByTestId("start-search")).toHaveCSS("background-color", "rgb(243, 246, 240)");
  finishRefresh();
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
  await expect(page.getByText("Calculating from live quotes")).toBeVisible();
  await page.getByTestId("start-search").click();
  await expect(sendAmount).toHaveValue("42269");
  await expect(receiveAmount).toHaveValue("10000");
  await expect(page.getByTestId("complete-route")).toHaveCount(1);

  const quotedSendAmount = await sendAmount.inputValue();
  await sendAmount.fill(quotedSendAmount);
  await page.getByTestId("start-search").click();
  await expect(receiveAmount).toHaveValue("10000");
});

test("RUB to RUB bank routes require checking the order payment method", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByLabel("Search banks and payment methods").fill("Sberbank");
  await sourcePicker.getByRole("option", { name: /Sberbank/ }).click();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();

  await page.getByLabel("Amount to send").fill("10000");
  await page.getByTestId("start-search").click();
  const route = page.getByTestId("complete-route").first();
  await expect(route).toBeVisible();
  await route.locator(".routeAmount").click();

  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await expect(instructions.getByText("Buy USDT for 10,000 RUB")).toBeVisible();
  await expect(instructions.getByText("Check the current rate, order limits, and payment method on Binance.")).toBeVisible();
  await expect(instructions.getByText("Release the asset only after you personally see the payment in your bank or payment account.")).toBeVisible();
});

test("AMD cycle keeps the best route visible when profit is unconfirmed", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("IDBank");
  await targetPicker.getByRole("option", { name: /IDBank/ }).click();

  await page.getByLabel("Amount to send").fill("10000");
  await page.getByTestId("start-search").click();

  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await expect(page.getByTestId("complete-route").first()).toContainText("Est. -100 AMD (-1.00%)");
  await expect(page.getByTestId("no-profitable-routes")).toHaveText(
    "No confirmed profitable route right now; showing the best available cycles.",
  );
});

test("cross-venue instructions include a numbered transfer step", async ({ page }) => {
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

  await page.getByTestId("complete-route").nth(2).locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expectNumberedTimeline(instructions, ["1", "2", "3"]);
  await expect(instructions.getByRole("heading", { name: "Transfer USDT to Bybit" })).toBeVisible();
  await expect(instructions.getByText("Choose the exact Ethereum (ERC-20) network on both platforms", { exact: false })).toBeVisible();
  await expect(instructions.getByText("Wait until Bybit shows the deposit as received before continuing.")).toBeVisible();
});

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

test("search venues announce providers reported by route statuses", async ({ page }) => {
  await mockBackend(page);

  let sendFirstRoute: (() => void) | undefined;
  let sendSecondRoute: (() => void) | undefined;
  let finishSearch: (() => void) | undefined;
  let socketConnections = 0;
  await page.routeWebSocket(/\/ws\/p2p\/routes$/, (socket) => {
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
        price: "1",
        available_asset: "1000000",
        min_fiat: "1000",
        max_fiat: "10000000",
        payment_methods: ["Bank transfer"],
        pay_time_limit_minutes: 15,
        advertiser: {
          id: `masked-${adId}`,
          nickname: `${source}-merchant`,
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
            { source: "bncex", ok: true, latency_ms: 18, offers_found: 1, error: null },
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
  sendFirstRoute?.();
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await expect(page.getByTestId("complete-route").first()).toHaveClass(/selected/);
  const foundVenues = panelTop.locator(".resultSummary").getByTestId("found-venue");
  await expect(foundVenues).toHaveCount(6);
  await expect(foundVenues.first()).toHaveAttribute("title", "Found on Bybit");
  await expect(foundVenues.nth(1)).toHaveAttribute("title", "Found on SkyLabs");
  await expect(foundVenues.nth(2)).toHaveAttribute("title", "Found on Bncex");
  await expect(foundVenues.nth(3)).toHaveAttribute("title", "Found on Bitcoin Center");
  await expect(foundVenues.nth(4)).toHaveAttribute("title", "No route on Binance");
  await expect(foundVenues.nth(5)).toHaveAttribute("title", "No route on OKX");
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
  await expect(foundVenues).toHaveCount(7);
  for (const title of ["Whitebird", "Bybit", "SkyLabs", "Bncex", "Bitcoin Center"]) {
    await expect(panelTop.locator(`[data-testid="found-venue"][title="Found on ${title}"]`)).toHaveCount(1);
  }
  for (const title of ["Binance", "OKX"]) {
    await expect(panelTop.locator(`[data-testid="found-venue"][title="No route on ${title}"]`)).toHaveCount(1);
  }
  await expect(searchingVenues).toHaveCount(4);

  await page.getByTestId("complete-route").nth(1).click();
  await expect(page.getByTestId("complete-route").nth(1)).toHaveClass(/selected/);
  finishSearch?.();
  await expect(refreshButton).toBeEnabled();
  await expect(searchingVenues).toHaveCount(0);
  await expect(page.getByTestId("complete-route").nth(1)).toHaveClass(/selected/);
});

test("reordered progressive snapshots do not restart card rendering at 100", async ({ page }) => {
  await mockBackend(page);

  let sendFirstSnapshot: (() => void) | undefined;
  let sendReorderedSnapshot: (() => void) | undefined;
  let finishSearch: (() => void) | undefined;
  await page.routeWebSocket(/\/ws\/p2p\/routes$/, (socket) => {
    socket.onMessage(() => {
      const offer = (adId: string) => ({
        source: "bybit",
        ad_id: adId,
        fiat: "AMD",
        asset: "USDT",
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

test("currency control only lists currencies supported by the selected payment method", async ({ page, isMobile }) => {
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
  const languageToggle = page.locator(".languageToggle");
  for (let index = 0; index < 3; index += 1) {
    if (isMobile) await page.locator(".menuToggle").click();
    await languageToggle.click();
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
  await expect(methodPicker.getByRole("option", { name: /Cash USD/ })).toBeVisible();
  await methodPicker.getByLabel("Search banks and payment methods").fill("Ameriabank");
  await expect(methodPicker.getByRole("option", { name: /^Ameriabank Bank transfer · USD/ })).toHaveCount(1);
  await methodPicker.getByLabel("Search banks and payment methods").fill("");
  await methodPicker.getByRole("option", { name: /Cash USD/ }).click();
  await expect(page.getByRole("button", { name: "Select sending payment method: Cash USD" })).toBeVisible();
  await expect(page.locator(".moneyPanelSource .currencyHint")).toHaveCount(0);

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetMethodPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
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
  await expect(cards.nth(0).locator(".workflow")).toHaveAttribute(
    "aria-label",
    "USDT Tether · ethereum → USDC USD Coin · ethereum (NEAR 1Click)",
  );
  await expect(cards.nth(1).locator(".workflow")).toHaveAttribute(
    "aria-label",
    "USDT Tether · ethereum → USDC USD Coin · ethereum (CoW Protocol Live)",
  );
  await cards.nth(0).locator(".routeAmount").click();
  let instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expect(instructions.getByTestId("route-wallet-execution")).toBeVisible();
  await expect(instructions.getByRole("button", { name: "Connect ethereum wallet" })).toBeVisible();
  await instructions.getByRole("button", { name: "Close instructions", exact: true }).click();
  await cards.nth(1).locator(".routeAmount").click();
  instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expect(instructions.getByTestId("route-wallet-execution")).toBeVisible();
});

test("ID Pay provides a direct AMD to RUB route with its API name", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByLabel("Amount to send").fill("42269");
  await page.getByTestId("start-search").click();

  const route = page.getByTestId("complete-route");
  await expect(route).toHaveCount(1);
  await expect(route).toContainText("10,000 RUB");
  await expect(route).not.toContainText("Quote by");
  await expect(route.locator(".workflow")).toHaveAttribute(
    "aria-label",
    "AMD → RUB (ID Pay Live)",
  );

  await route.locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expect(instructions.getByRole("heading", { name: "Transfer AMD to RUB via ID Pay Live" })).toBeVisible();
  await expect(instructions.getByRole("link", { name: "Open ID Pay Live exchange" })).toHaveAttribute("href", "https://id-pay.ru/");
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
  await expect(page.locator("#exchange-output")).toHaveText("0.00032478");
  await expect(route.locator(".workflow")).toHaveAttribute(
    "aria-label",
    "AMD → USDT Tether · optimism (Bybit) → BTC Bitcoin · near (NEAR 1Click)",
  );
  await route.locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await expect(instructions.getByText("Buy USDT for 10,000 AMD")).toBeVisible();
  await expect(instructions.getByRole("heading", { name: "Swap USDT for BTC via NEAR 1Click" })).toBeVisible();
  await expect(instructions.getByRole("button", { name: "Connect optimism wallet" })).toBeVisible();
});

test("direct Whitebird exchange uses provider wording and local venue icons", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  const picker = await chooseCrypto(page, "sending", "USDC ERC20");
  await picker.getByRole("option", { name: /Ethereum \(ERC-20\).*USDC/ }).click();

  await page.getByLabel("Amount to send").fill("100");
  await page.getByTestId("start-search").click();
  const route = page.getByTestId("complete-route");
  await expect(route).toHaveCount(1);
  await expect(route.locator(".workflow")).toHaveAttribute(
    "aria-label",
    "USDC USD Coin · Ethereum (ERC-20) (Whitebird) → RUB",
  );
  await expect(route.locator(".workflowVenueIcon img")).toHaveAttribute(
    "src",
    "/icons/venues/whitebird.png",
  );

  await route.locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expect(instructions.getByText("Direct exchange on Whitebird", { exact: true })).toBeVisible();
  await expect(instructions.getByRole("link", { name: "Open Whitebird exchange" })).toHaveAttribute(
    "href",
    "https://whitebird.io/",
  );
  await expect(instructions.getByText("Find by nickname", { exact: true })).toHaveCount(0);
  await expect(instructions.locator(".adHint")).toHaveCount(0);
  await expect(instructions.locator(".counterpartyAvatar .avatarLogo")).toHaveAttribute(
    "src",
    "/icons/venues/whitebird.png",
  );
});

test("Armenian bank picker uses the downloaded local icons", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const picker = page.getByRole("dialog", { name: "Choose where you pay from" });
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

  await expect(page.getByTestId("complete-route")).toContainText(
    "ETH Ether · Base → USDT Tether · TON (Binance)",
  );

  await page.getByTestId("complete-route").locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expectNumberedTimeline(instructions, ["1"]);
  await expect(instructions.getByRole("heading", { name: "Convert ETH to USDT" })).toBeVisible();
  await expect(instructions.getByText("First check that the pair changes ETH into USDT.")).toBeVisible();
});

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

  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await expect(instructions.getByRole("heading", { name: "Convert BTC to USDC" })).toBeVisible();
  await expect(instructions.getByRole("heading", { name: "Convert USDC to USDT" })).toBeVisible();
  await expect(instructions.getByText("BTCUSDC", { exact: true })).toBeVisible();
  await expect(instructions.getByText("USDCUSDT", { exact: true })).toBeVisible();
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
