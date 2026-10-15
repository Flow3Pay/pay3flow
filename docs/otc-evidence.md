# OTC implementation evidence

Recorded 2026-10-10. All settlement, federation and alert fixtures below are **simulations**. They use isolated PostgreSQL schemas, local mock transports or static signed-message/receipt fixtures. No customer funds, live wallet keys, real invoices or external alert recipients were used. The route is disabled by default and its database starts paused.

## Recorded checks

| Check | Result | Evidence and limits |
| --- | --- | --- |
| Backend `cargo check` | Passed | Compiles the service, server wiring and existing binaries; existing warnings remain. |
| Backend OTC tests, including database tests | 16 passed | Exact amounts/fee rounding; changed negotiation identities/terms; both directions; duplicate credits and concurrent inventory reservation; private/desk access; saved handoff replay; unknown payout blocks refund; late payment/refund; partial receipt refunds actual amount; invalidated receipt restoration retains one fee; dashboard/ledger reconciliation, missing data, zero traffic and alert delivery. |
| ActivityPub signature regression tests | 4 passed | Digest, replay window, signing round trip, and rejection of signatures that omit the request target/body digest. |
| Coordinated fmatch relay tests | 6 passed | Private audience filtering, direct inbox selection, actor normalization, network address checks, private-result isolation and durable delivery/idempotency/cursor acknowledgement. |
| Coordinated fmatch profile fixture | 1 passed | Same two-origin Buy/Sell Proposal/Intent/Agreement fixtures, structured amounts and original identities. This is fixture compatibility, not an independent deployed-peer exchange. |
| Frontend type/Svelte checks | 0 errors, 0 warnings | Connected application and runbook. |
| Frontend unit tests | 18 passed | Existing application unit suite. |
| Frontend production build | Passed | Existing d3 circular-dependency notices remain. |
| OTC browser tests | 6 passed | Desktop and mobile; 320px overflow/accessibility, fixed accepted terms/fees/three deadlines, refresh and unknown handoff blocking. Uses API fixtures and does not sign a real transfer. |
| Whitespace checks, both repositories | Passed | `git diff --check`. |
| fmatch repository line-limit check | Fails on 9 pre-existing oversized files | This repository-wide check is not green. The new relay modules stay below 300 lines; unrelated oversized modules were not reorganized. |

Temporary run logs were written under `/tmp/pay3flow-otc-{test,signature-test,check,ui-check,ui-unit,ui-build,ui-test}.log` and `/tmp/fmatch-otc-{test,profile-test,check}.log`. Repeatable tests below are the durable evidence; `/tmp` logs are not deployment artifacts.

## Repeatable checks

Use a disposable PostgreSQL database. Each database test creates and drops its own schema; never point the test variables at a production database. A normal `cargo test` skips database scenarios unless `--include-ignored` is supplied.

```sh
export OTC_TEST_DATABASE_URL='postgres://USER@127.0.0.1:PORT/otc_tests'
cargo check --manifest-path backend/Cargo.toml
cargo test --manifest-path backend/Cargo.toml --lib otc:: -- --include-ignored
cargo test --manifest-path backend/Cargo.toml --lib activitypub::signature::tests

cd pay3low-svelte-frontend
npm run check
npm run test:unit
npm run build
npm run test:e2e -- tests/otc.spec.ts
```

Playwright requires its browser installation or `PLAYWRIGHT_CHROMIUM_EXECUTABLE` pointing to a compatible installed Chromium. `PLAYWRIGHT_PORT` can select a free local port. This verification used Rust 1.97, PostgreSQL 18.6 and the installed Chromium 152.

In the authorized companion checkout `/home/lion/workspaces/fmatch-pay3flow-routing`:

```sh
export FMATCH_TEST_DATABASE_URL='postgres://USER@127.0.0.1:PORT/otc_tests'
cargo check
cargo test relay -- --include-ignored
cargo test otc_two_origin_fixtures
bash tools/check_line_limits.sh
```

The static compatibility context and both directions are checked in at `backend/tests/fixtures/otc/`; fmatch consumes the identical negotiation fixture. Pin their digest and the final built peer revision together. The profile extends peer base `848e0c59b41d28a1e4654db5a1803c614de81a51` with the accompanying uncommitted relay changes.

## Definition of Done evidence mapping

