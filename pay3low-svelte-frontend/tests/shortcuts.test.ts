import test from "node:test";
import assert from "node:assert/strict";
import { comboLabel, comboMatches, parseCombo } from "../src/lib/shortcuts.ts";

function key(init: Partial<KeyboardEvent>): KeyboardEvent {
  return { ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, key: "", code: "", ...init } as KeyboardEvent;
}

test("labels are human readable and stable", () => {
  assert.equal(comboLabel("alt+1"), "Alt+1");
  assert.equal(comboLabel("shift+alt+t"), "Shift+Alt+T");
  assert.equal(comboLabel("ctrl+alt+l"), "Ctrl+Alt+L");
  assert.equal(comboLabel("shift+$"), "Shift+$");
  assert.equal(comboLabel("shift+?"), "Shift+?");
  assert.equal(comboLabel("shift+enter"), "Shift+Enter");
  assert.equal(comboLabel("ctrl+shift+enter"), "Ctrl+Shift+Enter");
  assert.equal(comboLabel("alt+backspace"), "Alt+Backspace");
  assert.equal(comboLabel("alt+f7"), "Alt+F7");
  assert.equal(comboLabel("alt+esc"), "Alt+Esc");
});

test("letters match the physical key on any layout", () => {
  const combo = parseCombo("alt+w");
  assert.ok(combo);
  // Russian layout: the W key types "ц".
  assert.equal(comboMatches(combo, key({ altKey: true, code: "KeyW", key: "ц" })), true);
  assert.equal(comboMatches(combo, key({ altKey: true, code: "KeyW", key: "w" })), true);
  assert.equal(comboMatches(combo, key({ altKey: true, code: "KeyQ", key: "q" })), false);
});

test("modifiers must match exactly", () => {
  const alt = parseCombo("alt+t");
  assert.ok(alt);
  assert.equal(comboMatches(alt, key({ altKey: true, code: "KeyT" })), true);
  assert.equal(comboMatches(alt, key({ altKey: true, shiftKey: true, code: "KeyT" })), false);
  assert.equal(comboMatches(alt, key({ code: "KeyT" })), false);

  const both = parseCombo("shift+alt+t");
  assert.ok(both);
  assert.equal(comboMatches(both, key({ altKey: true, shiftKey: true, code: "KeyT" })), true);
  assert.equal(comboMatches(both, key({ altKey: true, code: "KeyT" })), false);
});

test("shifted symbols map to their physical key", () => {
  const usd = parseCombo("shift+$");
  assert.ok(usd);
  assert.equal(comboMatches(usd, key({ shiftKey: true, code: "Digit4", key: "$" })), true);
  // Russian layout: Shift+4 types ";".
  assert.equal(comboMatches(usd, key({ shiftKey: true, code: "Digit4", key: ";" })), true);
  assert.equal(comboMatches(usd, key({ code: "Digit4", key: "4" })), false);

  const max = parseCombo("shift+*");
  assert.ok(max);
  assert.equal(comboMatches(max, key({ shiftKey: true, code: "Digit8", key: "*" })), true);

  const venues = parseCombo("shift+?");
  assert.ok(venues);
  assert.equal(comboMatches(venues, key({ shiftKey: true, code: "Slash", key: "?" })), true);
  assert.equal(comboMatches(venues, key({ code: "Slash", key: "/" })), false);
});

test("digits use the number row", () => {
  const second = parseCombo("alt+2");
  assert.ok(second);
  assert.equal(comboMatches(second, key({ altKey: true, code: "Digit2", key: "2" })), true);
  assert.equal(comboMatches(second, key({ altKey: true, code: "Numpad2", key: "2" })), false);
});

test("named keys match event.key", () => {
  const submit = parseCombo("shift+enter");
  assert.ok(submit);
  assert.equal(comboMatches(submit, key({ shiftKey: true, key: "Enter" })), true);
  assert.equal(comboMatches(submit, key({ key: "Enter" })), false);

  const guide = parseCombo("ctrl+shift+enter");
  assert.ok(guide);
  assert.equal(comboMatches(guide, key({ ctrlKey: true, shiftKey: true, key: "Enter" })), true);

  const refresh = parseCombo("alt+f7");
  assert.ok(refresh);
  assert.equal(comboMatches(refresh, key({ altKey: true, key: "F7" })), true);

  const menu = parseCombo("alt+esc");
  assert.ok(menu);
  assert.equal(comboMatches(menu, key({ altKey: true, key: "Escape" })), true);
});

test("unknown combos are rejected instead of breaking the page", () => {
  const warn = console.warn;
  console.warn = () => {};
  try {
    assert.equal(parseCombo("hyper+t"), null);
    assert.equal(parseCombo("alt+unknownkey"), null);
  } finally {
    console.warn = warn;
  }
});
