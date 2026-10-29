import assert from "node:assert/strict";
import test from "node:test";
import { evmNetworkFor, p2pEvmNetworkFor, walletNetworkName } from "../src/lib/otc/network.ts";

test("the OTC wallet follows the Ethereum network published by the backend", () => {
  assert.deepEqual(evmNetworkFor(undefined), { name: "ethereum", label: "Ethereum mainnet", chainId: 1 });
  assert.deepEqual(evmNetworkFor(1), { name: "ethereum", label: "Ethereum mainnet", chainId: 1 });
  assert.deepEqual(evmNetworkFor(11155111), { name: "sepolia", label: "Sepolia testnet", chainId: 11155111 });
});

test("an unconfigured Ethereum chain stops the flow instead of signing elsewhere", () => {
  assert.throws(() => evmNetworkFor(5), /not available in this wallet integration/);
  assert.throws(() => evmNetworkFor(0), /not available in this wallet integration/);
});

test("only the Ethereum label is remapped, Everscale stays its own network", () => {
  const sepolia = evmNetworkFor(11155111);
  assert.equal(walletNetworkName("ethereum", sepolia), "sepolia");
  assert.equal(walletNetworkName("everscale", sepolia), "everscale");
  assert.equal(walletNetworkName("ethereum", evmNetworkFor(1)), "ethereum");
});

test("the desk keeps its network while peers settle on another one", () => {
  assert.deepEqual(p2pEvmNetworkFor({ ethereum_chain_id: 1, p2p_ethereum_chain_id: 11155111 }), { name: "sepolia", label: "Sepolia testnet", chainId: 11155111 });
  assert.deepEqual(p2pEvmNetworkFor({ ethereum_chain_id: 11155111 }), { name: "sepolia", label: "Sepolia testnet", chainId: 11155111 });
  assert.deepEqual(p2pEvmNetworkFor({}), { name: "ethereum", label: "Ethereum mainnet", chainId: 1 });
});
