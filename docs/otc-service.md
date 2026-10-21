# EVER/USDT OTC service

The backend owns a durable customer-first OTC workflow and the existing `/#/otc` page uses it. It trades native EVER on Everscale mainnet (global ID 42, 9 decimals) against issuer USDT on Ethereum mainnet (chain ID 1, contract `0xdac17f958d2ee523a2206206994597c13d831ec7`, 6 decimals). This is the Ethereum route selected during planning. The platform never signs a blockchain transaction or holds a customer or desk wallet key. Direct settlement uses two independent transfers. The optional per-trade Safe escrow below holds USDT until a two-owner settlement or an expiry refund; native EVER remains a separate Everscale transfer.

## Setup

### Wallet-owned buy and sell advertisements

`public_offers_enabled` enables publication from both sides of the existing
Exchange Buy/Sell ticket. `public_offer_directions` in `/config` advertises the
supported directions. The existing book shows public advertisements
alongside desk Proposals. With public offers enabled, its labels are Orderbook and
My orders; History shows the owner’s closed records. There is one terminal and one form.

The author verifies their EVER wallet. Its address sends EVER for a sale and
receives EVER for a buy offer. Their Ethereum address receives USDT for a sale
and pays USDT for a buy. An already configured Ethereum address is suggested;
a custom address is still editable. Publication requires no token inventory,
second wallet proof, desk identity or transfer. It does not enable settlement.

Both directions show EVER quantity first and total USDT second, with explicit
Buy/Sell amount and Total to pay/receive labels. Both the unit price and total USDT amount are editable. Entering a price and
EVER amount calculates the total with integer arithmetic, rounded down to six
USDT decimals with an explicit hint. With a price set, either amount can be edited: a USDT budget computes EVER
rounded down to nine decimals, and an EVER quantity computes USDT rounded down
to six. Without a manually entered price, entering both amounts derives the rate. Decimal commas and surrounding whitespace normalize before submission.
The disabled button explains the missing or invalid field. The review shows the
exact canonical amounts and addresses before publication, explicitly marked as
not yet published. The Publish button performs the write. The returned open
offer immediately populates the book and chart, even if the subsequent refresh
fails. Public and private refreshes update independently. A retry returning an
already cancelled or expired offer is shown as closed, never reported as a new
publication; another explicit review uses a new idempotency key.

The book displays unit price, EVER quantity, total USDT and the author’s wallet.
Sell offers sort by ascending unit price and buy offers by descending unit price,
using exact integer ratios. The public chart plots one point per publication at
its server publication time, with a visible USDT/EVER axis and exact selected
point readout. It is not a completed-trade chart. Cancelled and expired offers
leave the active book but remain in chart history. The history endpoint returns
only the publication id, time, direction and amounts, never wallet addresses.

Advertisements are public for 24 hours or until cancellation. They are local to
Pay3Flow, not accepted trades, federated desk Proposals or payment instructions.
Buyer acceptance and settlement still require an implemented, configured route.

`GET /api/otc/offers` reads active advertisements; `/offers/history` returns
sanitized publication history; `/offers/mine` reads the authenticated seller's
latest advertisements. `POST /offers` requires the OTC
session, a recent EVER proof and an `Idempotency-Key`. The body’s `direction` is
`buy` or `sell`; omission preserves legacy `sell` behavior. `owner_ever` is the
verified author address; `seller_ever` remains a legacy alias for older clients. Retrying the same body and
key restores the original advertisement. A different body or seller wallet
conflicts. `/offers/:id/cancel` authorizes the owner and remains available when
posting is disabled. The browser saves the exact draft and key before posting.

