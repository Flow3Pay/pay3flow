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
npm run test:e2e -- tests/otc.spec.ts tests/otc-settlement.spec.ts
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
