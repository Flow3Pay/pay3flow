/**
 * Desktop keyboard shortcuts.
 *
 * Controls opt in with `use:hotkey={"alt+1"}`. The action registers the element,
 * publishes the combo in `data-shortcut` (so IconTooltip can show it on hover)
 * and runs the element's own click handler when the combo is pressed.
 *
 * Combos are matched by `event.code`, so they keep working on non-Latin
 * layouts: Alt+W is the physical W key on a Russian keyboard as well.
 */

type Combo = {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  /** Matched against `event.key` for named keys such as Enter or Escape. */
  key?: string;
  /** Matched against `event.code` for letters, digits and symbols. */
  code?: string;
};

export type HotkeyOptions =
  | string
  | {
      keys: string | string[];
      /** Amount fields are inputs, so their shortcuts must work while typing. */
      allowInInputs?: boolean;
      /** Global controls stay active while a dialog is open. */
      global?: boolean;
      run?: (node: HTMLElement) => void;
    };

type Registration = {
  node: HTMLElement;
  combos: Combo[];
  allowInInputs: boolean;
  global: boolean;
  run: (node: HTMLElement) => void;
};

const NAMED_KEYS: Record<string, string> = {
  enter: "enter",
  escape: "escape",
  esc: "escape",
  space: " ",
  backspace: "backspace",
  tab: "tab",
  delete: "delete",
  home: "home",
  end: "end",
  pageup: "pageup",
  pagedown: "pagedown",
  arrowup: "arrowup",
  arrowdown: "arrowdown",
  arrowleft: "arrowleft",
  arrowright: "arrowright",
  insert: "insert",
  ...Object.fromEntries(Array.from({ length: 12 }, (_, index) => [`f${index + 1}`, `f${index + 1}`])),
};

const NAMED_KEY_LABELS: Record<string, string> = {
  enter: "Enter",
  escape: "Esc",
  esc: "Esc",
  space: "Space",
  backspace: "Backspace",
  tab: "Tab",
  delete: "Delete",
  home: "Home",
  end: "End",
  pageup: "PageUp",
  pagedown: "PageDown",
  arrowup: "↑",
  arrowdown: "↓",
  arrowleft: "←",
  arrowright: "→",
  insert: "Insert",
};

/** Shifted glyphs map to the physical key they sit on, as on a US keyboard. */
const SYMBOL_CODES: Record<string, string> = {
  "`": "Backquote",
  "~": "Backquote",
  "!": "Digit1",
  "@": "Digit2",
  "#": "Digit3",
  "$": "Digit4",
  "%": "Digit5",
  "^": "Digit6",
  "&": "Digit7",
  "*": "Digit8",
  "(": "Digit9",
  ")": "Digit0",
  "-": "Minus",
  "_": "Minus",
  "=": "Equal",
  "+": "Equal",
  "[": "BracketLeft",
  "{": "BracketLeft",
  "]": "BracketRight",
  "}": "BracketRight",
  "\\": "Backslash",
  "|": "Backslash",
  ";": "Semicolon",
  ":": "Semicolon",
  "'": "Quote",
  '"': "Quote",
  ",": "Comma",
  "<": "Comma",
  ".": "Period",
  ">": "Period",
  "/": "Slash",
  "?": "Slash",
};

function codeFor(key: string): string | undefined {
  if (/^[a-z]$/.test(key)) return `Key${key.toUpperCase()}`;
  if (/^[0-9]$/.test(key)) return `Digit${key}`;
  if (/^f([1-9]|1[0-2])$/.test(key)) return key.toUpperCase();
  return SYMBOL_CODES[key];
}

export function parseCombo(spec: string): Combo | null {
  const parts = spec.split("+").map((part) => part.trim()).filter(Boolean);
  const rawKey = parts.pop() ?? "";
  const key = rawKey.toLowerCase();
  const combo: Combo = { ctrl: false, alt: false, shift: false, meta: false };
  for (const part of parts) {
    switch (part.toLowerCase()) {
      case "ctrl":
      case "control":
        combo.ctrl = true;
        break;
      case "alt":
      case "option":
        combo.alt = true;
        break;
      case "shift":
        combo.shift = true;
        break;
      case "meta":
      case "cmd":
        combo.meta = true;
        break;
      default:
        console.warn(`[shortcuts] Unknown modifier "${part}" in "${spec}"`);
        return null;
    }
  }
  const named = NAMED_KEYS[key];
  if (named) combo.key = named;
  else {
    const code = codeFor(key);
    if (!code) {
      console.warn(`[shortcuts] Unknown key "${rawKey}" in "${spec}"`);
      return null;
    }
    combo.code = code;
  }
  return combo;
}

