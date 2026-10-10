# EVER/USDT OTC service

The backend owns a durable customer-first OTC workflow and the existing `/#/otc` page uses it. It trades native EVER on Everscale mainnet (global ID 42, 9 decimals) against issuer USDT on Ethereum mainnet (chain ID 1, contract `0xdac17f958d2ee523a2206206994597c13d831ec7`, 6 decimals). This is the Ethereum route selected during planning. The platform never signs a blockchain transaction or holds a customer or desk wallet key. There are two independent transfers and counterparty risk remains with the desk.

## Setup

1. Run the normal backend database setup. Startup applies the additive `backend/migrations/otc.sql` migration. Use separate databases for demo and live: the first enabled worker binds the database mode, and demo quotes remain unsignable after a configuration switch. OTC records use separate tables from the legacy acquiring/exchange simulation ledger. Preserve the database, JWT secret and encryption key across restarts.
2. Copy `backend/otc.example.json` outside version control. Set `OTC_CONFIG_FILE` to that absolute path in the backend environment. Without it the route is disabled. The example intentionally cannot enable live quotes.
3. Record the onboarded desk identity, canonical lowercase desk wallets, inventory and gas, two independently operated HTTPS RPC endpoints per chain, approved deployed single-key EverWallet code hashes, limits, UTC staffed hours, three deadline durations, refund/net-delivery policies, support owner, incident lead and backup, tested alert webhook, independent external heartbeat receiver, and operating/live-gate evidence. Configure stable asset-definition IRIs that identify the precise chain and deployment. The issuer address is fixed in the adapter and its precision is checked on chain.
4. Set the normal backend `SECRETS_KEY` and `JWT_SECRET` to strong persistent values (at least 32 characters for live OTC), and configure the externally served ActivityPub origin. Check the repository's normal configuration mapping for these values. Actor credentials are encrypted with the application encryption key; wallet signing keys are never uploaded.
5. Run the coordinated fmatch changes in `/home/lion/workspaces/fmatch-pay3flow-routing` against its persistent database. Keep HTTP signatures required. Link existing LF actors through the OTC page; do not provision replacement actors. One application instance manages the onboarded desk and customer participants, with their hosted LF actors on the configured relay. Arbitrary remote desks are outside this one-desk MVP.
6. The browser supports Ethereum EOAs through the existing injected/Reown connection and deployed single-key EVER Wallet accounts in workchain 0. Prove both wallets before RFQ/booking. Proofs expire after 24 hours, challenges after 5 minutes and OTC sessions after 8 hours. OTC JWTs are isolated from the demo email login. Link the desk's exact inventory wallets, test observers, then use **Resume healthy route** in the restricted console. Initial database state is paused.

The service validates configuration before enabled startup. Setting `enabled: true` is an operator release action, not proof of readiness: the independently reviewed live gate below must also pass. A route pause blocks new quotes/bookings. P0 additionally blocks new application signing handoffs. Read-only monitoring and reconciliation of existing obligations continue; external desk wallets are not frozen. Disabling new trading while retaining configuration also preserves reconciliation of outstanding trades.

## Marketplace and negotiation

The compatibility profile is `pay3flow-otc-fep0837-v1`, based on local peer commit `848e0c59b41d28a1e4654db5a1803c614de81a51` plus the coordinated relay fixes. The checked-in context is `backend/tests/fixtures/otc/context.json`; `/api/otc/config` reports its SHA-256. It uses ActivityStreams, Valueflows and OM2 terms. `OfferAgreement`, `AcceptAgreement`, `RejectAgreement` expand to ActivityStreams Offer/Accept/Reject. Public payloads add no fee, monitoring or proprietary namespace fields. Pin this context and the actual built peer revision together for deployment.

The desk publishes two original, structured Proposal objects. `publishes` is always desk output and `reciprocal` customer input. Public LF outbox collections expose the original identities; publishing a bare Proposal needs no Create wrapper. The desk also follows the configured matcher for marketplace registration. All private RFQs, exact `unitBased: false` Proposals, offers, decisions and participant confirmations travel through LF authenticated actor outboxes and durable fmatch delivery jobs. A transport acknowledgement means queued; participant ingestion records delivery. No quote is accepted from prose or converted through floating point.

