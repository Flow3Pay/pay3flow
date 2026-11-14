import test from "node:test";
import assert from "node:assert/strict";
import {
  p2pAction, p2pActive, p2pBothConfirmed, p2pCompatibleSources, p2pDirectAssetMatch, p2pDirectDirectionAllowed, p2pKind, p2pLegActive, p2pLegAsset, p2pLegDone,
  p2pFeeStatus, p2pFormatUnits, p2pLegFailed, p2pOpenAttempt, p2pOverdue, p2pShortAddress, p2pViewerLegs,
  type P2pDeal,
} from "../src/lib/otc/p2p.ts";

test("payment networks are restricted to routes for the exact destination token", () => {
  const sources = [
    { asset: "USDT@ethereum", asset_id: "eth-usdt", destination_asset_ids: ["eth-usdt", "near-usdt"] },
    { asset: "USDC@ethereum", asset_id: "eth-usdc", destination_asset_ids: ["near-usdt"] },
    { asset: "USDT@tron", asset_id: "tron-usdt", destination_asset_ids: ["sol-usdt"] },
  ];
  assert.deepEqual(p2pCompatibleSources(sources, "near-usdt"), [sources[0], sources[1]]);
  assert.deepEqual(p2pCompatibleSources(sources, "missing"), []);
  assert.equal(p2pDirectAssetMatch(sources[0], "USDT@ethereum", "eth-usdt", "ethereum"), true);
  assert.equal(p2pDirectAssetMatch(sources[0], "USDT@ethereum", "lookalike", "ethereum"), false);
  for (const network of [
    "base", "optimism", "bnb-smart-chain", "gnosis",
    "polygon-pos", "avalanche-c", "arbitrum-one",
  ]) {
    const asset = `USDT@${network}`;
    assert.equal(
      p2pDirectAssetMatch(
        { ...sources[0], asset, asset_id: `${network}-usdt` },
        asset,
        `${network}-usdt`,
        network,
      ),
      true,
    );
  }
  for (const network of ["sei", "tron", "ton", "solana"]) {
    const asset = `USDT@${network}`;
    assert.equal(
      p2pDirectAssetMatch(
        { ...sources[0], asset, asset_id: `${network}-usdt` },
        asset,
        `${network}-usdt`,
        network,
      ),
      true,
    );
    assert.equal(p2pDirectAssetMatch(sources[0], asset, `${network}-usdt`, network), false);
  }
  for (const network of ["tron", "ton", "solana"]) {
    assert.equal(p2pDirectDirectionAllowed("buy", network), true);
    assert.equal(p2pDirectDirectionAllowed("sell", network), false);
  }
  assert.equal(p2pDirectDirectionAllowed("sell", "base"), true);
  assert.equal(p2pDirectDirectionAllowed("buy", "near"), false);
});

function deal(overrides: Partial<P2pDeal> = {}): P2pDeal {
  return {
    id: "deal-1", offer_id: "offer-1", state: "payment_pending", role: "taker",
    counterparty: "https://relay.example/actors/owner",
    terms: {
      direction: "sell", ever_amount: "100", usdt_amount: "11.889",
      ever_units: "100000000000", usdt_units: "11889000",
      usdt_payer: "0:buyer", usdt_payee: "0:seller",
      ever_payer: "0:seller", ever_payee: "0:buyer",
    },
    my_leg: "payment", counterparty_leg: "payout",
    my_ever: "0:buyer", counterparty_ever: "0:seller",
    my_confirmed: false, counterparty_confirmed: false,
    pay_by: "2099-01-01T00:00:00Z", payout_by: null,
    created_at: "2026-11-03T12:00:00Z", attempts: [],
    ...overrides,
  };
}

function attempt(state: string, kind = "payment") {
  return { id: `${kind}-${state}`, kind, state, instructions: {}, reference: null } as any;
}

test("each party confirms once and the payment payer waits for the counterparty", () => {
  assert.equal(p2pAction(deal()), "confirm");
  assert.equal(p2pAction(deal({ my_confirmed: true, counterparty_confirmed: false })), null);
  assert.equal(p2pAction(deal({ my_confirmed: true, counterparty_confirmed: true })), "send");
  assert.equal(p2pBothConfirmed(deal({ my_confirmed: true, counterparty_confirmed: true })), true);
});

test("the open leg follows the deal state and the viewer's side", () => {
  assert.equal(p2pKind(deal()), "payment");
  assert.equal(p2pKind(deal({ my_leg: "payout" })), null);
  assert.equal(p2pKind(deal({ state: "delivery_pending", my_leg: "payout" })), "payout");
  assert.equal(p2pKind(deal({ state: "delivery_pending", my_leg: "payment" })), null);
  assert.equal(p2pKind(deal({ state: "completed" })), null);
  const feePaid = attempt("verified", "fee");
  assert.equal(p2pAction(deal({
    state: "delivery_pending", my_leg: "payout", my_confirmed: true, counterparty_confirmed: true,
    fee_accrued: true, attempts: [feePaid],
  })), "send");
  assert.equal(p2pAction(deal({
    state: "delivery_pending", my_leg: "payout", my_confirmed: true, counterparty_confirmed: true,
    fee_accrued: true,
  })), null);
});

