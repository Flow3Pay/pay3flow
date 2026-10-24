# Native EVER OTC multisig

Pay3Flow's `/#/otc` page includes a **Native EVER OTC multisig** workspace.
It imports the Rust CLI's public agreement, verifies the deployed Everscale
SafeMultisig wallet and uses the connected EVER Wallet for proposals and second
approvals. Owners keep their signing keys in their own wallets. The backend
does not accept seeds, private key files, arbitrary ABI calls or deployment
requests.

This workspace manages native EVER under a fixed 2-of-3 buyer/seller/arbitrator
agreement. Release sends the exact agreed amount to the buyer; refund sends it
to the seller. Two owners using another tool can authorize different spending:
these agreement checks are application policy, not restrictions added to the
standard SafeMultisig contract. There is no automatic expiry, unilateral refund
or atomic EVER/USDT swap. A successful wallet call does not prove recipient
delivery and does not credit or complete a Pay3Flow desk order. Existing desk
settlement and the Ethereum Safe workspace retain their separate accounting.

## Participant workflow

1. Build `evernode-rs` from the bundled [Rust crate](../backend/crates/evernode-multisig/README.md).
   Follow its [OTC guide](../backend/crates/evernode-multisig/OTC.md) to create and
   deploy the 2-of-3 wallet. Each owner supplies their own public key; each deal
   uses a fresh deployment key. That deployment key is not a fourth owner.
2. Export the public agreement and inspect its review hash:

   ```sh
   evernode-rs otc export --name deal001 --output /path/to/public-trade.json
   evernode-rs otc status --name deal001
   ```

   Exchange the terms hash through an independently verified channel. Review
   the wallet address, all three owner public keys, release/refund addresses,
   mainnet network ID 42 and exact amount with the other parties. Never paste a
   keypair or seed file into Pay3Flow.
3. Open the OTC page, expand **Native EVER OTC multisig**, paste the exported
   JSON and independently reviewed hash, then select **Review public agreement**.
   This local review works even while server signing or desk quotes are disabled.
4. Sign in with EVER Wallet, or verify/bind an Everscale wallet to the existing
   OTC session. The deployed single-key wallet must prove the same public key
   as one of the multisig owners. Select **Verify on Everscale**.
5. Select **Review release** or **Review refund**. Inspect the fixed destination,
   amount, action and public key, then select **Confirm in EVER Wallet**. Network
   fees come from the multisig balance; the fixed amount must leave a fee balance.
6. The second owner imports the same agreement, verifies it and selects
   **Review second approval** for the exact pending transaction ID. The server
   checks the destination, amount, flags, empty payload and actual on-chain
   owner index before preparing `confirmTransaction`.
7. If the wallet result is lost, reload and use **Reconcile original call** with
   the original Everscale transaction hash. Never send a replacement while its
   outcome is unknown. Provider rejection/timeouts remain conservatively blocked
   if no conclusive receipt is available. A preparation interrupted before the
   wallet was invoked can be explicitly continued using the same saved request
   and idempotency key. Neither recovery path automatically signs.

## Server setup

The feature is off by default (`everscale_multisig_backend: null` in
`backend/otc.example.json`). Configure these fields in `OTC_CONFIG_FILE`:

| Field | Requirement |
| --- | --- |
| `everscale_multisig_backend` | Absolute server-owned executable path to compatible `ever-cli`; tested with 0.44.0. |
| `everscale_graphql` | At least two distinct, independently operated HTTPS Everscale mainnet endpoints. |
| `everscale_global_id` | `42`. |
| `everscale_lag_seconds` | Nonzero maximum masterchain age, up to 600 seconds; normally 60. |
| `everwallet_code_hashes` | Approved deployed single-key EverWallet code hashes for owner sign-in/binding. |
| `relay_origin`, `origin` | Existing OTC wallet identity provisioning and origin-bound sign-in configuration. |
| `demo` | `false` for server verification/signing. Demo mode cannot enable server signing. |

JWT and encryption secrets must each be at least 32 characters whenever this
feature is configured, including when desk quotes are disabled. Supply the
TVM executable through the deployment's package or a read-only mount; the
default backend container does not download/install it. The Docker build copies
the bundled crate so the Rust binary has no dependency on a sibling checkout.

