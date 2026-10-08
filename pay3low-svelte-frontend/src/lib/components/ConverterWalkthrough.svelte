<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { locale, t } from "$lib/i18n";
  import { fiatFlagUrl } from "$lib/currency-flags";
  import { paymentMethodFavicon, type PaymentMethod } from "$lib/payment-methods";
  import type { CryptoNetwork } from "$lib/networks";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import PaymentMethodPicker from "./PaymentMethodPicker.svelte";

  export let card: HTMLElement;
  export let paymentMethods: PaymentMethod[];
  export let networks: CryptoNetwork[];
  export let onClose: () => void;

  let root: HTMLDivElement;
  let stage: HTMLDivElement;
  let closeButton: HTMLButtonElement;
  let replica: HTMLElement;
  let picker: "source" | "target" | null = null;
  let selected: PaymentMethod | null = null;
  let step = 1;
  let cursorX = 30, cursorY = 30;
  let clicking = false;
  let hole = { x: 0, y: 0, width: 0, height: 0 };
  let reducedMotion = false;
  let unlock: (() => void) | undefined;
  let previousFocus: HTMLElement | null = null;
  const inertSiblings = new Map<HTMLElement, boolean>();
  const controller = new AbortController();
  const captions = ["Start with Bridge", "Choose where you pay from", "Choose where the recipient gets paid", "Enter an amount", "Your route starts here"];
  const wait = (ms: number) => new Promise<void>((resolve, reject) => {
    if (controller.signal.aborted) { reject(new DOMException("Cancelled", "AbortError")); return; }
    const abort = () => { clearTimeout(timer); reject(new DOMException("Cancelled", "AbortError")); };
    const timer = window.setTimeout(() => { controller.signal.removeEventListener("abort", abort); resolve(); }, ms);
    controller.signal.addEventListener("abort", abort, { once: true });
  });

  function spotlight(element: HTMLElement) {
    const rect = element.getBoundingClientRect();
    hole = { x: rect.x - 8, y: rect.y - 8, width: rect.width + 16, height: rect.height + 16 };
  }
  async function point(element: HTMLElement, illuminated = element, click = false) {
    element.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "instant" });
    await tick();
    spotlight(illuminated);
    const rect = element.getBoundingClientRect();
    cursorX = rect.x + rect.width * .58;
    cursorY = rect.y + rect.height * .55;
    await wait(850);
    if (click) { clicking = true; await wait(180); element.click(); clicking = false; await tick(); }
  }
  function clonePanel(side: "source" | "target") {
    return replica.querySelector<HTMLElement>(side === "source" ? ".moneyPanelSource" : ".moneyPanelTarget")!;
  }
  function displayMethod(method: PaymentMethod) {
    selected = method;
    const panel = clonePanel(picker!);
    const name = panel.querySelector<HTMLElement>(".methodText strong")!;
    name.textContent = method.name;
    panel.querySelector(".methodText")?.classList.remove("methodTextAsset");
    const avatar = panel.querySelector<HTMLElement>(".methodAvatar")!;
    const logo = paymentMethodFavicon(method);
    avatar.replaceChildren();
    avatar.style.backgroundColor = logo ? "transparent" : method.color;
    if (logo) { const image = document.createElement("img"); image.src = logo; image.alt = ""; image.style.cssText = "width:100%;height:100%;object-fit:contain;border-radius:inherit"; avatar.append(image); }
    else avatar.textContent = method.initials;
    const currency = panel.querySelector<HTMLElement>(".networkButton")!;
    const dot = currency.querySelector<HTMLElement>(".networkDot")!;
    dot.replaceChildren();
    const flag = fiatFlagUrl(method.currency);
    if (flag) { const image = document.createElement("img"); image.src = flag; image.alt = ""; image.style.cssText = "width:18px;height:18px;border-radius:50%;object-fit:cover"; dot.append(image); }
    else dot.textContent = method.currency;
    currency.querySelector(".networkCopy")?.remove();
    const label = document.createElement("strong"); label.textContent = method.currency; label.style.fontSize = "14px"; currency.insertBefore(label, currency.querySelector("svg"));
    picker = null;
  }
  function demoMethod(role: "sender" | "recipient", preferred: string, otherCurrency = "") {
    const available = paymentMethods.filter(method => method.kind === "bank" && (method.role === role || method.role === "both"));
    return available.find(method => method.id === preferred) ?? available.find(method => method.currency !== otherCurrency) ?? available[0];
  }
  function option(name: string) {
    const options = Array.from(root.querySelectorAll<HTMLButtonElement>(".demoPicker .optionCard"));
    const match = options.find(button => button.querySelector(".optionName")?.textContent === name);
    if (!match) throw new Error("Demo option unavailable");
    return match;
  }
  async function selectMethod(side: "source" | "target", method: PaymentMethod) {
    step = side === "source" ? 2 : 3;
    const panel = clonePanel(side);
    await point(panel.querySelector<HTMLElement>(".methodTrigger")!, panel, true);
    selected = null;
    picker = side;
    await tick();
    await wait(220);
    const dialog = root.querySelector<HTMLElement>(".demoPicker .dialog")!;
    await point(option(method.currency.toUpperCase()), dialog, true);
    await wait(200);
    await point(option(method.name), dialog, true);
    spotlight(panel);
    await wait(450);
  }
  async function play() {
    const source = demoMethod("sender", "am-ameriabank");
    const target = demoMethod("recipient", "ru-sberbank", source?.currency);
    if (!source || !target) { onClose(); return; }
    try {
      await wait(350);
      await point(replica.querySelector<HTMLElement>(".modeActive")!, undefined, true);
      await selectMethod("source", source);
      await selectMethod("target", target);
      step = 4;
      const amount = replica.querySelector<HTMLInputElement>(".amountInput")!;
      stage.style.setProperty("--amount-digits", "4");
      await point(amount, clonePanel("source"), true);
      amount.value = "";
      for (const digit of "1000") { await wait(220); amount.value += digit; }
      const cta = replica.querySelector<HTMLButtonElement>(".cta")!;
      cta.textContent = `${t("Find routes", {}, $locale)} ↗`; cta.disabled = false;
      step = 5;
      await wait(1400);
      onClose();
    } catch (error) {
      if (!controller.signal.aborted) onClose();
    }
  }
  function keydown(event: KeyboardEvent) {
    if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); onClose(); }
    if (event.key === "Tab") { event.preventDefault(); closeButton.focus(); }
  }
  function resize() { onClose(); }
  onMount(() => {
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    replica = card.cloneNode(true) as HTMLElement;
    // DOM cloning deliberately copies no Svelte listeners. Demo changes can never
    // reach exchange state, URL persistence, analytics or route-search handlers.
    for (const input of Array.from(card.querySelectorAll("input"))) {
      const copy = replica.querySelector<HTMLInputElement>(`#${input.id}`);
      if (copy) copy.value = input.value;
    }
    for (const node of Array.from(replica.querySelectorAll<HTMLElement>("[id]"))) node.removeAttribute("id");
    replica.inert = true;
    replica.setAttribute("aria-hidden", "true");
    replica.style.width = `${card.getBoundingClientRect().width}px`;
    replica.style.maxWidth = "100%";
    const computed = getComputedStyle(card);
    stage.style.setProperty("--converter-width", computed.getPropertyValue("--converter-width"));
    stage.style.setProperty("--amount-digits", computed.getPropertyValue("--amount-digits"));
    const cta = replica.querySelector<HTMLButtonElement>(".cta")!;
    cta.textContent = t("Enter an amount to begin", {}, $locale); cta.disabled = true;
    replica.querySelector<HTMLInputElement>(".moneyPanelTarget .amountInput")!.value = "0";
    replica.querySelectorAll(".marketValue, .errorBox").forEach(node => node.remove());
    stage.append(replica);
    let ancestor: HTMLElement = root;
    while (ancestor.parentElement && ancestor !== document.body) {
      for (const sibling of Array.from(ancestor.parentElement.children)) {
        if (sibling !== ancestor && sibling instanceof HTMLElement) { inertSiblings.set(sibling, sibling.inert); sibling.inert = true; }
      }
      ancestor = ancestor.parentElement;
    }
    unlock = lockPageScroll();
    closeButton.focus({ preventScroll: true });
    window.addEventListener("keydown", keydown, true);
    window.addEventListener("resize", resize);
    void play();
  });
  onDestroy(() => {
    controller.abort();
    unlock?.();
    inertSiblings.forEach((inert, node) => node.inert = inert);
    if (typeof window !== "undefined") { window.removeEventListener("keydown", keydown, true); window.removeEventListener("resize", resize); }
    previousFocus?.focus({ preventScroll: true });
  });
