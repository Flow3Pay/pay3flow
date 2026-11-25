<script lang="ts">
  import { onMount, tick } from "svelte";

  let label = "";
  let shortcut = "";
  let x = 0;
  let y = 0;
  let tooltip: HTMLDivElement;
  let trigger: HTMLElement | null = null;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let previousDescription: string | null = null;
  let previousTitle: string | null = null;
  const id = "icon-control-tooltip";

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy() { node.remove(); } };
  }
  function hide() {
    clearTimeout(hideTimer);
    if (trigger) {
      if (previousDescription === null) trigger.removeAttribute("aria-describedby");
      else trigger.setAttribute("aria-describedby", previousDescription);
      if (previousTitle !== null) trigger.setAttribute("title", previousTitle);
    }
    trigger = null;
    label = "";
    shortcut = "";
  }
  function scheduleHide() {
    clearTimeout(hideTimer);
    hideTimer = setTimeout(hide, 120);
  }
  function position() {
    if (!trigger || !tooltip) return;
    const rect = trigger.getBoundingClientRect();
    const tip = tooltip.getBoundingClientRect();
    x = Math.max(8, Math.min(rect.left + rect.width / 2 - tip.width / 2, window.innerWidth - tip.width - 8));
    y = rect.bottom + 8;
    if (y + tip.height > window.innerHeight - 8) y = Math.max(8, rect.top - tip.height - 8);
  }
  function resolveLabel(node: HTMLElement) {
    if (node.hasAttribute("data-tooltip")) return node.getAttribute("data-tooltip") ?? "";
    return node.getAttribute("aria-label") || node.getAttribute("title") || "";
  }
  async function show(node: HTMLElement) {
    clearTimeout(hideTimer);
    if (trigger === node) return;
    hide();
    trigger = node;
    previousDescription = node.getAttribute("aria-describedby");
    previousTitle = node.getAttribute("title");
    const name = resolveLabel(node);
    const text = normalize(node.textContent);
    // Never repeat the visible text: the shortcut hint is the only new information.
    label = text && normalize(name) === text ? "" : name;
    shortcut = node.getAttribute("data-shortcut") ?? "";
    node.removeAttribute("title");
    node.setAttribute("aria-describedby", [previousDescription, id].filter(Boolean).join(" "));
    await tick();
    position();
  }
  function normalize(value: string | null) {
    return (value ?? "").replace(/\s+/g, " ").trim().toLocaleLowerCase();
  }
  function candidate(target: EventTarget | null) {
    if (!(target instanceof Element)) return null;
    const node = target.closest<HTMLElement>("button[aria-label], a[aria-label], [data-shortcut]");
    if (!node || node.matches(":disabled") || node.closest(".venueRow") || node.matches(".sheetHandle, .periodHandle")) return null;
    // Controls that opt out keep their accessible name for screen readers only.
    if (node.hasAttribute("data-no-tooltip")) return null;
    const name = resolveLabel(node);
    if (!name && !node.hasAttribute("data-shortcut")) return null;
    // Explicit tooltips and shortcut hints always add information.
    if (node.hasAttribute("data-shortcut") || node.hasAttribute("data-tooltip")) return node;
    // Text-labelled controls already explain themselves. Icon buttons and compact
    // language/currency controls need the longer accessible name on hover/focus.
    if ((node.textContent?.trim().length ?? 0) > 3) return null;
    // A tooltip that only repeats the visible label adds nothing.
    return normalize(name) === normalize(node.textContent) ? null : node;
  }
  onMount(() => {
    const over = (event: PointerEvent) => {
      if (event.pointerType === "touch") return;
      if (event.target instanceof Node && tooltip?.contains(event.target)) { clearTimeout(hideTimer); return; }
      const node = candidate(event.target);
      if (node) void show(node);
      else scheduleHide();
    };
    const out = (event: PointerEvent) => {
      if (event.relatedTarget instanceof Node && (trigger?.contains(event.relatedTarget) || tooltip?.contains(event.relatedTarget))) return;
      scheduleHide();
    };
    const focus = (event: FocusEvent) => {
      const node = candidate(event.target);
      if (node?.matches(":focus-visible")) void show(node);
      else hide();
    };
    const key = (event: KeyboardEvent) => { if (event.key === "Escape") hide(); };
    document.addEventListener("pointerover", over);
    document.addEventListener("pointerout", out);
    document.addEventListener("focusin", focus);
    document.addEventListener("focusout", scheduleHide);
    document.addEventListener("keydown", key);
    document.addEventListener("click", hide);
    document.addEventListener("scroll", hide, true);
    window.addEventListener("resize", hide);
    return () => {
      hide();
      document.removeEventListener("pointerover", over);
      document.removeEventListener("pointerout", out);
      document.removeEventListener("focusin", focus);
      document.removeEventListener("focusout", scheduleHide);
      document.removeEventListener("keydown", key);
      document.removeEventListener("click", hide);
      document.removeEventListener("scroll", hide, true);
      window.removeEventListener("resize", hide);
    };
  });
</script>

{#if label || shortcut}
  <div use:portal bind:this={tooltip} {id} role="tooltip" class="iconTooltip" style:left={`${x}px`} style:top={`${y}px`}>
    {#if label}<span>{label}</span>{/if}
    {#if shortcut}<kbd>{shortcut}</kbd>{/if}
  </div>
{/if}

<style>
  .iconTooltip { position: fixed; z-index: 2000; display: flex; align-items: center; gap: 8px; max-width: min(280px, calc(100vw - 16px)); padding: 8px 12px; border-radius: 8px; background: var(--color-text); color: var(--color-paper); font-size: 12px; font-weight: 650; line-height: 1.5; overflow-wrap: anywhere; }
  .iconTooltip kbd { flex: 0 0 auto; padding: 1px 5px; border: 1px solid currentColor; border-radius: 4px; font-family: var(--font-mono); font-size: 11px; font-weight: 600; white-space: nowrap; opacity: .85; }
  .iconTooltip::before { position: absolute; inset: -8px 0; z-index: -1; content: ""; }
</style>
