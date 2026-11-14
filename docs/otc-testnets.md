# OTC testnet settlement

This branch has an isolated Sepolia-only P2P mode for testing actual USDT/USDC
ERC-20 sends and on-chain receipt verification. It never uses mainnet stablecoin
contracts, charges no service fee, and never sends EVER. A successful test deal
is reported as `testnet_completed` with `ever_delivery: "skipped_testnet"`; it
must not be treated as a completed production trade.

`TestStablecoin` is an unrestricted-mint ERC-20 with configurable decimals.
Deploy it only to testnets: anybody can mint arbitrary balances, and the token
has no value or issuer relationship to USDT or USDC.

## Deploying test USDT and USDC to EVM testnets

Build the artifact first:

```sh
cd contracts/otc
npm ci
npm run build
```

Set a dedicated test-only deployer key, faucet recipient, and testnet RPC URLs.
The helper rejects chain IDs other than the listed testnets and checks each RPC
before broadcasting. The deployer needs that network's native faucet gas:

| `TESTNETS` name | Chain ID | Required RPC variable |
| --- | ---: | --- |
| `sepolia` | 11155111 | `SEPOLIA_RPC_URL` |
| `optimism_sepolia` | 11155420 | `OPTIMISM_SEPOLIA_RPC_URL` |
| `bnb_testnet` | 97 | `BNB_TESTNET_RPC_URL` |
| `gnosis_chiado` | 10200 | `GNOSIS_CHIADO_RPC_URL` |
| `polygon_amoy` | 80002 | `POLYGON_AMOY_RPC_URL` |
| `avalanche_fuji` | 43113 | `AVALANCHE_FUJI_RPC_URL` |
| `arbitrum_sepolia` | 421614 | `ARBITRUM_SEPOLIA_RPC_URL` |
| `base_sepolia` | 84532 | `BASE_SEPOLIA_RPC_URL` |
| `sei_atlantic_2` | 1328 | `SEI_ATLANTIC_2_RPC_URL` |

Example (use a new wallet with no mainnet funds or permissions):

```sh
export TESTNET_PRIVATE_KEY=0x...
export TEST_TOKEN_HOLDER=0x...
export TESTNETS=sepolia,base_sepolia
export SEPOLIA_RPC_URL=https://...
export BASE_SEPOLIA_RPC_URL=https://...
npm run deploy:test-stablecoins
```

The script deploys six-decimal USDT and USDC contracts with an initial supply
of one million tokens each. `TEST_TOKEN_SUPPLY` can change the whole-token
supply. Keep the deployer key local; never add it to configuration committed
to Git.

## Other testnet tokens

These are independent test assets, not real USDT/USDC. They are not wired into
the OTC application's settlement flow.

| Network | Token | Test asset address / type | Notes |
| --- | --- | --- | --- |
| TRON Nile | USDT | `TF4YzV4a2c5LMrwbQ3WbpJi57yZF9uuGyi` | TRC-20, 6 decimals, public `mint(address,uint256)` |
| TRON Nile | USDC | `TYPQgdQwj3Mi6pXaLHtbXorPtMjF3Zn3Rz` | TRC-20, 6 decimals, public `mint(address,uint256)` |
| TON testnet | USDT | `0QA6PBa61VPXDscE_38ZNFiVYRc5l-uyD_xHQwppzNdsepGK` | Jetton minter; mint authority is held by the deployer |
| TON testnet | USDC | `0QAWjUSL-6xviwDtM1NYA2mhApfyXRh82_XNM1s6Ghtu9w0H` | Jetton minter; mint authority is held by the deployer |
| Aptos testnet | USDT | `0x6615ec77a988cf8f64debf9170ec3b6d5812555ead2196cbb4a64e272031eaae::usdt::USDT` | 6 decimals; public `usdt::mint(&signer, amount)` |
| Aptos testnet | USDC | `0x6615ec77a988cf8f64debf9170ec3b6d5812555ead2196cbb4a64e272031eaae::usdc::USDC` | 6 decimals; public `usdc::mint(&signer, amount)` |
| Sui testnet | USDC | `0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC` | Circle test asset; request it at [Circle Testnet Faucet](https://faucet.circle.com/) |

The Aptos package source is in `contracts/testnet-tokens/aptos`. Its public
mint functions accept atomic units (for example, `1000000` mints one token).
The Aptos publish transaction is
[0x31f7f801…](https://explorer.aptoslabs.com/txn/0x31f7f8010c5da2981800279138381f70d057e9d5d718150244c79166f41f0672?network=testnet);
public mint was exercised for both assets ([USDT](https://explorer.aptoslabs.com/txn/0x8be09783dd899abf303f918a2bb05102c01fdd96ceeb420b7af779d895e9763a?network=testnet),
[USDC](https://explorer.aptoslabs.com/txn/0x74a50011309611bb779af9cfbc749e4f34eed506ffa7ee9ccc24b51eabac2c4c?network=testnet)).
Sui's Circle faucet requires a human to complete its CAPTCHA. No custom Sui
USDT has been deployed.

TON minters are admin-controlled; tokens can be transferred to a user's
address, but the deployer's signing key must stay private. Aptos and TRON
test tokens can be minted directly to a test wallet by calling their public
mint functions. Test tokens have no market value and must never be treated as
real stablecoins.

## Enabling isolated Sepolia P2P mode

Use only a test deployment and a test database. Set `testnet_mode` and
`testnet_skip_ever` to `true`, leave standard OTC `enabled` false, and configure
both `ethereum_rpc` and `p2p_ethereum_rpc` with two independent HTTPS Sepolia
RPC URLs. Enable `public_offers_enabled` and `p2p_enabled` only on that test
deployment. The backend rejects any non-Sepolia chain ID or token contract and
does not expose other direct-payment assets in this mode.

The relevant `OTC_CONFIG_FILE` values are:

```json
{
  "enabled": false,
  "public_offers_enabled": true,
  "p2p_enabled": true,
  "testnet_mode": true,
  "testnet_skip_ever": true,
  "ethereum_chain_id": 11155111,
  "usdt_contract": "0xf9c9730cb703512a754a4dd8bffa640a3aa670e2",
  "p2p_ethereum_chain_id": 11155111,
  "p2p_usdt_contract": "0xf9c9730cb703512a754a4dd8bffa640a3aa670e2",
  "p2p_usdc_contract": "0x2b11dbbb6c8dab48588cc13e8b20ab4d431f5e9e",
  "ethereum_rpc": ["https://sepolia-rpc-a.example", "https://sepolia-rpc-b.example"],
  "p2p_ethereum_rpc": ["https://sepolia-rpc-a.example", "https://sepolia-rpc-b.example"]
}
```

Keep the existing identity/relay settings needed for wallet login and offers.
The wallet doing the payment must be connected to Sepolia and funded with
testnet ETH for gas. The OTC service validates the actual ERC-20 transfer on
Sepolia. Test mode does not exercise NEAR Intents, does not reserve or transfer
EVER, and is not safe to expose as a production OTC market.

The standard OTC quote path remains disabled in this mode. This feature is
limited to Sepolia P2P offers; it does not enable the separately deployed
TRON/TON/Aptos/Sui tokens for OTC settlement.

Solana, NEAR, and native Everscale test assets are not provided here. NEAR
Intents testnet swap support is outside this setup; test tokens on independent
networks do not create an executable cross-chain route.
