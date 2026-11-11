# Georgian production release review

The release updates the deployed `cca7008` source through the current default
branch (`master`) and adds Georgian banks, GEL/EUR benchmarks and Solunex.

Independent round 1 reviewed the full unreleased range and reported seven
findings. All seven were checked against the implementation and addressed:

1. Payment polling now rotates pending batches using `last_polled_at`, excludes
   terminal orders and expires pending deposits even when the provider continues
   reporting `PENDING_DEPOSIT`. The actual SQL failed with 51 pending orders before
   the fix and passes against disposable PostgreSQL after it.
2. Warm provider capabilities remain usable during catalog refresh or outage.
   Solunex refresh failures back off; initial discovery has a time limit. A delayed
   provider reproduced the blocked warm-cache read before the fix.
3. Payers cannot reopen cancelled or failed checkouts. The state update also
   checks the previously observed status, protecting concurrent cancellation.
4. Webhooks resolve only public destinations, pin those addresses for delivery,
   require HTTPS/443, disable proxies and refuse redirects. Private-address and
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
