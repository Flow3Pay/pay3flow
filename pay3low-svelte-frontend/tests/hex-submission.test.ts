import assert from "node:assert/strict";
import test from "node:test";
import {Submissions,ConfirmedFailure,withAccountLock,type Relay} from "../src/lib/hex/submission.ts";

const signed={standard:"erc191",signature:"signature",payload:"original-unique-nonce"};
function setup(overrides:Partial<Relay>={}) {
  const values=new Map<string,string>(),published:unknown[]=[];
  const storage={getItem:(k:string)=>values.get(k)??null,setItem:(k:string,v:string)=>{values.set(k,v);},removeItem:(k:string)=>{values.delete(k);}};
  const relay:Relay={publish:async s=>{published.push(s);return {status:"OK",intent_hashes:["intent-1"]};},status:async()=>({status:"SETTLED",data:{hash:"transaction-1"}}),verifyTransaction:async()=>{},...overrides};
  return {journal:new Submissions(storage,relay),storage,relay,published};
}
test("wallet rejection and failed local persistence never publish",async()=>{
  const {journal,published}=setup();
  await assert.rejects(journal.submit("alice.near",async()=>{throw new Error("Rejected");}));
  assert.equal(journal.pending("alice.near"),null);assert.equal(published.length,0);
  const broken=new Submissions({getItem:()=>null,setItem:()=>{throw new Error("Storage blocked");},removeItem:()=>{}},{publish:async()=>{throw new Error("Must not publish");},status:async()=>({status:"PENDING"}),verifyTransaction:async()=>{}});
  await assert.rejects(broken.submit("alice.near",async()=>signed),/Storage blocked/);
});
test("lost publication response survives reload and retries the identical signature",async()=>{
  let first=true;const calls:unknown[]=[];
  const {journal,storage,relay}=setup({publish:async s=>{calls.push(s);if(first){first=false;throw new Error("Lost response");}return {status:"OK",intent_hashes:["original-intent"]};}});
  await assert.rejects(journal.submit("alice.near",async()=>signed),/Lost response/);
  await assert.rejects(journal.submit("alice.near",async()=>{throw new Error("Must not sign again");}),/original/);
  const restored=new Submissions(storage,relay);
  assert.equal(await restored.reconcile("alice.near"),"transaction-1");
  assert.deepEqual(calls,[signed,signed]);assert.equal(restored.pending("alice.near"),null);
});
test("pending settlement blocks new orders and account changes cannot reuse it",async()=>{
  const {journal,relay,published}=setup({status:async()=>({status:"PENDING"})});
  assert.equal(await journal.submit("alice.near",async()=>signed),null);
  assert.equal(journal.pending("bob.near"),null);
  await assert.rejects(journal.submit("alice.near",async()=>signed));
  assert.equal(journal.pending("alice.near")?.intentHash,"intent-1");
  relay.status=async()=>({status:"SETTLED",data:{hash:"tx"}});
  assert.equal(await journal.reconcile("alice.near"),"tx");assert.equal(published.length,1);
});
test("replay rejection preserves unknown original submission",async()=>{
  let response=false;
  const {journal}=setup({publish:async()=>{if(!response){response=true;throw new Error("Timeout");}return {status:"FAILED",reason:"Nonce used"};}});
  await assert.rejects(journal.submit("alice.near",async()=>signed));
  await assert.rejects(journal.reconcile("alice.near"),/Nonce used/);
  assert.ok(journal.pending("alice.near"));
});
test("receipt transport failure preserves reference, confirmed failure clears it",async()=>{
  const {journal,relay}=setup({verifyTransaction:async()=>{throw new Error("RPC unavailable");}});
  await assert.rejects(journal.submit("alice.near",async()=>signed));
  assert.equal(journal.pending("alice.near")?.transaction,"transaction-1");
  relay.verifyTransaction=async()=>{throw new ConfirmedFailure("Contract reverted");};
  await assert.rejects(journal.reconcile("alice.near"),/Contract reverted/);
  assert.equal(journal.pending("alice.near"),null);
});
test("concurrent reconciliation shares one operation; signing is not duplicated",async()=>{
  let finish!:()=>void,signs=0;
  const {journal,published}=setup();
  const operation=journal.submit("alice.near",async()=>{signs++;await new Promise<void>(r=>finish=r);return signed;});
  await assert.rejects(journal.submit("alice.near",async()=>signed));
  const checking=journal.reconcile("alice.near");finish();
  assert.deepEqual(await Promise.all([operation,checking]),["transaction-1","transaction-1"]);
  assert.equal(signs,1);assert.equal(published.length,1);
});
test("browser lock serializes reconciliation and new submissions across tabs",async()=>{
  let held=false,finish!:()=>void;
  const locks={request:async(_name:string,_options:unknown,callback:(lock:unknown)=>Promise<unknown>)=>{
    if(held)return callback(null);
    held=true;try{return await callback({name:"account"});}finally{held=false;}
  }} as unknown as Pick<LockManager,"request">;
  const reconciling=withAccountLock(locks,"alice.near",()=>new Promise<string>(resolve=>finish=()=>resolve("confirmed")));
  await assert.rejects(withAccountLock(locks,"alice.near",async()=>"new order"),/Another tab/);
  finish();assert.equal(await reconciling,"confirmed");
  assert.equal(await withAccountLock(locks,"alice.near",async()=>"new order"),"new order");
  await assert.rejects(withAccountLock(undefined,"alice.near",async()=>"unsafe"),/Web Locks/);
});