The pinned, immutable single-key Broxus EVER Wallet supports ownership proof
before deployment: the backend reconstructs its StateInit from the public key,
checks the exact derived workchain-0 address and approved code hash, and verifies
the origin-bound one-use signature. This identity check needs no RPC. It never
attests deployment, current balance or transfer receipt, and does not relax any
settlement observer checks. Nonstandard initial states still require the existing
deployed-account verification. Upstream derivation is documented in
[EverWallet.ts](https://github.com/broxus/everscale-standalone-client/blob/master/src/client/AccountsStorage/EverWallet.ts).

### Desk settlement setup

1. Run the normal backend database setup. Startup applies the additive `backend/migrations/otc.sql` and `backend/migrations/otc_escrow.sql` migrations. Use separate databases for demo and live: the first enabled worker binds the database mode, and demo quotes remain unsignable after a configuration switch. OTC records use separate tables from the legacy acquiring/exchange simulation ledger. Preserve the database, JWT secret and encryption key across restarts.
2. Copy `backend/otc.example.json` outside version control. Set `OTC_CONFIG_FILE` to that absolute path in the backend environment. Without it the route is disabled. The example intentionally cannot enable live quotes.
3. Record the onboarded desk identity, canonical lowercase desk wallets, inventory and gas, two independently operated HTTPS RPC endpoints per chain, approved deployed single-key EverWallet code hashes, limits, UTC staffed hours, three deadline durations, refund/net-delivery policies, support owner, incident lead and backup, tested alert webhook, independent external heartbeat receiver, and operating/live-gate evidence. Configure stable asset-definition IRIs that identify the precise chain and deployment. The issuer address is fixed in the adapter and its precision is checked on chain.
4. Set the normal backend `SECRETS_KEY` and `JWT_SECRET` to strong persistent values (at least 32 characters for live OTC), and configure the externally served ActivityPub origin. Check the repository's normal configuration mapping for these values. Actor credentials are encrypted with the application encryption key; wallet signing keys are never uploaded.
5. Run the coordinated fmatch changes in `/home/lion/workspaces/fmatch-pay3flow-routing` against its persistent database. Keep HTTP signatures required. The configured relay must expose its existing `/api/auth/api-key/generate` signup transport and link generated keys to its control plane. No relay credential environment variable is required: wallet sign-in creates a local account and does not contact the relay. The first federation operation generates an account-scoped delegation and encrypts it in the database. A relay outage or signup restriction cannot block wallet sign-in. First-time wallet users receive an opaque hosted actor automatically; retries reuse its persisted handle. Existing accounts keep their actor identity and trade history. New account identities reserve a stable relay handle locally; they are hosted on the relay only when federation is needed. The customer interface never asks for an actor URL or LF key. One application instance manages the onboarded desk and customer participants, with their hosted LF actors on the configured relay. Arbitrary remote desks are outside this one-desk MVP.
6. The browser supports Ethereum EOAs through the existing injected/Reown connection and deployed single-key EVER Wallet accounts in workchain 0. Sign in with either wallet, then prove the other wallet before RFQ/booking. Either linked wallet can reopen the same account. Verified login addresses remain account login identities even when the active transfer wallet changes; there is no wallet-removal flow in this release. Proofs expire after 24 hours, challenges after 5 minutes and OTC sessions after 8 hours. OTC JWTs are isolated from the demo email login. Onboard the existing desk actor through the LF session API, prove the desk's exact inventory wallets, test observers, then use **Resume healthy route** in the restricted console. Initial database state is paused.

The service validates configuration before enabled startup. Setting `enabled: true` is an operator release action, not proof of readiness: the independently reviewed live gate below must also pass. A route pause blocks new quotes/bookings. P0 additionally blocks new application signing handoffs. Read-only monitoring and reconciliation of existing obligations continue; external desk wallets are not frozen. Disabling new trading while retaining configuration also preserves reconciliation of outstanding trades.

## Marketplace and negotiation

The compatibility profile is `pay3flow-otc-fep0837-v1`, based on local peer commit `848e0c59b41d28a1e4654db5a1803c614de81a51` plus the coordinated relay fixes. The checked-in context is `backend/tests/fixtures/otc/context.json`; `/api/otc/config` reports its SHA-256. It uses ActivityStreams, Valueflows and OM2 terms. `OfferAgreement`, `AcceptAgreement`, `RejectAgreement` expand to ActivityStreams Offer/Accept/Reject. Public payloads add no fee, monitoring or proprietary namespace fields. Pin this context and the actual built peer revision together for deployment.

The desk publishes two original, structured Proposal objects. `publishes` is always desk output and `reciprocal` customer input. Public LF outbox collections expose the original identities; publishing a bare Proposal needs no Create wrapper. The desk also follows the configured matcher for marketplace registration. All private RFQs, exact `unitBased: false` Proposals, offers, decisions and participant confirmations travel through LF authenticated actor outboxes and durable fmatch delivery jobs. A transport acknowledgement means queued; participant ingestion records delivery. No quote is accepted from prose or converted through floating point.

Wallet sign-in verifies a single-use, origin-bound signature before restoring the wallet’s internal account. Newly provisioned actor delegation is checked against the private relay outbox and its advertised signature/profile requirements. Historical wallet mappings are backfilled only when unambiguous; conflicts require operator resolution and never merge accounts. Quotes require both bound participant wallets. Acceptance checks the original offer, primary/reciprocal Intent references, units, exact amounts, deadlines, finalized Agreement/Commitment IDs and desk author. A PostgreSQL transaction reserves reconciled desk inventory before accepting. Accepted terms, participants, fee policy and deadlines are immutable. Lost negotiation responses retain the original activity ID/body and block another selection while the original booking is unknown.

## Transfer and recovery contract

Payment instructions become actionable after participant ingestion of valid acceptance. Before opening a wallet prompt, the service locks the trade and sender wallet and stores an unknown outgoing attempt with its original call, chain, wallets, exact amount, EVM nonce and observer checkpoint. New handoffs from that wallet are blocked until the original resolves. The frontend never repeats a saved handoff after refresh. Track its original transaction hash instead. A declined prompt alone is not conclusive chain failure.

USDT handoffs are direct ERC20 `transfer` calls with zero ETH value; no allowance or swap contract. Receipts must match the issuer, sender, recipient, saved nonce, calldata and exact Transfer log, successful outcome, canonical block and finalized head. Both independent RPC readings must agree. EVER handoffs are direct bounced-safe native messages. A returned sender transaction hash is traced through its outgoing message to the receiving transaction. The recipient's actual balance increase must equal the promised net amount, with successful computation/action and no bounce; finalized receiving evidence is credited once.

`ever_receiving_cost` is an exact decimal, separate internal quote field. The wallet sends net EVER plus this receiving allowance; the observer verifies the actual net balance increase. Sender gas is additional and separately disclosed. Receiving costs depend on wallet execution/storage: an estimate that produces a different net increase enters owned review, never completion. Before live use, demonstrate the chosen net/gas policy on both approved receiving wallets. Refunds use the original proved customer wallet and the same explicit allowance policy; a mismatch remains review.

Unknown results remain blocked. Ethereum unknown references can be located by saved nonce, exact Transfer event and checkpoint; EVER searches for a unique matching receiving message after the persisted handoff. Ambiguous, same-second lost EVER results and replacements require the original wallet/message reference; the service does not guess or abandon them. Confirmed failed Ethereum calls or finalized failed/bounced EVER receiving traces preserve failure evidence and require an explicit evidence-backed retry authorization. Unresolved payout attempts block both another payout and a refund.

Timely receiving-chain inclusion preserves the quote while finality is pending. Late qualifying payments enter review, retain obligations, and accrue no fee. The desk may refund only after verified receipt and conclusive resolution of any outgoing payout. Short payments or a different native net receipt are verified separately for review. They cannot fund a partial fill or enable a payout; a refund uses only the verified actual received amount, the original proved customer wallet and separate receiving allowance. Wrong assets, networks or senders require manual owned review. Invalidated credited evidence moves the trade to review, preserves audit history and triggers P0. Monitoring is deliberately conservative when independent observers disagree.

## Internal application routes

All mutations use the isolated OTC bearer session; private reads are participant-restricted. Desk checks are server-side. These are application routes, not new public federation APIs:

| Surface | Purpose |
| --- | --- |
| `GET /api/otc/config`, `/listings` | Safe route metadata and public Proposals |
| `POST /api/otc/session/challenge`, `/session/proof` | Wallet sign-in; provision or restore the internal account, then issue an eight-hour session |
| `GET /api/otc/session` | Authoritative session, expiry, signing wallet and current verified wallets |
| `POST /api/otc/session` | Existing LF owner session for operator onboarding and compatibility |
| `POST /api/otc/session/revoke` | Revoke the application session |
| `POST /api/otc/wallet/challenge`, `/wallet/proof` | Single-use wallet ownership proof |
| `POST /api/otc/rfqs` | Exact customer input; requires `Idempotency-Key` |
| `GET /api/otc/trades`, `/trades/:id` | Participant history, fixed terms, attempts, evidence and customer updates |
| `POST /api/otc/trades/:id/apply` | Persist original customer OfferAgreement |
| `POST /api/otc/trades/:id/prepare` | Persist one wallet handoff; `kind` payment/payout/refund and `Idempotency-Key` |
| `POST /api/otc/attempts/:id/reference`, `/retry` | Track original result / authorize reviewed conclusive failure retry |
| `POST /api/otc/desk/listings`, `/quotes`, `/trades/:id/decision` | Desk publication, fixed quote, reservation/decision |
| `POST /api/otc/desk/control`, `/accounting`, `/incidents/:key` | Pause/resume, evidence-backed accounting and incident actions |
| `GET /api/otc/desk/dashboard`, `/metrics` | Restricted durable dashboard and Prometheus exposition |

The wallet picker reports provider connections separately from server-verified addresses. An individual wallet card can disconnect its provider without removing the account's ownership proof or history. Connecting a provider whose OTC verification is unavailable does not authenticate it for trading. The account-wide Disconnect requests application-session revocation and disconnects the Ethereum and Everscale providers. Private browser state and their saved connections are cleared even if server revocation fails. Server-side relay delegation remains available for outstanding-trade delivery and reconciliation. An operator may revoke compromised delegation separately; public wallet sign-in cannot restore revoked delegation or grant desk privileges. Access expiry never releases a reservation or declares an outgoing transfer failed.

## Fees, operations and alerts

One booking-linked fee accrues only after verified payment and payout. `max(USDT leg / 400, 5 USDT)` rounds upward once to 6 decimals. The fee uses Buy customer input or Sell promised net output and excludes network costs. Customer input/output are never changed. The ledger separates accrual, collection, compensation, fee returns, support/monitoring/exception costs and operator minutes. Desk accounting requires evidence and an idempotency key. Collections cannot exceed owed fees; only verified completed trades are collectible. Original entries are retained.

The dashboard derives totals from durable deduplicated events, trade/evidence state and ledger entries. RFQ coverage uses closed 120-second response windows, seven-day conversion uses linked RFQs, and on-time payout retains pending/refunded misses. Zero eligible traffic is N/A. The authenticated metrics endpoint uses bounded environment, desk, direction, chain and state labels; IDs, addresses and hashes are absent. The external heartbeat receiver must independently alert on missing heartbeats; never interpret an old healthy scan as current health.

Stable incidents own alert retries and backup escalation. Webhook messages contain opaque incident keys, severity, owner and runbook/dashboard links; transaction/customer evidence remains in the restricted console. P0 pauses booking/signing; P1 covers overdue funded obligations and unknown outgoing results; P2 covers unfinanced federation/observer degradation and fee mismatch. Aggregate quote alerts require 20 eligible observations. Every open funded case remains visible. Quote response p95, waiting/payment/booking ages, obligations by asset, agreed-USDT-leg notional, payment-to-payout intervals and incident acknowledgement/restoration/financial-resolution timings are available in the dashboard evidence. Reconciled receipts do not increment business-stage totals a second time. Overdue customer updates alert after 30 minutes. P2 acknowledgement counts staffed UTC hours. Technical restoration and financial resolution are separate, and P0/P1 closure requires a root cause, preventive test and follow-up owner/date.

The browser runbook is `/otc-runbook`. Review/closure evidence may be JSON; P0/P1 closure requires `root_cause`, `preventive_test`, `follow_up_owner`, `follow_up_date` and financial disposition. Use read-only transaction evidence to reconcile; never delete ledger records or manually mark success.

## Verification and live gate

See [recorded evidence](otc-evidence.md) and the commands there. Fixtures are explicitly simulations and use isolated schemas. No test sends funds or calls real alert recipients. Real wallet signing/receipt traces, independently hosted peer compatibility, live RPC availability, receiving gas/net accounting, funded desk inventory, operating requirements, alert recipients/backup and operator coverage for outstanding funds remain release-gate evidence. A simulation does not authorize live deposits.

Primary implementation references: [FEP-0837](https://fediverse.codeberg.page/fep/fep/0837/), [USDT supported protocols](https://tether.to/en/supported-protocols/), [Ethereum JSON-RPC](https://ethereum.org/en/developers/docs/apis/json-rpc/), [EVER Wallet RPC](https://docs.broxus.com/pages/raw/provider-api.html), [Everscale receipt rules](https://docs.everscale.network/arch/transactions/), [Everscale pagination](https://docs.everscale.network/develop/graphql-pagination/), [TVM chain identity](https://namespaces.chainagnostic.org/tvm/caip2), [Prometheus instrumentation](https://prometheus.io/docs/practices/instrumentation/).

The main OTC terminal uses `/api/otc` directly for original desk Proposals, private exact RFQs/quotes, immutable trade details, verified transfer evidence and the restricted desk console. The Exchange card is the settlement entry point; there is no separate settlement section or test-market feed. The existing chart shows the participant’s server-recorded quote observations for desk trades, or current public asking prices when creating a sale or when no private observations exist. Public points use their publication timestamps and are explicitly labelled as asking prices rather than completed trades. The original terminal and panel collapse controls serve both types of offers. Sale amounts remain exact decimal strings, and the form distinguishes total USDT proceeds from the calculated USDT-per-EVER rate. Charts remain empty without data. Public sharing includes only the pair and direction, never private quote amounts, rates or trade identities. Saved booking/handoff markers block duplicate selection or sends after a lost response; reconciliation uses the original booking/attempt. Live settlement remains paused until configuration and the live gate are complete.

The OTC connection bar has been removed. The Exchange card opens wallet sign-in or wallet management. Wallet prompts require a user action; application restoration verifies the saved token and wallet status with the server. Removing the status bar does not resume a paused route or relax trading availability checks.

`GET /api/otc/config` reports `wallet_login_chains` and `wallet_login_available` separately from trading availability. Configured wallet sign-in remains usable when new trading is disabled, so existing participants can reopen their history and monitor outstanding obligations. Login does not resume the route. Pending actor provisioning is excluded from relay reconciliation until ownership/profile checks finish.


## Production Safe wallet

The OTC page also offers a standard 2-of-3 Ethereum Safe 1.4.1 wallet, independently of desk quote availability. Enter buyer, seller and arbitrator owner addresses, review the predicted wallet, and create it through one owner wallet. The browser checks the canonical factory, singleton and fallback handler runtime hashes published by [Safe deployments](https://github.com/safe-global/safe-deployments/tree/main/src/assets/v1.4.1). It calls the existing official factory with atomic owner setup, threshold 2, zero setup delegatecall, and zero payment. No custom factory, guard or vault is deployed in this production flow. Owners pay creation gas; no tokens move during creation.

The wallet stores the original reviewed creation across reloads. A submitted or unknown provider outcome blocks another creation; on-chain inspection verifies the predicted address, owners, threshold and singleton. An explicit wallet rejection or confirmed reverted creation can be reviewed again. Open the resulting or existing wallet in the official Safe application to fund it, propose transfers, collect approvals and execute. Participants supply their own owner addresses; the backend does not choose an arbitrator or hold keys. Ordinary Safe transfers and refunds require the configured owner threshold. There is **no automatic four-hour expiry or unilateral timeout refund** in this standard Safe flow. Native EVER delivery remains separate.

## Optional four-hour Safe escrow

Set `escrow` in the private OTC configuration to the deployed factory policy. The example keeps it `null`; this implementation does not deploy contracts or enable live trading. Quotes snapshot the policy into immutable terms. Existing direct trades keep their original settlement path.

```json
{
  "escrow": {
    "factory": "LOWERCASE_FACTORY_ADDRESS",
    "factory_code_hash": "0x_RUNTIME_KECCAK256",
    "arbitrator": "LOWERCASE_INDEPENDENT_EOA",
    "singleton": "LOWERCASE_SAFE_1_4_1_SINGLETON",
    "singleton_code_hash": "0x_RUNTIME_KECCAK256"
  }
}
```

Each accepted trade creates one official Safe 1.4.1 proxy and one USDT vault. Owners are the EVER buyer, EVER seller, and the configured independent arbitrator; any two can settle. All three must be distinct EOAs. Both traders sign EIP-712 consent binding the chain, factory, trade ID, accepted terms hash, owners, USDT deposit, native EVER size, and deployment validity. Factory-scoped CREATE2 prevents third parties from predeploying the trade's Safe. Setup atomically fixes the owners, 2-of-3 threshold and guard. The guard permits only zero-value CALLs to the vault's `release()` and `refundEarly()` with zero Safe gas reimbursement; ownership/threshold changes, guard removal, modules, fallback changes and delegatecalls are blocked.

The exact USDT deposit is held in the separate vault, not in the Safe. The cap is 100,000 native EVER (9 decimals), excluding receiving allowance. The vault deadline is its creation block timestamp plus 14,400 seconds. Consent's `validUntil` is the booked deployment deadline, not the escrow lifetime. Both Buy and Sell fund USDT first: the customer deposits on Buy; the desk deposits on Sell. Only after finalized funding can the EVER seller prepare the separate native EVER transfer. New EVER signing closes 1,800 seconds before expiry, with checks at preparation and immediately before the browser opens the send prompt.

Normal release proposals require finalized EVER delivery; normal early refunds require no delivered or unresolved EVER transfer. Dispute proposals require a reason and structured evidence. Any pair of owners can approve the fixed-recipient settlement, and an explicit executing wallet pays gas. USDT release without qualifying EVER evidence, or refund after EVER delivery/an unresolved EVER send, enters owned review and earns no completion fee. The existing fee and participant confirmation are recorded once only after both qualifying legs complete. A USDT refund cannot reverse EVER delivery; this is not atomic settlement across Ethereum and Everscale.

At or after expiry, anyone can call `refundExpired()` without Safe signatures, sending the agreed USDT to its original depositor. Release and early refund are then disabled. An unfunded vault expires without transferring tokens. Extra donated tokens cannot count as funding and can be recovered to the depositor only after a terminal state. Real USDT issuer restrictions still apply to token transfers. The UI displays the creation/deadline and exports the vault address, refund ABI and a direct recovery command. Timeout recovery does not depend on the new-trading/signing pause or the factory being available; calling the vault directly also works without this application.

The arbitrator signs into the existing wallet interface with the configured Ethereum EOA. The account sees assigned accepted escrow trades and their evidence/proposals, but receives no desk inventory, pause, incident or administrative permissions. EIP-712 approvals never themselves move funds. All wallet prompts follow explicit review. Durable handoffs serialize each sender wallet across escrow and direct transfers. Reload shows the original unknown/submitted call and blocks another send; tracking accepts only its original reference. Observers can recover a lost reference from the saved nonce, exact calldata and checkpoint. Only a canonical finalized reverted call enables an explicit fresh retry review; elapsed time or a cancelled prompt is insufficient.

Both independently operated Ethereum RPCs must agree on chain 1, pinned runtime hashes, original deployment consent, Safe owners/threshold, canonical finalized receipts, vault events and exact USDT Transfer logs. On-chain events are authoritative even if participants execute outside this UI. Credited receipts are revalidated; invalidation preserves audit evidence, moves the trade to review and raises P0. Existing Everscale reconciliation continues independently.

### Escrow application routes

All routes remain under OTC bearer authentication; mutations require `Idempotency-Key`.

| Route | Purpose |
| --- | --- |
| `GET /api/otc/trades/:id/escrow` | Accepted policy, owners, typed consent, creation/deadline, proposals, calls and evidence |
| `POST .../escrow/consent` | Immutable trader deployment consent |
| `POST .../escrow/proposals` | Normal or documented dispute settlement |
| `POST .../escrow/approvals` | Verify and save an owner's EIP-712 approval |
| `POST .../escrow/prepare` | Persist reviewed deploy/allowance/fund/execute/timeout call |
| `POST .../escrow/reference` | Track the immutable original transaction reference |

### Build and verification

Contracts and locked dependencies are in `contracts/otc`; exported ABIs are tracked in `contracts/otc/abi`.

```sh
cd contracts/otc
npm ci
npm run build
npm test
npm run export
ETHEREUM_FORK_RPC=https://YOUR_READ_ONLY_MAINNET_RPC npm run test:fork
OTC_TEST_DATABASE_URL=postgres://USER@localhost/DISPOSABLE_DB npm run test:integration
```

The fork test uses real Ethereum USDT runtime and local storage only. The integration harness starts local Anvil, deploys actual Safe/vault/guard contracts with mock USDT, and drives the Rust service against a disposable PostgreSQL database; EVER evidence is simulated. These do not sign or send mainnet transactions.

`script/Deploy.s.sol` supports a separately reviewed mainnet deployment using `OTC_ARBITRATOR`, `SAFE_SINGLETON`, an Ethereum RPC and a local Foundry keystore. It checks chain 1 and Safe version 1.4.1 and emits the factory/singleton runtime hashes. First run `node scripts/foundry.mjs forge script script/Deploy.s.sol:Deploy --rpc-url "$ETHEREUM_RPC_URL" --account YOUR_LOCAL_KEYSTORE` without broadcasting. Live enablement still requires the existing operating gate, independent contract review, approved/pinned deployments, two independent RPCs, and real native EVER delivery/recovery evidence. No escrow live gate is claimed by the local tests.
