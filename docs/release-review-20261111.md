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