| Requirement | Recorded development evidence | Still requires deployment/operator evidence |
| --- | --- | --- |
| End-to-end and fees | Buy/Sell database scenarios record accepted terms, two deduplicated receipts and exactly one 5 USDT fee. Fee tests cover below/at/above 2,000 USDT and single precision rounding. Refunded/short cases accrue no fee. | Real signed payment/payout receipts in both directions; desk agreement to the policy. |
| Marketplace and acceptance | Shared two-origin structured fixtures and exact acceptance validation; immutable terms and persisted original decisions. | Two deployed origins exchanging the pinned messages, real fmatch registration/discovery and supported peer revision. |
| Idempotency/recovery | Concurrent reservation/credit tests, saved unknown attempt replay, cursor acknowledgement, invalidation/restoration and blocked payout/refund tests. | Process-kill/restart drill with real relay/observers and wallet handoff; prove replacements under adapter rules. Service reconstruction in tests shares the durable database and is not a complete operating-system restart drill. |
| Verification | Ethereum exact deployment/calldata/nonce/log/canonical/finality fixtures; EVER receiving net/bounce fixtures; receipt invalidation and late/short review. | Current independent mainnet RPCs, deployed wallet code/public-key proofs, exact receiving-cost policy and real finality observations. |
| Usability/access | Participant/desk denial tests, isolated OTC sessions, desktop/mobile browser fixtures and refresh blocking. | Supported real wallet combinations, operational keyboard/touch review and operator acceptance. |
| Metrics | Fixture/ledger totals, N/A zero traffic, missing observer telemetry, restricted metrics and simulated alert receiver. | Every configured alert/backup/heartbeat route, retention/access and staffed operating targets. |
| Incidents | Stable owned incidents, unknown payout/observer/fee detection, restoration versus financial closure and participant update evidence. | Full lost-acceptance/lost-payout/stale-observer/fee-mismatch drills with named operators, actual acknowledgement/containment/customer update and financial disposition. |
| Handover | [Setup and recovery contract](otc-service.md), disabled example configuration, repeatable fixtures/tests, and `/otc-runbook`. | Complete all Day 1 configuration and live-pilot gate entries; record reviewers and evidence. |

The Pay3Flow application was deployed on 2026-10-10 to the configured Lefine production k3s target from revision `e4339598b129395336227e5cb12658d2e7e15361` (image tag `live-20261010-e433959`). Both backend and frontend Deployments reached 1/1 Ready; the deploy script public root, `/health`, and provider checks passed. A post-rollout public `GET /api/otc/config` returned `enabled: false` and `available: false`; no OTC configuration, wallet signing or deposits were enabled. The coordinated fmatch source changes were not deployed to the hosted matcher. The table deliberately does not mark unperformed peer, wallet, inventory or financial drills complete. A successful simulation or dormant application deployment does not authorize live deposits.

## Existing fmatch line-limit failures

The repository checker reports `routing/validation_helpers.rs`, `routing/entry.rs`, `routing/validation.rs`, `routing/openrouter_route_test.rs`, `routing/activity_protocol.rs`, `routing/delivery.rs`, `typesense/search.rs`, `activitypub/mod.rs` and `db.rs` above 300 lines. They were already oversized at the checkout base. This remains a release-review item; the targeted compilation and tests above do not imply that every repository-wide check passed.

## Released interface restoration — 2026-10-10

Restored the terminal, navigation, chart, orderbook, panel controls and activity UI from `c5b0fb1`, retaining subsequent provider/share changes from the live release. The original test market remains explicitly labeled and cannot submit settlement transfers. The EVER/USDT desk workflow is preserved in a separate expandable section below the terminal.

Validation: backend `cargo check` passed; Svelte check reported zero errors/warnings; 27 frontend unit tests passed; production frontend build passed. The 38-scenario browser run passed 36 cases and found two panel-layout scope failures caused by nesting the settlement section inside the terminal. After separating the sections, all 8 targeted desktop/mobile panel and settlement checks passed. The 320px fixture screenshot had no horizontal overflow. No real signing or live deposits were enabled.

Deployment: `abfeb06b844cb6a2fe75fa47dded1299140ffb4c` (`live-20261010-abfeb06`) is live at https://pay3flow.lefine.pro/#/otc. Backend/frontend each reported 1 ready replica; public route checks passed. The live browser showed the restored Russian heading, connected test-market WebSocket and rendered chart, no page errors, and no overflow at 320px. The separate settlement section reported `enabled:false`, `available:false` and disabled quote requests. Backend, wallet picker and provider profile sources match preceding live release `6598486`; no live-deposit gate was opened.

## Direct terminal integration — 2026-10-10

