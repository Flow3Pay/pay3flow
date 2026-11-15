import assert from "node:assert/strict";
import test from "node:test";
import { mobileWalletLinks } from "../src/lib/mobile-wallet-links.ts";

test("mobile wallet links preserve the current route and query", () => {
  const links = mobileWalletLinks("https://pay3flow.example/swap/ETH/USDC?amount=2#review");
  assert.deepEqual(links.map(link => link.name), ["MetaMask", "Trust Wallet", "Coinbase Wallet"]);
  assert.equal(links[0].href, "https://metamask.app.link/dapp/pay3flow.example/swap/ETH/USDC?amount=2#review");
  assert.equal(new URL(links[1].href).searchParams.get("url"), "https://pay3flow.example/swap/ETH/USDC?amount=2#review");
  assert.equal(new URL(links[2].href).searchParams.get("cb_url"), "https://pay3flow.example/swap/ETH/USDC?amount=2#review");
});
