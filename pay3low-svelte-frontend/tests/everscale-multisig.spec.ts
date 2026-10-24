import { test, expect, type Page } from "@playwright/test";
import { createHash } from "node:crypto";
import { SAFE_ABI } from "../src/lib/otc/everscale-multisig";

const agreement = { schema_version: 1, terms: { trade_id: "deal001", network_id: 42,
  buyer: { public_key: "11".repeat(32), address: `0:${"a".repeat(64)}` },
  seller: { public_key: "22".repeat(32), address: `0:${"b".repeat(64)}` },
  arbitrator: "33".repeat(32), deployment_key: "44".repeat(32), amount: "2000000000" },
  wallet_address: `0:${"c".repeat(64)}` };
const hash = createHash("sha256").update("evernode-otc/v1\0").update(JSON.stringify(agreement)).digest("hex");
const txHash = "e".repeat(64), callId = "00000000-0000-4000-8000-000000000001";
const session = { token: "test-token", actor: "https://relay.example/actors/buyer", desk: false, expires_at: "2099-01-01T00:00:00Z", signing_wallet: {chain:"everscale",address:agreement.terms.buyer.address}, wallets:[{chain:"everscale",address:agreement.terms.buyer.address,verified_at:new Date().toISOString()}] };

async function setup(page: Page, mode = "normal", signing = true) {
  const calls: any[] = []; let accepted = false, prepared = false;
  const pendingTx = mode === "confirmation" ? [{id:"18446744073709551615",action:"refund",destination:agreement.terms.seller.address,amount:agreement.terms.amount,confirmations_mask:"1"}] : [];
  const review = () => ({trade:agreement,terms_hash:hash,verified:true,balance_nanotokens:"3000000000",owners:[{index:2,public_key:agreement.terms.buyer.public_key},{index:0,public_key:agreement.terms.seller.public_key},{index:1,public_key:agreement.terms.arbitrator}],transactions:pendingTx,call:null,handoff:prepared && !accepted ? {id:callId,terms_hash:hash} : null});
  await page.addInitScript(({session,agreement,mode}) => {
    localStorage.setItem("pay3flow-locale", "en"); sessionStorage.setItem("pay3flow.otc.session.v1", JSON.stringify(session));
    const browser = window as any; browser.multisigWalletCalls = [];
    browser.__ever = {
      request: async ({method,params}:any) => {
        if (method === "getProviderState") return {selectedConnection:mode === "network" ? "testnet" : "mainnet",networkId:42,permissions:{accountInteraction:{address:agreement.terms.buyer.address,publicKey:agreement.terms.buyer.public_key}}};
        if (method === "requestPermissions") return {accountInteraction:{address:agreement.terms.buyer.address,publicKey:agreement.terms.buyer.public_key}};
        if (method === "sendExternalMessage") { browser.multisigWalletCalls.push(params); if (mode === "lost") throw new Error("Wallet result lost"); return {transaction:{id:{hash:"e".repeat(64)}}}; }
        throw new Error(`Unexpected wallet method ${method}`);
      }, addListener:()=>{},removeListener:()=>{},
    };
  }, {session,agreement,mode});
  await page.route("**/api/otc/**", async route => {
    const path = new URL(route.request().url()).pathname.replace("/api/otc", "");
    let result:unknown = {};
    if (path === "/config") result = {enabled:false,available:false,demo:false,everscale_multisig_available:signing};
    else if (path === "/session") result = session;
    else if (path === "/trades") result = {trades:[],rfqs:[]};
    else if (path === "/listings") result = [];
    else if (path === "/multisig/review") result = review();
    else if (path === "/multisig/prepare") {
      const body = route.request().postDataJSON(); calls.push(body); prepared = true;
      const op = body.operation;
      const params = op.transaction_id ? {transactionId:op.transaction_id} : {dest:agreement.terms[op.action === "release" ? "buyer" : "seller"].address,value:agreement.terms.amount,bounce:true,allBalance:false,payload:""};
      if (mode === "tamper") (params as any).dest = agreement.wallet_address;
      result = {id:callId,state:"unknown",reference:null,resumed:false,instructions:{request:body,review:{...review(),call:{publicKey:op.public_key,recipient:agreement.wallet_address,payload:{abi:SAFE_ABI,method:op.transaction_id ? "confirmTransaction" : "submitTransaction",params}}}}};
    } else if (path.endsWith("/reference")) { accepted = true; result = {id:callId,state:"accepted",settlement_delivery_verified:false}; }
    else if (path === `/multisig/calls/${callId}`) result = {id:callId,state:accepted ? "accepted" : "unknown",reference:accepted ? txHash : null,settlement_delivery_verified:false};
    await route.fulfill({json:result});
  });
  await page.goto("/#/otc");
  const panel = page.getByTestId("everscale-multisig"); await panel.locator("summary").click();
  await panel.getByLabel("Public OTC agreement JSON").fill(JSON.stringify(agreement));
  await panel.getByLabel("Independently reviewed terms hash").fill(hash);
  await panel.getByRole("button", {name:"Review public agreement",exact:true}).click();
  await expect(panel.getByText("2 EVER",{exact:true})).toBeVisible();
  return { panel, calls };
}

