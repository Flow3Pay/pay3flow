import { test } from "node:test";
import assert from "node:assert/strict";
import { isTonAddress, isValidWalletAddress } from "../src/lib/wallet-address.ts";

test("OTC receiving addresses are validated per network", () => {
  const valid: Array<[string, string]> = [
    ["ethereum", "0x1d249110d194c792104dCa35B9029db5f1b9652F"],
    ["sei", "0xc791077f8a026e134b43d19d0151563f317d7448"],
    ["bnb-smart-chain", "0x1d249110d194c792104dCa35B9029db5f1b9652F"],
    ["near", "89c37ba0641b7458b6010cd4ac1ec080c53318adbb5410d7320b998c64f4b806"],
    ["near", "alice.near"],
    ["tron", "TNsUNXyXyd9H3A4Yu5iXBi5Vu9K5gqixz9"],
    ["solana", "GPcpfFvrycKRemZR4DHKAFVenxtoRhTVjJ5S4AxUjhBU"],
    ["ton", "UQC4PWxCucg-jsDID6jHrvaEFQL0CY9N3Nq4jpLSBXanotqc"],
    ["ton", `0:${"a".repeat(64)}`],
    ["sui", "0x561148c7d7ed5dc33f2134beb38736a15d866b221e43c8671c58493c681c6183"],
    ["aptos", "0xa60f23997ab5853d18d340d8a4a46cb4917ae54316d2fe8827b08069e07e57d5"],
  ];
  for (const [network, address] of valid) assert.equal(isValidWalletAddress(network, address), true, `${network} ${address}`);

  const invalid: Array<[string, string]> = [
    ["ethereum", "0x0000000000000000000000000000000000000000"],
    ["ethereum", "GPcpfFvrycKRemZR4DHKAFVenxtoRhTVjJ5S4AxUjhBU"],
    ["sei", "sei1c7gswlu2qfhpxj6r6xwsz52k8uch6azgpv378y"],
    ["solana", "0x1d249110d194c792104dCa35B9029db5f1b9652F"],
    ["tron", "T0sUNXyXyd9H3A4Yu5iXBi5Vu9K5gqixz9"],
    ["ton", "UQC4PWxCucg-jsDID6jHrvaEFQL0CY9N3Nq4jpLSBXanotqd"],
    ["near", "Alice.NEAR"],
    ["sui", `0x${"0".repeat(64)}`],
    ["aptos", "0x0"],
    ["bitcoin", "bc1qkt5mw3c7jvedwtkcgf2ktdte2p23as04d2ypuf"],
  ];
  for (const [network, address] of invalid) assert.equal(isValidWalletAddress(network, address), false, `${network} ${address}`);
});

test("TON accepts both the raw and the user-friendly form with a valid checksum", () => {
  assert.equal(isTonAddress(`-1:${"b".repeat(64)}`), true);
  assert.equal(isTonAddress(`-1:${"b".repeat(63)}`), false);
  assert.equal(isTonAddress(`2:${"b".repeat(64)}`), false);
  assert.equal(isTonAddress("EQC4PWxCucg-jsDID6jHrvaEFQL0CY9N3Nq4jpLSBXanotqc"), false);
});
