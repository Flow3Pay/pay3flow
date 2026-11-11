# Georgian production release review

The release updates the deployed `cca7008` source through the current default
branch (`master`) and adds Georgian banks, GEL/EUR benchmarks and Solunex.

Independent round 1 reviewed the full unreleased range and reported seven
findings. All seven were checked against the implementation and addressed:

1. Payment polling now rotates pending batches using `last_polled_at`, excludes
   completed delivery/refund orders. The actual SQL failed with 51 pending orders
   before the fix and passes against disposable PostgreSQL after it. Round 3
   corrected the first repair: a price expiry is not proof of settlement.
   Accepted addresses in underpaid, expired, cancelled or failed local states
   remain observed until actual delivery or refund.
2. Warm provider capabilities remain usable during catalog refresh or outage.
   Solunex refresh failures back off; initial discovery has a time limit. A delayed
   provider reproduced the blocked warm-cache read before the fix.
3. Payers cannot reopen cancelled or failed checkouts. The state update also
   checks the previously observed status, protecting concurrent cancellation.
   A checkout with an accepted address cannot supersede it with another address;
   a merchant creates a separate checkout for a new price.
4. Webhooks resolve only public destinations, pin those addresses for delivery,
   retain HTTP(S)/custom-port compatibility, disable proxies and refuse redirects. Private-address and
   local-DNS checks cover the boundary.
5. Missing or expired provider deadlines fail with an explicit unavailable error
   instead of returning an already-expired quote. No replacement deadline is
   invented for an executable quote.
6. API key coverage documentation now states that the route inventory is
   hand-maintained; router-completeness enforcement remains pending.
7. Checkout replay returns the stored return URL and current stored status.

The three initial regression tests failed against the old behavior. A second
run passed those repairs and reproduced the missing-deadline and slow-catalog
failures. The production receipt records the final checks, fresh review outcome,
source identity and rollback snapshot after promotion.

Round 2 was aborted after a compilation failure and did not clear a release.
Round 3 found six items. The settlement and underpayment findings were accepted
and reproduced before repair; webhook compatibility and startup-catalog recovery
were repaired. The low-priority backoff timing issue now records completion even
on cancellation. The supplemental SQL harness now requires a disposable-container
label and test database name. Automatic hosted-CI wiring remains deferred because
this repository has no hosted Docker fixture; the release explicitly runs the
real-database harness in addition to Cargo tests.

Backend image assembly copies application inputs in one step, preserving the
separate dependency-cache layer. This avoids repeated snapshots of the large
Rust dependency layer on the production host; environment files remain excluded
from the build context.


Round 4 identified remaining B2B launch issues (observation capacity, expiry
notifications, address replacement and webhook transport). They are recorded in
`b2b-launch-blockers.md`. They do not belong to the Georgian-support launch:
B2B routes, OpenAPI operations and worker execution are now disabled by default,
including for older production configurations. The already deployed service did
not expose this API (public `/api/b2b/orders` returned 404 before the change).
The shared warm-cache regression was reproduced (one failing test, three gate
checks passed) and repaired; a failed refresh now preserves non-empty assets.
The index-name change also handles earlier prototype databases safely.


Round 5 reviewed the final gate/cache repair and found three deferred items:

| Finding | Classification | Reason deferred | Revisit trigger |
| --- | --- | --- | --- |
| A catalog outage at process startup can hide that provider until the next successful 300-second refresh. | Pre-existing, Medium | Bounded recovery; warm catalogs now survive outages. Live Solunex discovery and quotes passed after this rollout. | Shorten recovery when startup outages recur or availability requirements demand recovery below five minutes. |
| Background coefficient warmup snapshots provider assets once per process. | Pre-existing, Low | Live searches still quote; the effect is latency and cache freshness. | Rework background warmup when provider catalogs change during runtime or measured search latency warrants it. |
| Disabled B2B endpoints return 404 outside tracing, metrics and CORS middleware. | Introduced, Low | Disabled prototype paths have no active product flow; request counters remain balanced. | Revisit before B2B launch or when monitoring of blocked paths is needed. |

Round 5 satisfies the release stopping rule: no introduced finding above Low,
and no High or Critical remains in the enabled Georgian release. The separate
B2B launch blockers remain behind the default-off gate; this is not B2B launch
clearance. The production deployment and live verification are recorded in
[`georgia-20261111.json`](../deploy/releases/georgia-20261111.json).

Round 6 reviewed the currency-picker correction (`56078c3..3e9c1c2`) and
reported three findings. The code correction was confirmed correct. Triage:

1. Existing selector tests expect method-scoped lists, whereas the existing
   discovery tests and shipped converter require global currency choices. The
   claim that the new Georgian regression cannot pass was refuted by the actual
   six-test run (exit 0), including both desktop and mobile, and by the compiled
   bundle tested against production data. The added `ANY` assertion restates the
   pre-existing discovery contract; it introduces no new inconsistency or user
   behavior. Older full-suite expectations remain pre-existing test debt. Defer
   their alignment because this release preserves the global discovery contract;
   revisit before treating the full browser suite as a release gate. An
   accidentally unfiltered full run was interrupted (exit 130), not passed.
2. Legacy picker rows can remain visible to catalog consumers, including an
   `ANY` chip on About. This pre-existing display issue is deferred: deleting
   production directory records is unnecessary for the selector fix. Revisit
   when catalog ownership/retirement is specified or About's currency list is
   refreshed. The converter now tolerates the existing catalog safely.
3. The missing receipt link is documentation-only and is completed by the
   post-deploy receipt with the actual running revision and image IDs. Round 5
   cleared its reviewed changes, not the subsequently discovered live wildcard
   case; this round and the final live browser smoke cover that correction.

After triage, Round 6 has no introduced finding above Low and no open High or
Critical in the enabled release. Full-suite browser test debt is explicitly
not represented as green; the relevant six-test suite, compiled production-data
browser check, unit tests, type check and build passed.
