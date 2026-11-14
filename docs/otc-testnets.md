# OTC testnet token preparation

This branch adds an EVM-only faucet token and a deployment helper for isolated
EVM testnets. It does **not** enable testnets in the OTC backend and does not
make the current production OTC flow safe to use with test assets.

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

## Remaining testnet work

The OTC backend still binds settlement to Everscale mainnet and Ethereum
mainnet, and intentionally rejects testnet offers. A testnet OTC mode needs
separate wallet-login, chain-observer, fee, and Everscale testnet wiring before
it can accept or confirm payments. This EVM token deployment helper alone
cannot make those trades safe.

The other supported wallet families do not share an ERC-20 deployment
mechanism: TRON, Solana, NEAR, Sui, TON, Aptos, and native Everscale test
assets need chain-specific contracts/programs, testnet RPCs, wallet network
support, and observers. NEAR Intents testnet swap support is outside this setup;
test tokens on independent networks do not create an executable cross-chain
route.
