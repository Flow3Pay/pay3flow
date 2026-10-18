# SWAP widgets and wallet execution

Verified against the repository on 2026-10-18. The frontend is SvelteKit;
the NEAR Intents React widget is a client-mounted island.

## Route actions

Every swap chapter offers `SWAP` and `SWAP guide`. The result card can open
either mode, and shared guide links preserve `view=swap|guide`. Switching modes
hides the live panel while keeping it mounted, preserving its form and wallet
state. Moving to another chapter selects that chapter's provider and assets.

Wallet support in the site header does not determine whether a swap chapter
appears. A route without a Pay3Flow `execution` descriptor can still open the
provider widget or provider website. Connecting a wallet does not prepare a
transaction, create a route execution, approve a token, or submit a swap.

| Provider or step | SWAP behavior |
| --- | --- |
| CoW Swap | Official `@cowprotocol/widget-lib` iframe, including composed-route legs and routes without a native execution descriptor |
| NEAR Intents | Official Aurora standalone React widget with its own wallet connection flow |
| Symbiosis | Native Pay3Flow execution panel when supported; otherwise the public app iframe, independent of site-header Solana wallet support |
| Other providers and spot steps | Provider embed where supported, otherwise the selected quote and a link to the provider |

Provider frame restrictions and browser third-party storage policies can prevent
an external app from working in an iframe. An external Open link remains
available. A displayed quote does not establish that an external provider
permits embedded execution. In particular, 0x and Barter retain external links.

## Assets and amounts

[`route-tutorial.ts`](../pay3low-svelte-frontend/src/lib/route-tutorial.ts)
provides `fromAsset`, `toAsset` and the exact `inputAmount` for each composed leg.
Widgets use these fields before whole-route fallbacks. For
`BTC → USDC@ethereum → EVER@ethereum`, the second CoW widget receives that
leg's USDC amount and Ethereum network, rather than the original BTC amount.
Human-readable rounded chapter amounts are not authoritative widget inputs.

Provider token catalogs resolve addresses, decimals and network-qualified
identities. Native EVER and its Ethereum representation remain separate.
Legacy TON endpoints and asset-bearing fields in shared operation signatures
canonicalize to GRAM; provider IDs, advertiser IDs and network `ton` remain intact.

## CoW Swap

[`CowSwapWidget.svelte`](../pay3low-svelte-frontend/src/lib/components/CowSwapWidget.svelte)
passes the selected tokens, source and target chain IDs, amount, language and
theme to the official widget. Supported shared EVM sessions are bridged through
the origin-checked widget transport. Other supported widget networks use CoW's
wallet flow. CoW handles its own quotes, approvals, signing and tracking.

The iframe uses `color-scheme: normal` so the browser's iframe canvas does not
introduce a white background when CoW renders its dark theme. The provider's
theme setting and the enclosing site theme are synchronized separately.

## NEAR Intents

[`NearIntentsWidget.svelte`](../pay3low-svelte-frontend/src/lib/components/NearIntentsWidget.svelte)
lazy-loads [`near-widget.ts`](../pay3low-svelte-frontend/src/lib/near-widget.ts),
which mounts React with `createRoot`, `WidgetConfigProvider` and `WidgetSwap`
from `@aurora-is-near/intents-swap-widget-standalone`.

The standalone package supplies its EVM/Solana, NEAR and Stellar wallet flows.
These sessions are separate from the shared Pay3Flow header wallets. Route
assets and chains are constrained in widget configuration; direction is locked.
`NEAR` allowlists include catalog `wNEAR`, and `USDT` allowlists include `USDT0`,
because the SDK filters catalog symbols before normalizing display aliases.
The exact source amount is seeded after the selected tokens are ready. The
SDK may abbreviate the visible input to five decimals; its displayed text
should not be used as an application amount serializer.

The Aurora widget integration key is `PUBLIC_AURORA_INTENTS_API_KEY`:

- Local development: set it in ignored `.env.local`.
- Production: supply it to the frontend Node process at runtime. It is read
  through `$env/dynamic/public`; a build-time-only setting is insufficient.
- Kubernetes: store it in `pay3flow-secrets` and reference that key from the
  frontend Deployment's `env`.

Obtain the key from [Aurora Widget Studio](https://studio.aurora.dev/).
The [official package repository](https://github.com/aurora-is-near/intents-swap-widget)
documents standalone integration and chain adapters. This browser integration
key is separate from backend `NEAR_INTENTS_JWT`, which authenticates server-side
1Click requests. Server JWTs never belong in `PUBLIC_` variables.

## Symbiosis

[`swap-embed.ts`](../pay3low-svelte-frontend/src/lib/swap-embed.ts) resolves the
selected tokens and chains against Symbiosis metadata and constructs an app
URL with the current leg's input. Embedding does not require the site header
to support every source/destination wallet family. Native execution remains
limited to chains and contracts accepted by the backend validator.

Some browsers block the external app's third-party storage and cause its
embedded flow to fail. Use the supplied Open link in that case. Widget
availability and provider liquidity are separate: neither guarantees the other.

## Native Pay3Flow execution

`wallet_execution_enabled` controls signed backend execution descriptors and
the `/api/p2p/route-executions` flow. Native NEAR Intents deposits support the
implemented EVM, NEAR and TRON actions; native CoW actions validate supported
same-chain ERC-20 orders; native Symbiosis validates configured routers and
approval gateways. Everscale wallet connection alone does not add native EVER
swap execution. See [configuration](configuration.md#wallet-swaps).

Provider-owned widgets have their own execution and tracking. A swap submitted
inside CoW or Aurora's widget is not automatically a Pay3Flow `route_executions`
record. Neither guide animations nor route selection submit a transaction.

`@0x/swap-ts-sdk` is installed for server integrations. Its installation does
not provide an in-page 0x execution form. Backend `ZEROX_API_KEY` and
`BARTER_API_KEY` remain server-only quote credentials.

## Build and verification

The Aurora standalone SDK is lazy-loaded and isolated from the legacy NEAR
Wallet Selector's React dependency. Browser Node polyfills and explicit dev
dependency optimization are configured in
[`vite.config.ts`](../pay3low-svelte-frontend/vite.config.ts).
Production enables the required globals; dev uses the plugin's dev-only global
mode to avoid reinjecting shims into the large optimized SDK bundle.

Use `NODE_OPTIONS=--max-old-space-size=6144` for builds and, when needed, cold
dev optimization. Both Docker build paths set this for the build step and reuse the dependency
stage for the builder. Live deployment uses
[`deploy/frontend-runtime/Dockerfile`](../deploy/frontend-runtime/Dockerfile), whose fresh
runtime image receives a filtered dependency tree without declarations,
source maps or unused raw Material Symbols SVG source packages. Runtime
JavaScript, native modules and compiled icon components are retained.
`.dockerignore` excludes local environment files.

Browser coverage is in
[`wallet-execution.spec.ts`](../pay3low-svelte-frontend/tests/wallet-execution.spec.ts)
and [`exchange-flow.spec.ts`](../pay3low-svelte-frontend/tests/exchange-flow.spec.ts):
mode preservation, dark theme, unsupported site wallet families, per-leg amounts
and networks, NEAR catalog aliases, and zero signing/submission on connection.
Use serial workers for cold standalone SDK checks on memory-constrained hosts.