test("native agreement can be reviewed while desk and signing are unavailable", async ({page}) => {
  const {panel,calls} = await setup(page,"normal",false);
  await expect(panel.getByText(/signing is not configured/)).toBeVisible();
  await expect(panel.getByRole("button",{name:"Verify on Everscale"})).toHaveCount(0);
  expect(calls).toHaveLength(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});
test("release uses exact reviewed native EVER call and independently tracks receipt", async ({page}) => {
  const {panel,calls} = await setup(page);
  await panel.getByRole("button",{name:"Verify on Everscale"}).click();
  await panel.getByRole("button",{name:"Review release",exact:true}).click();
  expect(calls).toHaveLength(0);
  await panel.getByRole("button",{name:"Confirm in EVER Wallet",exact:true}).click();
  await expect(panel.getByText(/Original wallet call accepted/)).toBeVisible();
  const sent = await page.evaluate(() => (window as any).multisigWalletCalls);
  expect(sent).toHaveLength(1); expect(sent[0].recipient).toBe(agreement.wallet_address);
  expect(sent[0].payload.params).toEqual({dest:agreement.terms.buyer.address,value:"2000000000",bounce:true,allBalance:false,payload:""});
  await expect(panel.getByText(/Recipient delivery is not yet verified/)).toBeVisible();
});
test("refund approval preserves full uint64 transaction ID", async ({page}) => {
  const {panel} = await setup(page,"confirmation");
  await panel.getByRole("button",{name:"Verify on Everscale"}).click();
  await panel.getByRole("button",{name:"Review second approval"}).click();
  await expect(panel.getByText("ID: 18446744073709551615",{exact:true})).toBeVisible();
  await panel.getByRole("button",{name:"Confirm in EVER Wallet",exact:true}).click();
  await expect.poll(async () => (await page.evaluate(() => (window as any).multisigWalletCalls)).length).toBe(1);
  const sent = await page.evaluate(() => (window as any).multisigWalletCalls[0]);
  expect(sent.payload).toMatchObject({method:"confirmTransaction",params:{transactionId:"18446744073709551615"}});
});
test("lost wallet response survives reload and never resends", async ({page}) => {
  const {panel,calls} = await setup(page,"lost");
  await panel.getByRole("button",{name:"Verify on Everscale"}).click();
  await panel.getByRole("button",{name:"Review refund",exact:true}).click();
  await panel.getByRole("button",{name:"Confirm in EVER Wallet",exact:true}).click();
  await expect(panel.getByRole("alert")).toContainText("Wallet result lost");
  await page.reload(); await panel.locator("summary").click();
  await expect(panel.getByText(/Original handoff pending or unresolved/)).toBeVisible();
  await expect(panel.getByRole("button",{name:"Confirm in EVER Wallet",exact:true})).toHaveCount(0);
  await expect(panel.getByRole("button",{name:"Continue original wallet review"})).toHaveCount(0);
  await panel.getByLabel("Original Everscale transaction hash").fill(txHash);
  await panel.getByRole("button",{name:"Reconcile original call"}).click();
  await expect(panel.getByText(/Original wallet call accepted/)).toBeVisible();
  expect(calls).toHaveLength(1);
  expect(await page.evaluate(() => (window as any).multisigWalletCalls)).toHaveLength(0);
});
test("tampered call is blocked before a wallet signature", async ({page}) => {
  const {panel} = await setup(page,"tamper");
  await panel.getByRole("button",{name:"Verify on Everscale"}).click();
  await panel.getByRole("button",{name:"Review release",exact:true}).click();
  await panel.getByRole("button",{name:"Confirm in EVER Wallet",exact:true}).click();
  await expect(panel.getByRole("alert")).toContainText("Transfer differs");
  expect(await page.evaluate(() => (window as any).multisigWalletCalls)).toHaveLength(0);
});