test("an unresolved attempt blocks another signature and a failed one retries", () => {
  const both = { my_confirmed: true, counterparty_confirmed: true };
  const prepared = deal({ ...both, attempts: [attempt("prepared")] });
  assert.equal(p2pAction(prepared), "send");
  assert.equal(p2pOpenAttempt(prepared, "payment")?.state, "prepared");
  assert.equal(p2pAction(deal({ ...both, attempts: [attempt("submitted")] })), null);
  assert.equal(p2pAction(deal({ ...both, attempts: [attempt("confirming")] })), null);
  assert.equal(p2pAction(deal({ ...both, attempts: [attempt("failed")] })), "retry");
  assert.equal(p2pLegFailed(deal({ ...both, attempts: [attempt("failed")] }), "payment"), true);
  assert.equal(p2pLegFailed(deal({ ...both, attempts: [attempt("prepared")] }), "payment"), false);
});

test("terminal deals do not render as active", () => {
  for (const state of ["completed", "testnet_completed", "expired", "cancelled"]) {
    assert.equal(p2pActive(deal({ state })), false);
    assert.equal(p2pAction(deal({ state })), null);
  }
  assert.equal(p2pActive(deal()), true);
  assert.equal(p2pActive(deal({ state: "delivery_pending" })), true);
});

test("leg visuals only mark the leg that is actually open", () => {
  const both = { my_confirmed: true, counterparty_confirmed: true };
  const payment = deal(both);
  assert.equal(p2pLegActive(payment, "payment"), true);
  assert.equal(p2pLegActive(payment, "payout"), false);
  assert.equal(p2pLegDone(payment, "payment"), false);

  const delivered = deal({ ...both, state: "delivery_pending" });
  assert.equal(p2pLegDone(delivered, "payment"), true);
  assert.equal(p2pLegActive(delivered, "payout"), false);

  const completed = deal({ ...both, state: "completed" });
  assert.equal(p2pLegDone(completed, "payment"), true);
  assert.equal(p2pLegDone(completed, "payout"), true);

  const testnetCompleted = deal({ ...both, state: "testnet_completed" });
  assert.equal(p2pLegDone(testnetCompleted, "payment"), true);
  assert.equal(p2pLegDone(testnetCompleted, "payout"), false);
  assert.equal(p2pAction(testnetCompleted), null);
});

test("the modal names the exact token each viewer sends and receives", () => {
  assert.equal(p2pLegAsset("payment"), "USDT");
  assert.equal(p2pLegAsset("payout"), "EVER");
  assert.deepEqual(p2pViewerLegs(deal()), {
    send: { asset: "USDT", amount: "11.889" },
    receive: { asset: "EVER", amount: "100" },
  });
  assert.deepEqual(p2pViewerLegs(deal({ my_leg: "payout" })), {
    send: { asset: "EVER", amount: "100" },
    receive: { asset: "USDT", amount: "11.889" },
  });
  assert.equal(p2pLegAsset("payment", "USDC"), "USDC");
  assert.deepEqual(p2pViewerLegs(deal({ terms: { ...deal().terms, stablecoin: "USDC" } })), {
    send: { asset: "USDC", amount: "11.889" },
    receive: { asset: "EVER", amount: "100" },
  });
  assert.deepEqual(p2pViewerLegs(deal({
    fee_policy: {
      version: "v1", rate: "0.03", floor: "0", basis_units: "11889000",
      fee_units: "356670", net_units: "11532330", precision: 6,
    },
  })), {
    send: { asset: "USDT", amount: "11.53233" },
    receive: { asset: "EVER", amount: "100" },
  });
  assert.equal(p2pViewerLegs(deal({ my_leg: undefined })), null);
});

test("late windows are visible without blocking a delivery retry", () => {
  const now = Date.parse("2026-11-03T12:00:00Z");
  assert.equal(p2pOverdue(deal({ pay_by: "2026-11-03T11:59:59Z" }), now), true);
  assert.equal(p2pOverdue(deal({ pay_by: "2099-01-01T00:00:00Z" }), now), false);
  const lateDelivery = deal({
    state: "delivery_pending", my_leg: "payout",
    my_confirmed: true, counterparty_confirmed: true,
    fee_accrued: true,
    attempts: [attempt("verified", "fee")],
    payout_by: "2026-11-03T11:00:00Z",
  });
  assert.equal(p2pOverdue(lateDelivery, now), true);
  assert.equal(p2pAction(lateDelivery), "send");
});

test("addresses stay readable in the modal", () => {
  assert.equal(p2pShortAddress("0:abcdef0123456789"), "0:abcd…6789");
  assert.equal(p2pShortAddress("short"), "short");
  assert.equal(p2pShortAddress(undefined), "");
});

test("fee collection status follows a verified fee attempt and formats exact USDT units", () => {
  const completed = deal({
    state: "completed",
    fee_accrued: true,
    fee_policy: {
      version: "v1", rate: "0.03", floor: "5", basis_units: "10000000",
      fee_units: "5000000", net_units: "9500000", precision: 6,
    },
  });
  assert.equal(p2pFeeStatus(completed), "accrued");
  assert.equal(p2pFeeStatus({ ...completed, attempts: [attempt("prepared", "fee")] }), "pending");
  assert.equal(p2pFeeStatus({ ...completed, attempts: [attempt("verified", "fee")] }), "collected");
  assert.equal(p2pFormatUnits("5000000"), "5");
  assert.equal(p2pFormatUnits("1234567"), "1.234567");
  assert.equal(p2pFormatUnits("12x"), "");
});