This supersedes the separate settlement section in the preceding restoration entry. The restored terminal now calls the durable `/api/otc` service directly. No test-market WebSocket is opened by this UI. Public desk offers retain their Proposal identity and resource-based direction; exact participant quotes supply displayed output/rates. Quote history and completed USDT volume come from participant trade records, with no generated liquidity or candle fallback. The Exchange card, My trades/details and restricted desk console cover the workflow together. Wallet handoff review follows durable preparation; reload never repeats signing.

Recorded verification (all funds/desk/chain responses in browser tests are simulations):

- `npm run check`: 0 errors, 0 warnings.
- `npm run test:unit`: 30 passed, including exact chain precision/u128 input validation, structured Proposal direction and completed-volume arithmetic beyond Number precision.
- Playwright `tests/otc.spec.ts`: 24 desktop/mobile workflow cases passed, then 2 additional public-sharing/navigation cases passed. This covers both directions, canonical exact RFQs and stable retry keys, quote editing, accepted terms/deadlines/fee, lost booking/prepare responses, restart blocking, expired/rejected quotes, restricted desk controls, unresolved payout/refund exclusion, zero-traffic metrics, refunds in history, backend outage, layout restoration, delayed private-response discard after logout and 320px accessibility/overflow. Public sharing excludes participant terms and quote observations.
- `npm run build` in the release worktree: passed. Existing dependency bundling warnings remain; Svelte diagnostics are clean.
- Browser screenshots of the integrated quote view were visually inspected; terminal hero, three columns, card style and panel toggles are preserved.

Backend financial/protocol code is unchanged by this integration. Production live-route configuration and real-money gate remain separate.

Live deployment verified: `cc83ef1a50f2d241ff871f36dd6af738f45c79a6` (`live-20261010-cc83ef1`) at https://pay3flow.lefine.pro/#/otc. The release includes the preceding live wallet-picker changes from `cc67b50`. Backend/frontend each reported 1 ready replica and root/health/provider route checks passed. Real-browser checks at 1440px and 320px confirmed the single Russian OTC terminal, real `/api/otc/config` and `/api/otc/listings` requests, no test-market WebSocket, no settlement section, no generated chart points/output, no page errors and no horizontal overflow. Anonymous trade/dashboard reads returned 401. The real service still reports `enabled:false`, `available:false`; quote requests are disabled. No live route or deposit gate was enabled.


## Wallet sign-in and OTC connection bar removal (2026-10-10)

The OTC customer interface now uses Ethereum or EVER Wallet sign-in and has no connection bar or actor/credential form. First-time accounts receive an opaque server-managed hosted actor; existing wallet mappings retain their account and trade history. The Exchange card manages both wallets. Wallet sign-in stays available for configured routes when new trading is disabled, while quote and booking checks still apply.

Verification used local relay/RPC fixtures and simulated browser wallets; no real wallet sent funds:

- All 23 backend OTC tests passed, including five new login tests covering both signatures, replay and expiry, concurrent signup, provisioning retries, conflicting accounts, desk wallet protection and strict request payloads. Negative controls were run and restored to confirm all five tests fail when their invariants are inverted.
- All 30 frontend unit tests passed; Svelte checks reported zero errors and warnings; the final production build passed.
- The full OTC browser regression run passed 36 desktop/mobile scenarios. The final focused run passed 18 desktop/mobile checks for both login wallets, rejected signatures, authoritative restoration, session expiry, logout during pending reads/submissions, disabled trading, and the header wallet dialog.
- An isolated PostgreSQL migration smoke test applied the migration twice, retained legacy delegation defaults, backfilled an unambiguous normalized wallet, and left conflicting historical mappings unassigned.
- Rust formatting and all-target/all-feature Clippy completed. Existing repository warnings remain; the new login module has no Clippy warnings.

Wallet signup is local and requires no relay credential or relay network request. Federation obtains an account-scoped delegation lazily through the configured relay's existing key-generation transport. The key is never returned to the browser. Pending provisioning is omitted from relay polling; application logout retains managed delegation for outstanding obligations. Live deployment and real signing/receipt evidence remain separate from these fixture checks.


## Wallet login redeployment — 2026-10-10

