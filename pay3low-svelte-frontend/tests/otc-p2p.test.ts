import test from "node:test";
import assert from "node:assert/strict";
import {
  p2pAction, p2pActive, p2pBothConfirmed, p2pKind, p2pLegActive, p2pLegDone,
  p2pLegFailed, p2pOpenAttempt, p2pShortAddress,
  type P2pDeal, type P2pLeg,
} from "../src/lib/otc/p2p.ts";

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

function attempt(state: string, kind: P2pLeg = "payment") {
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
  assert.equal(p2pAction(deal({ state: "delivery_pending", my_leg: "payout", my_confirmed: true, counterparty_confirmed: true })), "send");
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
  for (const state of ["completed", "expired", "cancelled"]) {
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
});

test("addresses stay readable in the modal", () => {
  assert.equal(p2pShortAddress("0:abcdef0123456789"), "0:abcd…6789");
  assert.equal(p2pShortAddress("short"), "short");
  assert.equal(p2pShortAddress(undefined), "");
});
