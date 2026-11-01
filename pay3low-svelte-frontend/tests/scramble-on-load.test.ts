import { test } from "node:test";
import assert from "node:assert/strict";
import { frameText, introFinished, isScrambableText, isSkippedPath, planRun, SKIP_SELECTORS } from "../src/lib/scramble-on-load.ts";

test("only copy with letters is decoded", () => {
  assert.equal(isScrambableText("1 245,60"), false);
  assert.equal(isScrambableText("4.47061"), false);
  assert.equal(isScrambableText("OT"), false);
  assert.equal(isScrambableText("OTC"), true);
  assert.equal(isScrambableText("Reference rate"), true);
});

test("amount and rate readouts opt out by selector", () => {
  for (const selector of [".amountInput", ".marketValue", ".routeAmount", ".rateRow", ".srOnly", "[data-no-scramble]", "table"]) {
    assert.equal(SKIP_SELECTORS.includes(selector), true, selector);
  }
});

test("static long-form pages never run the load pass", () => {
  for (const [pathname, hash] of [["/about", ""], ["/terms", ""], ["/otc-runbook/", ""], ["/share/guide/am/ru-sberbank", ""], ["/", "#/guide/am/ru-sberbank"]] as const) {
    assert.equal(isSkippedPath(pathname, hash), true, pathname + hash);
  }
  for (const [pathname, hash] of [["/", "#/swap/AMD/RUB"], ["/", "#/otc"], ["/trends", ""], ["/providers", ""], ["/providers/binance", ""]] as const) {
    assert.equal(isSkippedPath(pathname, hash), false, pathname + hash);
  }
});

test("only letters scramble and every node ends on its original copy", () => {
  const value = "Send 12 AMD now";
  const run = planRun(value, 0, () => 0.5)!;
  assert.equal(run.chars.join(""), value);
  assert.equal(run.maxEnd > 0, true);

  let midflight = "";
  for (let elapsed = 0; elapsed < run.maxEnd; elapsed += 8) midflight = frameText(run, elapsed, elapsed, () => 0);
  assert.match(midflight, /#/, "the last letter is still decoding before maxEnd");
  assert.equal(frameText(run, run.maxEnd, run.maxEnd), value, "the final frame restores the copy");

  const first = frameText(run, run.starts[0], run.starts[0], () => 0);
  assert.equal(first[0], "#", "the letter that just started shows a glyph");
  assert.equal(first.slice(1).replace(/\s/g, ""), value.slice(1).replace(/\s/g, ""));
  assert.equal(first.match(/\d/g)?.join(""), "12", "digits keep their place");
  assert.equal([...first].length, [...value].length);
});

test("long paragraphs stay inside a short window", () => {
  const value = `Move money across borders ${"and keep more of it ".repeat(6)}`;
  const run = planRun(value, 900, () => 1)!;
  assert.equal(run.maxEnd - 900 < 900, true, `${run.maxEnd - 900}`);
});

test("the load pass waits for the startup animation", () => {
  let value = true;
  const unsubscribe = introFinished.subscribe((ready) => { value = ready; });
  introFinished.set(false);
  assert.equal(value, false);
  introFinished.set(true);
  assert.equal(value, true);
  unsubscribe();
});