- Deployed backend and frontend image tag `wallet-20261010T180533Z-af59cbce` to `https://pay3flow.lefine.pro`; both Deployments report one ready replica. Source snapshot SHA-256: `517c06d3b872c1061368f1a0adf9f4a408a2a6e721566a9f19075e81470b3c9a`. Database and deployment specifications were backed up before rollout.
- Removed the relay credential environment setting. New wallet accounts and sessions are created locally; relay delegation is requested only when federation needs it. A blocked or unavailable relay cannot block sign-in.
- All 24 backend OTC tests passed, including signup with unavailable relay access, isolated generated delegations, encrypted storage, returning-account reuse, replay and expiry. The new key-linkage assertion was exercised with a failing negative control before restoring it.
- Frontend checks found zero errors/warnings. The production frontend build passed. Eighteen desktop/mobile wallet, restore and logout checks passed, plus two checks for disabling unconfigured wallet chains. Earlier resource-related browser timeouts passed after disposable Rust build artifacts were cleared.
- Live API checks used a fresh disposable EVM signing key against the actual production verification RPCs: wallet challenge, signed proof, session restore and logout succeeded; proof replay and revoked sessions returned 401. No relay key was supplied or returned.
- Live browser checks verified the connection bar, Route paused label and Link actor control are absent; there is no automatic wallet prompt. Real signed login, reload restoration and disconnect passed. Home, health and provider endpoints returned 200. No transfer or trade was requested.
- Production currently enables EVM sign-in through two Ethereum RPCs. EVER Wallet requires its verification endpoints and approved contract hashes; its OTC button is disabled until those are configured. OTC trading remains disabled.


## Shared quote and wallet modal — 2026-10-10

“Request exact quote” now opens wallet connection when the session or either wallet proof is missing. The header and Exchange wallet controls use the same modal. Sign-in, ownership verification and explicit quote submission remain in that modal. Closing preserves the ticket, cancels pending sign-in retention and never submits an RFQ. Wallet connection remains accessible when trading is unavailable; actual quote submission still requires both verified wallets and an available desk offer.

Verification:

- Svelte checks: zero errors and warnings; production frontend image build passed.
- All 46 OTC desktop/mobile scenarios passed across the final regression run and the session-expiry rerun. The expiry fixture now waits for its private view to appear before testing its removal. All 30 generic wallet regression scenarios passed. Coverage includes one persistent dialog through both wallet proofs, 320px layout, focus containment/restoration, rejected signatures, cancellation of a late sign-in response, original RFQs and trading availability checks.
- Frontend image `wallet-modal-20261010T183236Z-df29f3ca` deployed at https://pay3flow.lefine.pro/#/otc; runtime image identity and readiness verified. Frontend source snapshot SHA-256: `df29f3ca9393307c1aee4d33114745bd43b1579ef94d10893dab3c5d1bacc612`. Previous frontend specification is saved in `/home/xila/pay3flow-backups/wallet-modal-20261010T183236Z-df29f3ca/frontend.json`. The backend deployment specification was preserved.
- Live browser checks confirmed the quote button opens one focused modal, real EVM sign-in keeps that same dialog, the header reuses it, reload restores the session, and logout works. The modal fits 320px and reports no browser errors. No RFQ or transfer was sent.
- Live API checks confirmed credential-free signed login, authoritative session restoration, proof replay rejection and session revocation. Production EVER verification availability and OTC trading availability retain their existing configuration.


## Remove duplicate Connect wallet buttons — 2026-10-10

Removed the OTC header wallet trigger and the signed-out Exchange account button. “Request exact quote” is the wallet connection entry and continues to open the shared modal. Signed-in “Manage wallets” remains available for ownership verification and logout. The shared modal stays mounted even while its header trigger is hidden.

Svelte checks reported zero errors and warnings; the production image build and all 32 targeted desktop/mobile browser checks passed. Live verification confirmed both Connect wallet buttons are absent, quote-triggered connection and real EVM sign-in work in one modal, session restoration/logout work, and the modal fits 320px. No RFQ or transfer was sent.

Frontend image `wallet-modal-20261010T184023Z-4bb0b923` is live at https://pay3flow.lefine.pro/#/otc, with one ready replica and verified image identity. Frontend source snapshot SHA-256: `4bb0b9235dc4288aa0a19fcf7903a64c4d483bc3cea3235ff684c19269f8a885`. Previous frontend specification: `/home/xila/pay3flow-backups/wallet-modal-20261010T184023Z-4bb0b923/frontend.json`. The backend deployment specification was preserved.


## Use the header wallet picker for OTC — 2026-10-10

Restored the header wallet button and removed the separate OTC wallet form/component. The header and “Request exact quote” now use the original searchable WalletMenu cards, avatars and focus behavior. Ethereum/Everscale cards handle account sign-in or wallet ownership verification; the same picker contains session logout and explicit pending quote submission. The Exchange card has no second account button. Signing in accepts either wallet; settlement wallet proof requirements retain the existing service contract.

