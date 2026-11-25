# Desktop keyboard shortcuts

The web app is keyboard-first on desktop: every primary control has a shortcut,
and hovering a control shows its combo in the tooltip next to the control name.
Shortcuts are implemented in
[`pay3low-svelte-frontend/src/lib/shortcuts.ts`](../pay3low-svelte-frontend/src/lib/shortcuts.ts)
and attached to controls with the `use:hotkey` action. The action registers the
element, publishes the combo in `data-shortcut` (rendered by
`IconTooltip.svelte`) and runs the element's own click handler, so a shortcut
always does exactly what a click does.

## Behavior rules

- Combos are matched by `event.code`, so they work on non-Latin layouts: Alt+W
  is the physical W key on a Russian keyboard too.
- Modifiers must match exactly: Alt+T does not fire on Shift+Alt+T.
- Shortcuts that drive amount fields (`Shift+$`, `Shift+*`, `Shift+Enter`, the
  token/network pickers) also work while an input has focus; the rest are
  ignored while typing so they do not steal characters from search fields.
- Page actions are suspended while a modal dialog is open. Header-level actions
  (navigation, wallet, language, theme, share, menu) stay active.
- Shortcuts only fire for visible controls, so the desktop and mobile variants
  of the same control never run twice.
- `event.repeat` and IME composition are ignored, and handled combos call
  `preventDefault()`.

## Combos

### Header

| Combo | Action |
| --- | --- |
| Alt+1 … Alt+4 | Switch product section in nav order: SWAP, OTC, TRENDS, PLATFORMS |
| Alt+W | Open or close the wallet dialog |
| Ctrl+Alt+L | Cycle the interface language without opening the picker |
| Alt+T | Switch light/dark theme |
| Alt+Backspace | Open the share dialog |
| Alt+Esc, Alt+M | Open the header menu (Alt+M is the fallback: some systems reserve Alt+Esc) |

The nav combos follow the rendered order (`ScrambleLink` takes `shortcutIndex`),
so adding a fifth section only needs the next index.

### SWAP

| Combo | Action |
| --- | --- |
| Shift+$ | Toggle the amount input between USD and the token |
| Shift+* | Use the whole wallet balance (Max) |
| Shift+Enter | Primary call to action: find routes, open the guide, or swap |
| Ctrl+Shift+Enter | Secondary “SWAP guide” button when both actions are shown |
| Alt+S | Route refresh settings |
| Alt+F7, Alt+R | Refresh routes now (Alt+R is the fallback: some window managers reserve Alt+F7) |
| Shift+? | Open the venue list for the found routes |
| Alt+A | Swap sender and recipient |
| Shift+T | Sender asset or payment method |
| Shift+C | Sender network or fiat currency |
| Shift+Alt+T | Recipient asset or payment method |
| Shift+Alt+C | Recipient network or fiat currency |

### OTC

| Combo | Action |
| --- | --- |
| Shift+B | Buy EVER |
| Shift+S | Sell EVER |
| Shift+T | Recipient token (USDT/USDC) |
| Shift+C | Recipient network |
| Shift+$ | Toggle the amount between USD and EVER |
| Shift+* | Use the whole available balance |
| Shift+1, Shift+2, Shift+3 | Activity tabs: orderbook/offers, my orders, history |
| Shift+Enter | Review or publish the offer |
| Alt+A | Swap buy and sell |

## Adding a shortcut

```svelte
<button use:hotkey={{ keys: "alt+s", allowInInputs: true }} on:click={openSettings}>
```

`keys` accepts a string or a list of alternatives. Add `global: true` for
header-level controls that must work while a dialog is open, and pass `run` when
the shortcut should do something other than click the element (for example
`Ctrl+Alt+L` cycles the language). Update this table when you add one.
