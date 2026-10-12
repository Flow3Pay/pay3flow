import test from "node:test";
import assert from "node:assert/strict";
import { validateEscrow, validateEscrowTypedData, everSigningAvailable, MAX_EVER_UNITS, type Escrow, type EscrowProposal } from "../src/lib/otc/escrow-model.ts";
import type { Trade } from "../src/lib/otc/service.ts";

const address = (digit: string) => `0x${digit.repeat(40)}`;
function fixture(direction: "buy" | "sell" = "buy") {
  const now = Date.parse("2026-10-12T12:00:00Z");
  const policy = { factory: address("8"), factory_code_hash: `0x${"a".repeat(64)}`, arbitrator: address("9"), singleton: address("7"), singleton_code_hash: `0x${"b".repeat(64)}` };
  const trade = { id: "trade", state: "funded", terms: { direction, escrow: policy, customer_usdt: address("1"), desk_usdt: address("2"), input_units: direction === "buy" ? "100000000" : "200000000000", output_units: direction === "buy" ? "200000000000" : "100000000", fee_policy: { basis_units: "100000000" }, pay_by: "2026-10-12T12:10:00Z" } } as Trade;
  const d = { trade_id: `0x${"c".repeat(64)}`, terms_hash: `0x${"d".repeat(64)}`, buyer: direction === "buy" ? address("1") : address("2"), seller: direction === "buy" ? address("2") : address("1"), usdt_amount: "100000000", ever_amount: "200000000000", valid_until: Math.floor(Date.parse(trade.terms.pay_by) / 1000) };
  const escrow: Escrow = { deployment: d, policy, safe: address("5"), vault: address("6"), created_at: new Date(now).toISOString(), expires_at: new Date(now + 14400000).toISOString(), observed_at: new Date(now).toISOString(), state: "funded", consents: [], proposals: [], calls: [], evidence: [{ kind: "funding", valid: true }], typed_data: {
    domain: { name: "Pay3Flow OTC Escrow", version: "1", chainId: 1, verifyingContract: policy.factory }, primaryType: "EscrowConsent", types: { EscrowConsent: [
      {name:"tradeId",type:"bytes32"},{name:"termsHash",type:"bytes32"},{name:"buyer",type:"address"},{name:"seller",type:"address"},{name:"arbitrator",type:"address"},{name:"token",type:"address"},{name:"usdtAmount",type:"uint256"},{name:"everAmount",type:"uint256"},{name:"validUntil",type:"uint256"}
    ] }, message: { tradeId: d.trade_id, termsHash: d.terms_hash, buyer: d.buyer, seller: d.seller, arbitrator: policy.arbitrator, token: "0xdac17f958d2ee523a2206206994597c13d831ec7", usdtAmount: d.usdt_amount, everAmount: d.ever_amount, validUntil: d.valid_until.toString() }
  } };
  return { trade, escrow, now };
}

for (const direction of ["buy", "sell"] as const) test(`${direction} binds the correct USDT payer and EVER cap`, () => {
  const { trade, escrow } = fixture(direction);
  validateEscrow(trade, escrow); validateEscrowTypedData(trade, escrow, escrow.typed_data);
  const changed = structuredClone(escrow); changed.deployment.buyer = address("3");
  assert.throws(() => validateEscrow(trade, changed));
  const key = direction === "buy" ? "output_units" : "input_units";
  trade.terms[key] = MAX_EVER_UNITS.toString(); escrow.deployment.ever_amount = MAX_EVER_UNITS.toString(); validateEscrow(trade, escrow);
  trade.terms[key] = (MAX_EVER_UNITS + 1n).toString(); escrow.deployment.ever_amount = trade.terms[key]; assert.throws(() => validateEscrow(trade, escrow));
});

test("EVER handoff closes at the exact 30-minute boundary and on stale funding", () => {
  const { escrow, now } = fixture();
  assert.equal(everSigningAvailable(escrow, now), true);
  escrow.expires_at = new Date(now + 1800000).toISOString(); assert.equal(everSigningAvailable(escrow, now), false);
  escrow.expires_at = new Date(now + 1800001).toISOString(); assert.equal(everSigningAvailable(escrow, now), true);
  escrow.observed_at = new Date(now - 60000).toISOString(); assert.equal(everSigningAvailable(escrow, now), false);
  escrow.observed_at = new Date(now).toISOString(); escrow.evidence[0].valid = false; assert.equal(everSigningAvailable(escrow, now), false);
  escrow.evidence[0].valid = true; escrow.state = "refunded"; assert.equal(everSigningAvailable(escrow, now), false);
});

test("creation-time expiry cannot be extended or restarted at funding", () => {
  const { trade, escrow } = fixture(); escrow.expires_at = "2026-10-12T16:00:01Z";
  assert.throws(() => validateEscrow(trade, escrow), /four hours/);
});

for (const key of ["tradeId", "termsHash", "buyer", "seller", "arbitrator", "token", "usdtAmount", "everAmount", "validUntil"]) test(`deployment signature rejects altered ${key}`, () => {
  const { trade, escrow } = fixture(); const data = structuredClone(escrow.typed_data);
  data.message[key] = ["usdtAmount", "everAmount", "validUntil"].includes(key) ? "1" : address("4");
  assert.throws(() => validateEscrowTypedData(trade, escrow, data));
});

test("signature rejects another network or factory", () => {
  const { trade, escrow } = fixture(); const data = structuredClone(escrow.typed_data);
  data.domain.chainId = 56; assert.throws(() => validateEscrowTypedData(trade, escrow, data));
  data.domain.chainId = 1; data.domain.verifyingContract = address("4"); assert.throws(() => validateEscrowTypedData(trade, escrow, data));
});

test("signature rejects an altered EIP-712 field type", () => {
  const {trade,escrow}=fixture();
  const data=structuredClone(escrow.typed_data);
  data.types.EscrowConsent.find(field=>field.name==="usdtAmount")!.type="uint128";
  assert.throws(()=>validateEscrowTypedData(trade,escrow,data), /schema/);
});

test("Safe approval rejects arbitrary calls, reimbursement, and delegatecall", () => {
  const { trade, escrow } = fixture();
  const zero = address("0");
  const proposal = { action: "release", typed_data: { domain: {chainId:1,verifyingContract:escrow.safe!},primaryType:"SafeTx",types:{SafeTx:[
    {name:"to",type:"address"},{name:"value",type:"uint256"},{name:"data",type:"bytes"},{name:"operation",type:"uint8"},{name:"safeTxGas",type:"uint256"},{name:"baseGas",type:"uint256"},{name:"gasPrice",type:"uint256"},{name:"gasToken",type:"address"},{name:"refundReceiver",type:"address"},{name:"nonce",type:"uint256"}
  ]},message:{to:escrow.vault!,value:"0",data:"0x86d1a69f",operation:0,safeTxGas:"0",baseGas:"0",gasPrice:"0",gasToken:zero,refundReceiver:zero,nonce:"0"}} } as EscrowProposal;
  validateEscrowTypedData(trade, escrow, proposal.typed_data, proposal);
  for (const [key,value] of [["to",address("4")],["data","0x12345678"],["operation",1],["gasPrice","1"],["value","1"],["refundReceiver",address("4")]] as const) {
    const data=structuredClone(proposal.typed_data);data.message[key]=value;
    assert.throws(()=>validateEscrowTypedData(trade,escrow,data,proposal),`changed ${key}`);
  }
});