Svelte checks reported zero errors and warnings; the production image build passed. Seventy desktop/mobile browser scenarios passed in the regression run. Six sign-in/picker scenarios passed after updating their fixtures for the search-first focus order and waiting for hydration/session restoration before clicking the header after reload. Tests exercise both quote-triggered and header-triggered login, shared-picker identity, cancelled sign-in, authoritative restoration, logout, exact RFQs and transfer safeguards.

Frontend image `wallet-modal-20261010T185255Z-e7d614b0` deployed with one ready replica and verified runtime image identity. Source snapshot SHA-256: `e7d614b02a3fb993790bbd9c9cdde5df73876755e40e65ff26d273986bf3b15a`. Previous frontend specification: `/home/xila/pay3flow-backups/wallet-modal-20261010T185255Z-e7d614b0/frontend.json`. The backend deployment specification was preserved.

Live checks confirmed the restored header trigger, absence of the duplicate OTC form/button, identical searchable wallet choices through both entry points, real credential-free EVM sign-in, session restoration, logout and 320px layout. No browser errors, RFQs or transfers occurred.


## Optional per-trade EVM multisig escrow — 2026-10-12

Implemented Ethereum mainnet USDT escrow using official Safe 1.4.1 contracts and a separate immutable vault. Each trade has buyer, seller and independent arbitrator owners with a 2-of-3 threshold. Both trader consents bind the exact deployment; the guard fixes owners/threshold and limits settlement to the vault. Native EVER remains on Everscale, capped at 100,000 EVER excluding receiving allowance. Both directions fund USDT first. Expiry is four hours from creation, new EVER signing closes for the last 30 minutes, and anyone can request the original-depositor USDT refund at expiry.

Added immutable policy/deployment persistence, verified typed approvals, durable wallet calls, checkpoint recovery for missing references, finalized event observers, evidence invalidation/P0 handling, owned cross-chain refund review, inventory/desk monitoring, and idempotent completion fees/participant confirmations. The existing terminal provides explicit reviews, assigned arbitrator access, receipts, cutoff controls, failed-call retry review and application-independent refund instructions. Existing direct-settlement trades retain their path. The example configuration leaves escrow and live trading disabled.

Verification on the final implementation:

- Foundry offline contract suite: 21 passed, including 256 fuzz runs for timeout conservation, all owner pairs, cap/deadline boundaries, invalid consent, guard bypass, inexact deposits, failed token transfer and surplus recovery.
- Real Ethereum USDT on a local mainnet fork: one passed, exercising non-returning approval/transfer behavior and permissionless expiry refund. RPC access was read-only; balance setup and transactions happened only on the local fork.
- Rust/PostgreSQL OTC suite: 31 passed, including restricted arbitrator access, immutable deployment terms, fee/confirmation deduplication, refund with delivered or unresolved EVER, and invalidated deployment proof blocking completion.
- Local Anvil + Rust/PostgreSQL integration: one passed across Buy and Sell completion, early refund and timeout refund. It executes actual Safe/vault/guard bytecode with mock USDT, tests missing-reference recovery and pre-allowance funding rejection, and uses simulated finalized EVER receipts. The two RPC adapters point to the same local node for this simulation.
- Frontend unit suite: 54 passed, including 16 escrow policy/signature/cutoff checks. An altered EIP-712 schema test and the invalidated-proof accounting test were observed failing before their fixes.
- Browser suites: 10 escrow and 46 direct-settlement desktop/mobile scenarios passed, including 320px accessibility, explicit reviews, reload blocking, confirmed-failure retry, arbitrator permissions, receipts and timeout recovery.
- Svelte diagnostics: zero errors/warnings. Production build passed. Cargo Clippy completed with existing repository warnings; final new escrow code was formatted with rustfmt. `git diff --check` passed.

No contract broadcast, mainnet token approval, live transfer, production rollout or live-route enablement occurred. Contract security review and independently operated chain endpoints are still deployment prerequisites. Real native EVER delivery and staffed recovery evidence are not supplied by the simulated integration run. Setup, routes, build commands and the deployment dry run are documented in `docs/otc-service.md`.


## Production application deployment — 2026-10-12