Startup applies the additive `otc_ever_multisig.sql` migration. Preserve the
database and browser recovery storage across restarts. `otc_controls.signing_paused`
blocks new preparations. Native multisig capability is independent of desk quote
availability and is reported as `everscale_multisig_available` by `/api/otc/config`.
Do not enable the feature until the configured providers, binary and wallet
sign-in path have been checked in the deployment environment.

Before preparing a call the server requires agreement on a recent mainnet head
and identical wallet reviews from both providers. Rust verifies the wallet
address from its deployment public key, pinned SafeMultisig code hash,
three exact owner keys, threshold two and pending transfers against a single
account snapshot per provider. Subprocesses have ten-second per-command limits
and run on bounded blocking workers. Executable paths and RPC URLs come from
server configuration, not HTTP input.

An immutable agreement registry binds each wallet to one terms hash. Recovered
preparations must pass a fresh chain review and the signing-pause control. A durable
handoff is written before the wallet prompt; an unresolved handoff blocks every
participant from preparing another call for that wallet. Database constraints
and advisory locks serialize competing requests. An idempotency key can only
return its original immutable request. The browser saves a recovery marker
before preparation and switches it to signing before invoking EVER Wallet.

Reconciliation requires two providers to agree on a unique finalized transaction.
The backend verifies the external-message Ed25519 signature with mainnet ID 42,
then decodes the actual ABI call and matches its wallet, signer, method, parameters
and post-preparation message timestamp. Old receipts, another contract's receipt,
invalid signatures and receipt reuse cannot unblock a handoff. Only a conclusive
accepted/aborted matching receipt resolves it. All responses retain
`settlement_delivery_verified: false`.

## HTTP API

All endpoints require an OTC Bearer session and return `Cache-Control: no-store`.
Requests use only the version 1 public agreement format. Amounts, transaction
IDs and ABI uint values remain decimal strings.

| Endpoint | Request/result |
| --- | --- |
| `POST /api/otc/multisig/review` | `{trade, terms_hash}` → verified balance, indexed owners, reviewed pending transactions and unresolved handoff ID. |
| `POST /api/otc/multisig/prepare` | `{trade, terms_hash, operation: {public_key, action, transaction_id}}`, plus `Idempotency-Key` → durable handoff and unsigned EVER Wallet call. `transaction_id: null` proposes; a string confirms. |
| `GET /api/otc/multisig/calls/:id` | Public chain reconciliation state (`unknown`, `accepted`, `failed`) and saved evidence. |
| `POST /api/otc/multisig/calls/:id/reference` | `{transaction_hash}` → independently verified original call result. |

`sendExternalMessage` follows the
[official EVER Wallet provider API](https://provider-docs.broxus.com/api-reference/index.html).
The browser pins the same ABI and validates every returned call against its own
reviewed agreement before passing it to the provider.

## Verification

```sh
cargo fmt --manifest-path backend/Cargo.toml --all --check
cargo test --manifest-path backend/Cargo.toml --workspace
cargo clippy --manifest-path backend/Cargo.toml -p evernode-multisig --all-targets -- -D warnings

# Disposable PostgreSQL only; the TVM check only encodes/decodes local test messages.
OTC_TEST_DATABASE_URL=postgresql://USER@127.0.0.1:PORT/DATABASE \
EVER_CLI_TEST_BINARY=/absolute/path/to/ever-cli \
  cargo test --manifest-path backend/Cargo.toml --lib otc:: -- --include-ignored --skip local_chain_roundtrip

cd pay3low-svelte-frontend
npm run check
npm run test:unit
npm run build
npm run test:e2e -- tests/everscale-multisig.spec.ts tests/production-safe.spec.ts tests/otc.spec.ts
```

The ignored native service test additionally needs Python 3 and OpenSSL for its
temporary local TLS fixtures. The separate Ethereum `local_chain_roundtrip` test
requires the environment provisioned by `contracts/otc/scripts/integration.mjs`.

Browser tests use injected provider/API fixtures. Rust runtime tests use a fake
chain and real offline codec checks; PostgreSQL tests check persisted handoffs,
immutability and receipt uniqueness. None of these tests deploy a contract,
broadcast a message or prove live recipient delivery.
