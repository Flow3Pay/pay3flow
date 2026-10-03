# Background routing implementation TODO

This checklist tracks the Pay3Flow/Fmatch routing work in the isolated
`pay3flow-fiat-progressive` worktree. Do not push until all items are complete.

- [x] Keep the shared route response cache in Redis for 15 seconds.
- [x] Move P2P and direct exchanger offer discovery, including Fmatch calls, to
  startup and periodic background refreshes.
- [x] Read offer snapshots from memory or Redis during user searches; retain
  progressive route composition and the current response shape.
- [x] Poll P2P snapshots on a 15/30/60-second demand cadence and exchanger
  snapshots every five minutes.
- [x] Start background spot, direct fiat, and public route quote polling at
  service startup.
- [x] Verify all production route paths avoid live provider calls in user
  request handling, and test cold and warm searches.
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
- [ ] Commit the finished slices separately, then push the completed work to
  `master`. Change Fmatch only if its code needs a matching contract update.