export function comboMatches(combo: Combo, event: KeyboardEvent): boolean {
  if (event.ctrlKey !== combo.ctrl || event.altKey !== combo.alt || event.shiftKey !== combo.shift || event.metaKey !== combo.meta) return false;
  if (combo.key) return event.key.toLowerCase() === combo.key;
  return event.code === combo.code;
}

function keyLabel(key: string): string {
  const lower = key.toLowerCase();
  const named = NAMED_KEY_LABELS[lower];
  if (named) return named;
  if (/^[a-z]$/.test(lower) || /^f([1-9]|1[0-2])$/.test(lower)) return lower.toUpperCase();
  return key;
}

/** Human readable combo for tooltips, e.g. `shift+alt+t` → `Shift+Alt+T`. */
export function comboLabel(spec: string): string {
  const parts = spec.split("+").map((part) => part.trim()).filter(Boolean);
  const key = parts.pop() ?? "";
  const modifiers: string[] = [];
  for (const part of parts) {
    switch (part.toLowerCase()) {
      case "ctrl":
      case "control":
        modifiers.push("Ctrl");
        break;
      case "alt":
      case "option":
        modifiers.push("Alt");
        break;
      case "shift":
        modifiers.push("Shift");
        break;
      case "meta":
      case "cmd":
        modifiers.push("⌘");
        break;
    }
  }
  return [...modifiers, keyLabel(key)].filter(Boolean).join("+");
}

const registry = new Set<Registration>();
let listening = false;

function isEditable(target: EventTarget | null): boolean {
  return target instanceof Element && Boolean(target.closest("input, textarea, select, [contenteditable]:not([contenteditable='false'])"));
}

function isModalOpen(): boolean {
  return Boolean(document.querySelector('[aria-modal="true"]'));
}

function isActionable(node: HTMLElement): boolean {
  if (!node.isConnected || node.closest("[inert]")) return false;
  if (node.matches(":disabled, [aria-disabled='true']")) return false;
  return node.getClientRects().length > 0;
}

function onKeyDown(event: KeyboardEvent) {
  if (event.defaultPrevented || event.repeat || event.isComposing || event.key === "Unidentified") return;
  for (const registration of registry) {
    if (!registration.combos.some((combo) => comboMatches(combo, event))) continue;
    if (!registration.global && isModalOpen()) continue;
    if (!registration.allowInInputs && isEditable(event.target)) continue;
    if (!isActionable(registration.node)) continue;
    event.preventDefault();
    registration.run(registration.node);
    return;
  }
}

function listen() {
  if (listening || typeof window === "undefined") return;
  listening = true;
  window.addEventListener("keydown", onKeyDown);
}

export function hotkey(node: HTMLElement, options: HotkeyOptions) {
  let registration: Registration | null = null;

  const remove = () => {
    if (!registration) return;
    registry.delete(registration);
    registration = null;
    delete node.dataset.shortcut;
  };

  const apply = (value: HotkeyOptions) => {
    remove();
    const normalized = typeof value === "string" ? { keys: value } : value;
    const specs = (Array.isArray(normalized.keys) ? normalized.keys : [normalized.keys]).map((spec) => spec.trim()).filter(Boolean);
    const combos = specs.map(parseCombo).filter((combo): combo is Combo => combo !== null);
    if (!combos.length) return;
    registration = {
      node,
      combos,
      allowInInputs: normalized.allowInInputs ?? false,
      global: normalized.global ?? false,
      run: normalized.run ?? ((target) => target.click()),
    };
    node.dataset.shortcut = specs.map(comboLabel).join(" / ");
    registry.add(registration);
    listen();
  };

  apply(options);
  return { update: apply, destroy: remove };
}
