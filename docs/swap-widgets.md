# Unified SWAP interface

Every wallet swap chapter renders `UnifiedSwap.svelte` and the same
`SwapForm.svelte`. The form owns the amount, asset cards and application theme.
There are no provider iframes or provider swap components. `SWAP guide` keeps
this form mounted while showing the walkthrough; changing chapters selects
that chapter’s exact assets, networks and input amount.

`swap-service.ts` creates service adapters for steps without a signed execution
descriptor. A descriptor selects the existing `RouteExecutionPanel.svelte`
controller, which uses the same shared form. Connecting a wallet never submits
a swap. The user explicitly starts the swap and confirms wallet requests.

| Provider | Service execution |
| --- | --- |
| CoW Swap | Trading SDK for same-chain ERC20 and native currency orders; Bridging SDK with the NEAR Intents bridge for cross-chain quotes and orders |
| NEAR Intents | Aurora SDK hooks for token metadata, quotes, wallet transfers and status; the React service host returns `null`, never `WidgetSwap` |
| Symbiosis | Backend executable quotes, configured router and approval gateway validation, shared wallet signing, and backend status tracking |

An unavailable wallet adapter or unsupported provider is reported inside the
common form. There is no fallback to an external swap application. Availability
still depends on provider liquidity, configured contracts and wallet support.
0x and Barter quote credentials do not by themselves enable wallet execution.

## Amounts and signing

`route-tutorial.ts` supplies exact `fromAsset`, `toAsset`, and `inputAmount`
values for composed legs. For BTC → USDC@ethereum → EVER@ethereum, the CoW step
uses the intermediate USDC amount rather than the original BTC amount. The
readonly amount field preserves the full decimal string.

Preparations bind the source wallet, recipient and quote expiry. Recipient or
account changes invalidate a preparation. Token approvals require a refreshed
quote. Native CoW orders use the EthFlow transaction built from the reviewed
quote. Cross-chain tracking waits for bridge execution, not only the CoW leg.
Symbiosis submission retries retain the broadcast reference to avoid signing a
second transfer after a notification failure.

Steps without descriptors request fresh backend execution through
`POST /api/p2p/route-executions/prepare`. Client preferences never supply
calldata or approval targets. The endpoint checks the provider, assets, amount,
wallet addresses and configured execution contracts, then uses the same quote
and transaction validation as signed search descriptors. It requires an
`Idempotency-Key` and respects `wallet_execution_enabled`.

## NEAR SDK configuration

The browser service uses `PUBLIC_AURORA_INTENTS_API_KEY`, supplied to the
frontend Node process at runtime (`$env/dynamic/public`). Kubernetes references
this value from `pay3flow-secrets`. Backend `NEAR_INTENTS_JWT` remains server-only.
Wallet selection may use the SDK’s wallet dialog; the swap itself always uses
Pay3Flow’s form. The SDK can also prepare an external deposit with explicit
recipient and refund addresses, including required deposit memos.

NEAR allowlists include catalog `wNEAR`, and USDT allowlists include `USDT0`.
These aliases are handled before seeding the exact source amount. Provider
catalogs determine contract addresses and precision; native EVER and its
Ethereum representation remain distinct.

## Verification

Use `npm run check` and the desktop/mobile tests in
`tests/wallet-execution.spec.ts`. Coverage includes routes with and without
execution descriptors, absence of provider iframes, mode preservation, exact
composed-leg amounts, NEAR aliases, explicit swap intent, approval refreshes,
and retrying tracking without signing twice.

Build with `NODE_OPTIONS=--max-old-space-size=6144`. Live deployment uses
`deploy/frontend-runtime/Dockerfile` and its filtered runtime dependency tree.
