import { test } from "node:test";
import assert from "node:assert/strict";
import { shortPairHash, shortPairLocation } from "../src/lib/short-pair.ts";

test("short asset links expand into the base ticker plus wildcard pair", () => {
  assert.equal(shortPairHash("EVER"), "#/swap/EVER/ANY?from=any-EVER&to=any-ANY");
  assert.equal(shortPairHash("ever"), "#/swap/EVER/ANY?from=any-EVER&to=any-ANY");
});

test("short asset links keep legacy ticker aliases and the URL origin prefix", () => {
  assert.equal(shortPairHash("TON"), "#/swap/GRAM/ANY?from=any-GRAM&to=any-ANY");
  assert.equal(shortPairLocation("EVER"), "/#/swap/EVER/ANY?from=any-EVER&to=any-ANY");
});

test("short asset links pass only safe amount and language settings", () => {
  const params = new URLSearchParams({ amount: "1000,50", lang: "ru", from: "other", to: "other", path: "[bad" });
  assert.equal(shortPairHash("EVER", params), "#/swap/EVER/ANY?amount=1000.50&from=any-EVER&to=any-ANY&lang=ru");
  assert.equal(
    shortPairHash("EVER", new URLSearchParams({ amount: "-1", lang: "de" })),
    "#/swap/EVER/ANY?from=any-EVER&to=any-ANY",
  );
});

test("short asset links reject unknown or unsafe tickers", () => {
  for (const ticker of ["", "EVER/BTC", "<EVER>", "EVER?from=x", "TOO-LONG-TICKER-NAMES"]) {
    assert.equal(shortPairHash(ticker), null);
    assert.equal(shortPairLocation(ticker), null);
  }
});