</script>

<div class="walkthrough" class:reducedMotion bind:this={root} role="dialog" aria-modal="true" aria-label={t("How to find a route", {}, $locale)} data-testid="converter-walkthrough" data-step={step} tabindex="-1">
  <div class="demoStage" bind:this={stage}></div>
  <div class="demoPicker" inert>
    <PaymentMethodPicker open={picker !== null} title={t(picker === "target" ? "Choose where the recipient gets paid" : "Choose where you pay from", {}, $locale)} role={picker === "target" ? "recipient" : "sender"} {paymentMethods} {networks} {selected} selectedNetwork={undefined} {onClose} onSelect={displayMethod} />
  </div>
  <svg class="veil" aria-hidden="true" width="100%" height="100%"><defs><mask id="converter-demo-spotlight"><rect width="100%" height="100%" fill="white"/><rect x={hole.x} y={hole.y} width={hole.width} height={hole.height} rx="14" fill="black"/></mask></defs><rect width="100%" height="100%" fill="#030703" opacity=".79" mask="url(#converter-demo-spotlight)"/><rect x={hole.x} y={hole.y} width={hole.width} height={hole.height} rx="14" fill="none" stroke="#b5f500" stroke-opacity=".7" stroke-width="1.5"/></svg>
  <div class="cursor" class:clicking style:transform={`translate3d(${cursorX}px, ${cursorY}px, 0)`} aria-hidden="true"><span></span><svg width="32" height="38" viewBox="0 0 32 38" fill="none"><path d="M3 2L27 22L15 24L9 35L3 2Z" fill="#b5f500" stroke="#132015" stroke-width="2.5"/></svg></div>
  <div class="guideTop"><span>PAY3FLOW <b>/</b> {t("How to find a route", {}, $locale)}</span><button type="button" bind:this={closeButton} on:click={onClose} aria-label={t("Close guide", {}, $locale)}>{t("Close guide", {}, $locale)} <span aria-hidden="true">×</span></button></div>
  <div class="guideCaption"><span class="stepNumber">0{step}<small>/ 05</small></span><div><strong aria-live="polite">{t(captions[step - 1], {}, $locale)}</strong><div class="progress" aria-hidden="true">{#each captions as _, index}<span class:complete={index < step}></span>{/each}</div></div></div>
</div>

<style>
  .walkthrough { position: fixed; inset: 0; z-index: 3000; isolation: isolate; }
  .demoStage { position: absolute; inset: 78px 16px 108px; overflow: auto; display: flex; align-items: safe center; flex-direction: column; padding: 12px; scrollbar-width: none; }
  .demoStage :global(.card) { flex-shrink: 0; margin-block: auto; }
  .demoPicker :global(.backdrop) { padding: 76px 16px 112px; }
  .demoPicker :global(.dialog) { max-height: calc(100dvh - 200px); }
  .demoPicker :global(.methods) { height: min(540px, calc(100dvh - 360px)); }
  .veil { position: fixed; inset: 0; z-index: 1200; pointer-events: none; }
  .cursor { position: fixed; top: 0; left: 0; z-index: 1300; pointer-events: none; transition: transform .7s cubic-bezier(.22,.68,0,1); filter: drop-shadow(0 4px 12px #0008); }
  .cursor svg { position: relative; transition: scale .15s; }
  .cursor > span { position: absolute; inset: -12px; width: 36px; height: 36px; border: 2px solid #b5f500; border-radius: 50%; opacity: 0; }
  .cursor.clicking svg { scale: .85; }
  .cursor.clicking > span { opacity: .6; }
  .guideTop { position: fixed; inset: 16px 24px auto; z-index: 1400; display: flex; align-items: center; justify-content: space-between; gap: 12px; color: #f0f4e8; font: 11px/1.5 var(--font-mono); }
  .guideTop b { margin-inline: 8px; color: #b5f500; }
  .guideTop button { display: flex; align-items: center; gap: 12px; padding: 10px 14px; background: #182016; color: #edf2e6; border: 1px solid #b5f50066; border-radius: 10px; white-space: nowrap; }
  .guideTop button:hover { background: #b5f500; color: #132015; }
  .guideTop button > span { font: 23px/1 var(--font-sans); }
  .guideCaption { position: fixed; inset: auto 16px 20px; z-index: 1400; display: flex; align-items: center; gap: 18px; width: fit-content; max-width: calc(100% - 32px); margin-inline: auto; padding: 18px 26px; border: 1px solid #b5f50044; border-radius: 16px; background: #142010; color: #f1f5e9; box-shadow: 0 12px 40px #0004; }
  .stepNumber { font: 30px/1 var(--font-mono); color: #b5f500; white-space: nowrap; }
  .stepNumber small { font-size: 11px; color: #9ea995; }
  .guideCaption strong { font-size: 15px; line-height: 1.4; }
  .progress { display: flex; gap: 5px; margin-top: 10px; }
  .progress span { width: 28px; height: 3px; border-radius: 2px; background: #ffffff26; }
  .progress span.complete { background: #b5f500; }
  .reducedMotion .cursor { transition: none; }
  @media (max-width: 640px) { .guideTop { inset: 12px 16px auto; } .guideTop > span { max-width: 54%; } .guideTop b { margin-inline: 3px; } .guideCaption { padding: 14px 18px; gap: 12px; } .guideCaption strong { font-size: 13px; } .demoStage { inset-inline: 4px; } }
</style>