The service authenticates the linked LF owner against the private relay outbox and its advertised signature/profile requirements. Quotes require both bound participant wallets. Acceptance checks the original offer, primary/reciprocal Intent references, units, exact amounts, deadlines, finalized Agreement/Commitment IDs and desk author. A PostgreSQL transaction reserves reconciled desk inventory before accepting. Accepted terms, participants, fee policy and deadlines are immutable. Lost negotiation responses retain the original activity ID/body and block another selection while the original booking is unknown.

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
| `POST /api/otc/session`, `/session/revoke` | Link/revoke an existing LF owner session and relay delegation |
| `POST /api/otc/wallet/challenge`, `/wallet/proof` | Single-use wallet ownership proof |
| `POST /api/otc/rfqs` | Exact customer input; requires `Idempotency-Key` |
| `GET /api/otc/trades`, `/trades/:id` | Participant history, fixed terms, attempts, evidence and customer updates |
| `POST /api/otc/trades/:id/apply` | Persist original customer OfferAgreement |
| `POST /api/otc/trades/:id/prepare` | Persist one wallet handoff; `kind` payment/payout/refund and `Idempotency-Key` |
| `POST /api/otc/attempts/:id/reference`, `/retry` | Track original result / authorize reviewed conclusive failure retry |
| `POST /api/otc/desk/listings`, `/quotes`, `/trades/:id/decision` | Desk publication, fixed quote, reservation/decision |
| `POST /api/otc/desk/control`, `/accounting`, `/incidents/:key` | Pause/resume, evidence-backed accounting and incident actions |
| `GET /api/otc/desk/dashboard`, `/metrics` | Restricted durable dashboard and Prometheus exposition |

Actor logout revokes the stored relay delegation as well as the session. Relink promptly if this interrupts delivery for an outstanding trade. Access expiry never releases a reservation or declares an outgoing transfer failed.

## Fees, operations and alerts

One booking-linked fee accrues only after verified payment and payout. `max(USDT leg / 400, 5 USDT)` rounds upward once to 6 decimals. The fee uses Buy customer input or Sell promised net output and excludes network costs. Customer input/output are never changed. The ledger separates accrual, collection, compensation, fee returns, support/monitoring/exception costs and operator minutes. Desk accounting requires evidence and an idempotency key. Collections cannot exceed owed fees; only verified completed trades are collectible. Original entries are retained.

The dashboard derives totals from durable deduplicated events, trade/evidence state and ledger entries. RFQ coverage uses closed 120-second response windows, seven-day conversion uses linked RFQs, and on-time payout retains pending/refunded misses. Zero eligible traffic is N/A. The authenticated metrics endpoint uses bounded environment, desk, direction, chain and state labels; IDs, addresses and hashes are absent. The external heartbeat receiver must independently alert on missing heartbeats; never interpret an old healthy scan as current health.

Stable incidents own alert retries and backup escalation. Webhook messages contain opaque incident keys, severity, owner and runbook/dashboard links; transaction/customer evidence remains in the restricted console. P0 pauses booking/signing; P1 covers overdue funded obligations and unknown outgoing results; P2 covers unfinanced federation/observer degradation and fee mismatch. Aggregate quote alerts require 20 eligible observations. Every open funded case remains visible. Quote response p95, waiting/payment/booking ages, obligations by asset, agreed-USDT-leg notional, payment-to-payout intervals and incident acknowledgement/restoration/financial-resolution timings are available in the dashboard evidence. Reconciled receipts do not increment business-stage totals a second time. Overdue customer updates alert after 30 minutes. P2 acknowledgement counts staffed UTC hours. Technical restoration and financial resolution are separate, and P0/P1 closure requires a root cause, preventive test and follow-up owner/date.

The browser runbook is `/otc-runbook`. Review/closure evidence may be JSON; P0/P1 closure requires `root_cause`, `preventive_test`, `follow_up_owner`, `follow_up_date` and financial disposition. Use read-only transaction evidence to reconcile; never delete ledger records or manually mark success.

## Verification and live gate

See [recorded evidence](otc-evidence.md) and the commands there. Fixtures are explicitly simulations and use isolated schemas. No test sends funds or calls real alert recipients. Real wallet signing/receipt traces, independently hosted peer compatibility, live RPC availability, receiving gas/net accounting, funded desk inventory, operating requirements, alert recipients/backup and operator coverage for outstanding funds remain release-gate evidence. A simulation does not authorize live deposits.

Primary implementation references: [FEP-0837](https://fediverse.codeberg.page/fep/fep/0837/), [USDT supported protocols](https://tether.to/en/supported-protocols/), [Ethereum JSON-RPC](https://ethereum.org/en/developers/docs/apis/json-rpc/), [EVER Wallet RPC](https://docs.broxus.com/pages/raw/provider-api.html), [Everscale receipt rules](https://docs.everscale.network/arch/transactions/), [Everscale pagination](https://docs.everscale.network/develop/graphql-pagination/), [TVM chain identity](https://namespaces.chainagnostic.org/tvm/caip2), [Prometheus instrumentation](https://prometheus.io/docs/practices/instrumentation/).

The main OTC terminal retains the design and explicitly labeled test market from release `c5b0fb1`. Its chart and demo orders are separate from settlement. Open **EVER / USDT · OTC desk settlement** below the activity panel for the durable desk workflow, wallet proofs, private quotes, trade details and restricted console. Test-market orders never create a settlement booking or sign a transfer. Live settlement remains disabled until its configuration and release gate are approved.
