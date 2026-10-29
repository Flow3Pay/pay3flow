# Exchange and arbitrage discovery

The existing Exchange converter includes Any currency and Any method in its
sending and receiving pickers. The regular Find routes action searches wildcard
selections through the existing search state, route cards, loading and empty
states, provider filters, feedback, selection and instructions. There is no
separate wildcard results component. The ordinary refresh action and automatic
refresh continue remaining combinations for unchanged selections; edits start
a new search. Comparisons between currencies use value ratios rather than
nominal output amounts. Instruction links use each route's resolved methods. Shared wildcard
selections use `/#/swap/ANY/ANY?from=any-ANY&to=any-ANY`; older discovery links
are restored inside Exchange. Arbitrage remains available through the API.
RUB supports the selectable SBP / СБП method in both directions. The method
catalog includes enabled directory methods, and provider availability still
determines whether an amount and method can produce a route.

`GET /api/p2p/discovery` accepts `mode=exchange|arbitrage`, `source_fiat` and
`target_fiat` (default `ANY`), and required decimal-string `source_amount`.
Method constraints are stable `/api/banks` method ids; directory rows without
`method_id` use `directory-{id}`. Omission means any supported method. Selecting
a method excludes routes whose provider cannot verify that endpoint method. Networks,
intermediary assets, sources, exchange mode, reputation filters, action count
(2–8), price deviation and result limit follow concrete route search semantics.
Arbitrage defaults to four actions and intermediate cross-venue transfers.

The same numeric amount is searched separately in each origin currency, not an
equal USD budget. Arbitrage must close at the identical currency, network,
verified payment method and venue. Unknown endpoint identity excludes a cycle.
Quotes use the first leg's actual output for the next leg. Fees that are already
included in provider output are not charged twice. Unknown required costs keep
profit unconfirmed. Native wallet gas and deposit costs are not inferred to be
zero. Nothing in discovery submits an order, signs a transaction or sends funds.

Ranked USD profits use fresh identified crypto prices and live fiat/USD provider
coefficients; mock payment FX rates are never used. Missing valuations remain
visible without a combined USD profit rank. Exchange compares USD value
received per USD value sent. These valuations are indicative, not execution
quotes. Every result includes resolved settlement endpoints, missing costs,
observation time and valuation provenance.

A batch schedules up to 16 concrete combinations, at most four concurrently,
and stops new work after 30 seconds. Existing venue request serialization,
retries and provider quote limits still apply. Catalog loading and valuation
can precede the route batch. The endpoint never promises the global optimum:
`routes_exhaustive=false` also reflects the existing bounded graph composers.

`/ws/p2p/discovery` accepts the same query as JSON. It emits `routes_updated`
and `search_finished` with a `response`, or `search_failed` with an `error`.
Closing the socket cancels the search. Response coverage lists total/completed
combinations, failed combinations and unfinished combinations at the deadline.
Provider failures remain visible alongside usable partial results.

`next_cursor` resumes remaining combinations with an unchanged query and
catalog. Redis holds traversal metadata only for 15 minutes; it never holds P2P
offers as discovery answers. Sampled combinations are not repeated within a
continuation. Partial inner route searches stream immediately and advance the
traversal; combinations that produced no snapshot are retried. Coverage reports
the current batch's partial combinations separately. A changed catalog or expired
cursor requires a new search. Redis failure permits initial search results but
cannot provide continuation. The browser merges fresh continuation results by
route id, removes expired estimates, and keeps at most 100 results.

The SBP payment method uses the unmodified compact official logo from
https://sbp.nspk.ru/storage/settings/common/logo_mobile/5e648604-f95d-472d-afe2-33b856f345ed.svg
(the logo used by the mobile header of https://sbp.nspk.ru/), stored locally at
`pay3low-svelte-frontend/static/icons/assets/sbp.svg`. The shared payment-method
icon mapping displays it in the existing picker, converter, cards and instructions.