Backend and frontend are deployed at https://pay3flow.lefine.pro/#/otc using release `otc-safe-20261012T131150Z-571a4227`, isolated source revision `571a42276334e159c30d0318136dbebdbc4ca072`, archive SHA-256 `b9bf53488b81c18c895c8be3b7bc6d57b20d07444887a7bce712fcb25ce61906`. The release was assembled from the actual deployed backend/frontend snapshots and the OTC changes, preserving production provider, TON and interface updates outside this scope. Build and k3s image identities matched; both deployments have one ready replica. A PostgreSQL custom-format backup and original deployment/configmap manifests were saved under `/home/xila/pay3flow-backups/otc-multisig-20261012T130230Z-cda76e18` before rollout; the archive was checked with pg_restore. Six additive escrow tables were applied.

The user selected existing production Safe contracts and declined deployment of new custom contracts. The OTC page now exposes a standard Ethereum Safe 1.4.1 wallet with buyer, seller and arbitrator owners and threshold 2. Creation uses the official existing factory, singleton and fallback handler; all three runtime hashes were checked against Ethereum. No custom vault, guard or factory was deployed, and no mainnet wallet creation or token transfer was broadcast by the agent. The owner wallet creates its Safe only after explicit review in the application. Funding, proposals, approvals and execution use the official Safe application. This flow has no automatic four-hour expiry or unilateral refund. The private escrow policy remains null and desk trading remains disabled; standard Safe setup is available independently.

Validation: 31 backend OTC tests, 58 frontend unit tests, 16 new desktop/mobile browser tests, Svelte check with zero errors/warnings, and a local Anvil creation using official Safe bytecode passed. The local creation verified the predicted address, atomic three-owner/two-approval setup, no modules and zero transferred value. The deployed snapshots contain older OTC test expectations for wallet restoration and focus order: the current workspace OTC tests were run against the isolated release frontend and all 46 passed. Public checks passed for backend health, authenticated escrow routing and Safe review at widths 1440 and 390, including actual Ethereum bytecode reads and no wallet prompt or broadcast. The first rollout check encountered a draining old backend and automatically restored the previous images; the rerun waited for draining pods and completed successfully.

Deployment metadata is saved in `deploy/releases/otc-safe-20261012T131150Z-571a4227.json`.


## New server profile deployment — 2026-10-12

The same verified release `otc-safe-20261012T131150Z-571a4227` is deployed to `root@195.123.218.200`, profile `lefine-next`, namespace `pay3flow`. Both OCI images were transferred from the original server and their CRI IDs matched the original verified builds. The additive namespace has separate PostgreSQL/Redis volumes. Private runtime configuration, signing identity and application secrets were copied securely; no credentials are in the release/profile records. A fresh PostgreSQL dump and Redis snapshot were taken and restored. Database file SHA-256 was verified inside the target pod before a successful restore; 68 public tables and 299 snapshot Redis keys were restored. OTC trade and escrow tables contained no accepted trades.

Backend and frontend each have one ready replica, PostgreSQL and Redis are ready, and Caddy is active. The profile uses ports 30280/30281 to avoid the existing Orator service. Its application route is `/var/lib/lefine-app-host/routes/pay3flow.caddy`. The existing certificate and its renewal metadata were migrated into the new Caddy certificate cache; the domain route retains automatic certificate management. DNS changes were left to the user, as requested. HTTPS checks used the original hostname with an explicit address override to the new server, and the browser asserted the actual response address was 195.123.218.200. Desktop/mobile wallet review passed at widths 1440 and 390 with live Ethereum contract verification and no wallet prompt or transaction broadcast. Health, providers JSON, the frontend and direct server HTTP health passed. Desk quoting remains paused and no custom contracts were deployed.

Profile metadata is in `deploy/profiles/lefine-next.json`; deployment evidence is in `deploy/releases/otc-safe-20261012T131150Z-571a4227-lefine-next.json`. The fresh migration archive and secured manifests are under `/var/lib/pay3flow/migrations/otc-safe-20261012T131150Z-571a4227` on the new server, with source backups under `/home/xila/pay3flow-backups/new-profile-otc-safe-20261012T131150Z-571a4227`.

## Sepolia standard Safe support — 2026-10-15

