import assert from "node:assert/strict";
import test from "node:test";
import { evmNetworkFor, p2pEvmNetworkFor, walletNetworkName } from "../src/lib/otc/network.ts";

test("the OTC wallet follows the Ethereum network published by the backend", () => {
  assert.deepEqual(evmNetworkFor(undefined), { name: "ethereum", label: "Ethereum mainnet", chainId: 1 });
  assert.deepEqual(evmNetworkFor(1), { name: "ethereum", label: "Ethereum mainnet", chainId: 1 });
  assert.deepEqual(evmNetworkFor(8453), { name: "base", label: "Base", chainId: 8453 });
  assert.deepEqual(evmNetworkFor(42161), { name: "arbitrum-one", label: "Arbitrum One", chainId: 42161 });
});

test("testnets and unconfigured Ethereum chains stop OTC instead of signing elsewhere", () => {
  assert.throws(() => evmNetworkFor(5), /OTC settlement network 5 is not supported/);
  assert.throws(() => evmNetworkFor(0), /OTC settlement network 0 is not supported/);
});

test("only the Ethereum label is remapped, Everscale stays its own network", () => {
  assert.equal(walletNetworkName("ethereum", evmNetworkFor(1)), "ethereum");
  assert.equal(walletNetworkName("base", evmNetworkFor(8453)), "base");
  assert.equal(walletNetworkName("everscale", evmNetworkFor(1)), "everscale");
});

test("the peer settlement network is explicit and unsupported networks fail closed", () => {
  assert.throws(
    () => p2pEvmNetworkFor({ ethereum_chain_id: 1, p2p_ethereum_chain_id: 5 }),
    /OTC settlement network 5 is not supported/,
  );
  assert.throws(() => p2pEvmNetworkFor({ ethereum_chain_id: 5 }), /OTC settlement network 5 is not supported/);
  assert.deepEqual(p2pEvmNetworkFor({}), { name: "ethereum", label: "Ethereum mainnet", chainId: 1 });
});
