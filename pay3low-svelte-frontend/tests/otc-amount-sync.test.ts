import { test } from "node:test";
import assert from "node:assert/strict";
import { offerTotal, offerEver, saleRate, marketUsdtPerEver } from "../src/lib/otc/offer-amount.ts";
import { sameWalletAddress } from "../src/lib/wallet-amount.ts";

/** Mirror of the OTC ticket hint: convert one filled leg without writing the other. */
function conversionHints(everAmount: string, usdtAmount: string, rate: string) {
  return {
    usdtApprox: rate ? offerTotal(everAmount, rate)?.amount || "" : "",
    everApprox: rate ? offerEver(usdtAmount, rate)?.amount || "" : "",
  };
}

/** Mirror of the funded USDT balance gate: Ethereum balances never apply to other networks. */
function fundedStablecoinBalance(args: {
  network: string;
  receivingAddress: string;
  verifiedEthereum: string;
  balances: Array<{ symbol: string; chain: string; state: string; amount: string | null }>;
  symbol: string;
}) {
  if (args.network !== "ethereum" || !args.verifiedEthereum) return "";
  if (!sameWalletAddress("evm", args.receivingAddress.trim(), args.verifiedEthereum)) return "";
  return args.balances.find(
    (balance) => balance.symbol === args.symbol && balance.chain === "ethereum" && balance.state === "ok",
  )?.amount || "";
}

test("changing one amount only updates the opposite conversion hint", () => {
  const rate = marketUsdtPerEver(0.000837, 1.0054)!;
  const afterEver = conversionHints("1200", "", rate);
  assert.equal(afterEver.usdtApprox, "0.999005");
  assert.equal(afterEver.everApprox, "");

  const afterUsdt = conversionHints("1200", "5", rate);
  assert.equal(afterUsdt.usdtApprox, "0.999005");
  assert.equal(afterUsdt.everApprox, offerEver("5", rate)?.amount);
  // The entered legs stay whatever the trader typed; the rate never rewrites them.
  assert.equal(saleRate("1200", "5"), "0.004166666666");
});

test("empty counterpart fields keep amounts independent of the market rate", () => {
  const rate = "0.001";
  assert.equal(offerTotal("100", rate)?.amount, "0.1");
  assert.equal(offerEver("0.1", rate)?.amount, "100");
  // A filled EVER amount with an empty USDT amount only produces a hint.
  assert.deepEqual(conversionHints("100", "", rate), { usdtApprox: "0.1", everApprox: "" });
  assert.deepEqual(conversionHints("", "0.1", rate), { usdtApprox: "", everApprox: "100" });
});

test("USDT shortfall uses the Ethereum balance only on the Ethereum network", () => {
  const balances = [
    { symbol: "USDT", chain: "ethereum", state: "ok", amount: "2" },
    { symbol: "EVER", chain: "everscale", state: "ok", amount: "1000" },
  ];
  const eth = `0x${"a".repeat(40)}`;
  assert.equal(
    fundedStablecoinBalance({
      network: "ethereum",
      receivingAddress: eth,
      verifiedEthereum: eth,
      balances,
      symbol: "USDT",
    }),
    "2",
  );
  assert.equal(
    fundedStablecoinBalance({
      network: "near",
      receivingAddress: "alice.near",
      verifiedEthereum: eth,
      balances,
      symbol: "USDT",
    }),
    "",
    "NEAR offers must not inherit the Ethereum USDT balance warning",
  );
  assert.equal(
    fundedStablecoinBalance({
      network: "ethereum",
      receivingAddress: `0x${"b".repeat(40)}`,
      verifiedEthereum: eth,
      balances,
      symbol: "USDT",
    }),
    "",
    "a hand-typed paying address is not judged against the verified wallet",
  );
});