The standalone multisig component supports Ethereum Mainnet (the initial selection) and Sepolia, chain ID `11155111`. Sepolia is labelled as a test network and links to the [Sepolia ETH faucet](https://cloud.google.com/application/web3/faucet/ethereum/sepolia). Its reads use `https://ethereum-sepolia-rpc.publicnode.com`, transaction links use `https://sepolia.etherscan.io`, and Safe links explicitly use `sep:`. Browser wallet and Reown connections include Sepolia. Creation checks the selected RPC chain, official contract hashes, the reviewed owner account and the wallet's active chain before requesting the factory transaction.

Network records use `pay3flow.otc.production-safe.v1.1` and `pay3flow.otc.production-safe.v1.11155111`. The previous unscoped record migrates to Mainnet with its state and transaction hash preserved; persistence must succeed before the legacy record is removed. Switching networks restores that network's record and is disabled during operations. Both `unknown` and `submitted` states prohibit a second creation after switching or reloading. An unreadable record blocks creation on its network.

Read-only Sepolia RPC verification at block **11921509** confirmed all three official Safe 1.4.1 deployments. The canonical addresses and runtime hashes match the Safe deployment registry for [Safe](https://github.com/safe-global/safe-deployments/blob/main/src/assets/v1.4.1/safe.json), [factory](https://github.com/safe-global/safe-deployments/blob/main/src/assets/v1.4.1/safe_proxy_factory.json) and [fallback handler](https://github.com/safe-global/safe-deployments/blob/main/src/assets/v1.4.1/compatibility_fallback_handler.json):

| Contract | Sepolia address | Runtime keccak256 |
| --- | --- | --- |
| Factory | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` | `0x50c3cdc4074750a7a974204a716c999edd37482f907608d960b2b025ee0b3317` |
| Singleton | `0x41675C099F32341bf84BFc5382aF534df5C7461a` | `0x1fe2df852ba3299d6534ef416eefa406e56ced995bca886ab7a553e6d0c5e1c4` |
| Compatibility fallback handler | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` | `0x7c6007a5d711cea8dfd5d91f5940ec29c7f200fe511eb1fc1397b367af3c42f9` |

The bytecode and 486-byte factory `proxyCreationCode()` response are captured in `pay3low-svelte-frontend/tests/fixtures/safe-1.4.1.json`, with source RPC and block number. A unit test checks fixture runtime hashes. Browser tests use these bytes and a simulated EIP-1193 wallet; their transaction hashes and recovered contract state are fixtures, not Sepolia transaction evidence.

Local verification from `pay3low-svelte-frontend`:

| Check | Result | Coverage and limits |
| --- | --- | --- |
| `npm run test:unit` | 71 passed | Both network configurations/links; legacy state/hash migration; isolated network storage; failed persistence and invalid saved records; official fixture hashes, plus the existing suite. |
| `npm run check` | Passed, 0 errors / 0 warnings | Frontend types and Svelte diagnostics, including both browser-wallet and Reown configuration. |
| `npm run build` | Passed | Dependency bundle warnings remain. |
| `playwright test tests/production-safe.spec.ts` | 34 passed | Desktop and mobile: Mainnet default, Sepolia selection/faucet/links, exact reviewed zero-value factory calldata, wallet network switching, owner and runtime-hash rejection, reload recovery, network locking, explicit rejection retry, lost responses and duplicate-send blocking. Uses simulated wallet/RPC responses. |
| Wallet execution, Everscale wallet, OTC and escrow Playwright suites | 127 passed initially; remaining scenario passed isolated recheck | 128 desktop/mobile regression scenarios. One mobile wallet-menu focus restoration assertion failed once during the run and passed when repeated alone; no wallet-menu source changes were made. |
| Live Sepolia RPC browser review | Passed at widths 1440 and 390 | Real contract reads and predicted-address review with illustrative owner addresses. No injected wallet, signature or broadcast. No horizontal overflow; screenshots inspected locally. |
| `git diff --check` | Passed | No whitespace errors. |

Browser verification used Chromium 152 with the existing `PLAYWRIGHT_CHROMIUM_EXECUTABLE` override and `PLAYWRIGHT_PORT=4173 PLAYWRIGHT_SHARE_PORT=18091` because the default app port was occupied. No live Reown relay session or participant signature was exercised. These are local implementation checks; verified deployment evidence is recorded separately when available.

### Participant-run Sepolia cycle — pending

No participant addresses or wallet signatures were provided during implementation. No Safe creation, faucet request, funding or transfer was broadcast in this work. The following fields remain unverified until the three participants complete the cycle. Use Sepolia ETH only. Create through Pay3Flow, then propose, sign and execute through the official Safe application. Backend OTC settlement, EVER/USDT and the custom four-hour escrow are outside this run.

| Evidence | Actual result |
| --- | --- |
| Buyer / seller / arbitrator public addresses | Awaiting three distinct participant addresses |
| Faucet funding and executor gas balances | Pending |
| Safe address and creation transaction hash | Pending |
| Predicted address equals deployed address; owners equal the three participants; threshold equals `2` | Pending on-chain checks |
| Funding transaction hash; Safe ETH balance before/after | Pending |
| One owner signature cannot execute a proposed transfer | Pending; record Safe transaction hash, confirmation count, unchanged nonce and balances |
| Buyer + seller transfer | Pending; record Safe transaction hash, execution transaction hash, amount, recipient and balances at receipt block |
| Buyer + arbitrator transfer | Pending; record the same evidence |
| Seller + arbitrator refund to original funder | Pending; record the same evidence and final Safe balance |

Suggested sequence after participant addresses are available:

1. Obtain faucet ETH for the creating wallet and every wallet that will pay execution gas. Select **Sepolia (testnet)** in OTC, review the three participant addresses and create the Safe with an owner wallet. Keep the creation transaction hash. Use **Check on-chain status** to verify the deployed address, owner set, singleton and threshold before funding.
2. Open the resulting `sep:` Safe link and confirm Sepolia and the same three owners/threshold. Fund the Safe with a small amount, for example `0.003` Sepolia ETH, and record sender, transaction hash and Safe balances at the blocks before/after funding.
3. Propose a `0.0001` ETH transfer to the seller with buyer + seller as signers. With only the first confirmation, record the proposal hash, `1/2` confirmation state and unchanged Safe nonce/balances. Add the second signature, execute and record the execution receipt and `ExecutionSuccess` event. A successful outer receipt alone does not prove the inner Safe transfer succeeded.
4. Repeat a `0.0001` ETH transfer to the arbitrator using buyer + arbitrator, again observing the single-signature boundary before the second confirmation and execution.
5. Use seller + arbitrator to return the remaining Safe ETH balance to the original funder, observing the same single-signature boundary. For the example amounts and no other transfers or Safe reimbursements, the refund is `0.0028` ETH and the final Safe balance is zero. The executing owner pays gas separately.
6. For every execution, record both the Safe transaction hash and on-chain transaction hash, receipt block, signers, recipient and amount. At fixed before/after blocks, check `Safe_after = Safe_before - amount` and recipient credit; account for gas when a recipient is also the transaction executor. Check the nonce advanced exactly once per executed transfer. Replace the pending rows above with actual links and measured values.

### OTC panel removal requested before deployment — 2026-10-15

The user requested removal of the standalone **OTC multisig wallet** block because wallet connection is already available in the header. `OtcWorkspace` no longer imports or mounts `OtcProductionSafe`. The header wallet menu remains available. Standalone Safe creation is consequently not exposed on the OTC page; the participant-run procedure above describes the retained component and is pending a separate entry point if this workflow is wanted again. Existing saved Safe records are left intact.

The retained component's browser scenarios mount it only through a development test fixture. The existing OTC desktop/mobile layout scenario checks that the production page has no standalone Safe block and still exposes the header wallet connection.

### OTC layout and activity motion — 2026-10-15

The OTC screen now uses shared panel padding (20px desktop / 16px mobile), 56px panel headings, consistent 12–13px supporting text, 16px panel titles, and 44–48px actions. The market strip uses a responsive grid, amount inputs have matching dimensions and stable borders, and desktop Exchange/Chart/Desk offers panels share their top and bottom edges. Chart coordinates follow the available plot dimensions so labels and markers retain their intended sizes. Settlement forms, operator controls and transfer review spacing use the same scale.

Activity tabs have a moving underline and a short content entrance animation. A ResizeObserver animates the enclosing height when the next tab has different content; keyboard navigation and reduced-motion settings remain supported. The **Available networks** heading was removed only from the wallet connection dialog. Currency/payment network pickers retain their headings.

Local visual inspection covered 1440, 1024, 768, 390 and 320px in Russian, plus light/dark desktop/mobile screens. No page overflow, clipped visible labels or browser exceptions were observed. The three desktop panels and their 56px headings aligned; sending and receiving amount fields had matching widths and heights. Populated private-trade screens were also inspected using simulated desk responses. No signature or financial transfer was performed.

Validation for the final UI source: 71 unit tests passed; Svelte check reported 0 errors and 0 warnings; production build and tracked-file secret audit passed. The five browser suites verified 168 distinct desktop/mobile scenarios: 162 passed in the full run and the six new layout/motion cases passed after correcting test selectors for tabs whose accessible names include trade counters. Their checks include both themes, 320–1440px widths, panel/control geometry, populated trade details, moving indicator alignment, animated content height, keyboard navigation and reduced motion. The standalone Safe component is exercised only through its development fixture; it is absent from the product OTC screen.
