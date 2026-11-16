import { test } from "node:test";
import assert from "node:assert/strict";
import {
  connectedWalletAddress,
  destinationAddressFor,
  preferredDestinationAsset,
  walletFamilyForNetwork,
} from "../src/lib/wallet-address.ts";

const evm = `0x${"b".repeat(40)}`;
const ton = "UQC4PWxCucg-jsDID6jHrvaEFQL0CY9N3Nq4jpLSBXanotqc";
const solana = "GPcpfFvrycKRemZR4DHKAFVenxtoRhTVjJ5S4AxUjhBU";
const assets = [
  { symbol: "USDT" as const, network: "ethereum", asset_id: "eth-usdt" },
  { symbol: "USDT" as const, network: "avalanche-c", asset_id: "avax-usdt" },
  { symbol: "USDT" as const, network: "ton", asset_id: "ton-usdt" },
  { symbol: "USDT" as const, network: "solana", asset_id: "sol-usdt" },
  { symbol: "USDC" as const, network: "ethereum", asset_id: "eth-usdc" },
];

test("each OTC receiving network maps to its wallet family", () => {
  assert.equal(walletFamilyForNetwork("ethereum"), "evm");
  assert.equal(walletFamilyForNetwork("avalanche-c"), "evm");
  assert.equal(walletFamilyForNetwork("ton"), "ton");
  assert.equal(walletFamilyForNetwork("solana"), "solana");
  assert.equal(walletFamilyForNetwork("tron"), "tron");
  assert.equal(walletFamilyForNetwork("unknown"), "");
});

test("a connected non-EVM wallet is reused for its receiving network", () => {
  const wallets = {
    ton: { family: "ton", address: ton },
    solana: { family: "solana", address: solana },
  };
  assert.equal(connectedWalletAddress("ton", wallets), ton);
  assert.equal(destinationAddressFor("ton", wallets), ton);
  assert.equal(destinationAddressFor("solana", wallets), solana);
  assert.equal(destinationAddressFor("near", wallets), "");
  assert.equal(preferredDestinationAsset(assets, "USDT", wallets)?.network, "ton");
});

test("an EVM account is reused on every EVM network without a chain switch", () => {
  const wallets = { evm: { family: "evm", address: evm } };
  assert.equal(destinationAddressFor("ethereum", wallets), evm);
  assert.equal(destinationAddressFor("avalanche-c", wallets), evm);
  assert.equal(destinationAddressFor("polygon-pos", wallets), evm);
  assert.equal(destinationAddressFor("ton", wallets), "");
});

test("the verified EVM address remains the fallback only on EVM networks", () => {
  assert.equal(destinationAddressFor("avalanche-c", {}, evm), evm);
  assert.equal(destinationAddressFor("ton", {}, evm), "");
  assert.equal(preferredDestinationAsset(assets, "USDT", {})?.network, "ethereum");
  assert.equal(preferredDestinationAsset(assets, "USDC", {})?.network, "ethereum");
  assert.equal(preferredDestinationAsset(assets, "DAI", {}), null);
});
