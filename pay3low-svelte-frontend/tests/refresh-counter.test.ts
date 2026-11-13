import { test } from "node:test";
import assert from "node:assert/strict";
import { refreshCounter } from "../src/lib/refresh-counter.ts";

test("auto-refresh counts down after the latest snapshot", () => {
  assert.deepEqual(refreshCounter(12_000, 10_000, 15, null, false), {
    secondsSinceUpdate: 2,
    elapsedSeconds: 2,
    secondsUntilRefresh: 13,
  });
});

test("the age readout freezes while the next search is running", () => {
  assert.deepEqual(refreshCounter(25_000, 10_000, 15, 15_000, true), {
    secondsSinceUpdate: 5,
    elapsedSeconds: 5,
    secondsUntilRefresh: 10,
  });
});

test("auto-refresh never shows an age past its interval", () => {
  assert.deepEqual(refreshCounter(60_000, 10_000, 15, null, false), {
    secondsSinceUpdate: 50,
    elapsedSeconds: 15,
    secondsUntilRefresh: 0,
  });
});

test("manual refresh mode leaves the age unbounded and has no countdown", () => {
  assert.deepEqual(refreshCounter(20_000, 10_000, 0, 15_000, true), {
    secondsSinceUpdate: 5,
    elapsedSeconds: 5,
    secondsUntilRefresh: null,
  });
});

test("an initial search has no age until the first snapshot arrives", () => {
  assert.deepEqual(refreshCounter(20_000, null, 15, 15_000, true), {
    secondsSinceUpdate: null,
    elapsedSeconds: null,
    secondsUntilRefresh: null,
  });
});
