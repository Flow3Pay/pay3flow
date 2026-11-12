import { test } from "node:test";
import assert from "node:assert/strict";
import { bestchangeProfileUrl } from "../src/lib/guides/bestchange/frames.ts";

test("BestChange profile links keep exchanger pages on every supported host", () => {
  for (const url of [
    "https://www.bestchange.com/geekchange-exchanger.html",
    "https://www.bestchange.pro/geeexchange-exchanger.html#pay3flow-id=608",
    "https://www.bestchange.net/example-exchanger.html",
    "https://bestchange.biz/geeexchange-exchanger",
  ]) {
    assert.equal(bestchangeProfileUrl(url), url);
  }
});

test("BestChange aggregate direction pages are not treated as exchanger profiles", () => {
  for (const url of [
    "https://www.bestchange.com/tether-bep20-to-usdc-base.html?p=1345467&fromAmount=25000",
    "https://www.bestchange.pro/",
    "https://example.com/geekchange-exchanger.html",
  ]) {
    assert.equal(bestchangeProfileUrl(url), null);
  }
  assert.equal(bestchangeProfileUrl(null), null);
  assert.equal(bestchangeProfileUrl(""), null);
});
