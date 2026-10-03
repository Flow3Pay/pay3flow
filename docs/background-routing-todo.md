# Background routing implementation TODO

This checklist tracks the Pay3Flow/Fmatch routing work in the isolated
`pay3flow-fiat-progressive` worktree. Do not push until all items are complete.

## Expanded market coverage

- [x] Bound startup polling to a three-worker priority queue, keep one worker
  available for requested directions, and cap local offer snapshots at 128.
- [x] Enumerate every supported exchanger currency and asset direction from
  the configured catalogs; refresh them continuously with bounded concurrency.
- [x] Keep quote coefficients for every supported public route provider pair in
  the background, with popular and requested directions first.
- [x] Search P2P on demand, while refreshing popular P2P pairs in the background.
- [x] Verify ADA to AMD accepts a BestChange offer without a minimum, and
  uncommon catalog assets remain in background polls.
- [x] Update docs, run checks, commit slices, then push the finished fix.

- [x] Keep the shared route response cache in Redis for 15 seconds.
- [x] Move direct exchanger offer discovery, including Fmatch calls, to startup
  and periodic background refreshes.
- [x] Read offer snapshots from memory or Redis during user searches; retain
  progressive route composition and the current response shape.
- [x] Poll P2P snapshots on a 15/30/60-second demand cadence and exchanger
  snapshots every five minutes.
- [x] Start background spot, direct fiat, and public route quote polling at
  service startup.
- [x] Test cold and warm exchanger snapshots and the P2P on-demand path.
- [x] Keep anonymous instruction and link actions in Redis, detect spam, roll
  back a spam session to its first action, then flush aggregates safely to SQL.
- [x] Calculate provider reputation from 50 points, with +10 per like, -15 per
  dislike, +5 per instruction open, +5 per link open, -15 per inactive day, and
  bounds of 0 to 100. Cache the result in Redis for background search priority;
  it must not affect route order.
- [x] Rank close-priced routes using only likes/dislikes; keep the original
  price order when there are no votes.
- [x] Complete API/frontend types and documentation for instruction opens and
  reputation. Verify backend and frontend checks plus Redis behavior.
- [x] Validate the migration and write-behind flush SQL against PostgreSQL.
- [x] Commit the finished slices separately, then push the completed work to
  `master`. Change Fmatch only if its code needs a matching contract update.
