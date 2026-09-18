# fmatch exchange discovery

EX-4 adds solver discovery for the new exchange orderbook.

## Request payload

The backend sends an ActivityPub/FEP-0837 request proposal through the existing
`ActivityPubService::submit_request("candidates", content)` path. The proposal
shape and HTTP signature are the same as the legacy payment quote flow; only
the `content` string is exchange-specific.

Content format:

```text
exchange order {order_id}; send {source_amount_minor} {source_currency} from {source_country}/{source_currency} via {source_method_type} to {target_country}/{target_currency} via {target_method_type}; source_country={source_country}; source_currency={source_currency}; target_country={target_country}; target_currency={target_currency}; source_method={source_method_type}; target_method={target_method_type}; source_amount_minor={source_amount_minor}
```

Example:

```text
exchange order ...; send 100000 AMD from AM/AMD via card to RU/RUB via bank; source_country=AM; source_currency=AMD; target_country=RU; target_currency=RUB; source_method=card; target_method=bank; source_amount_minor=100000
```

## Response parsing

The backend accepts the same fmatch candidate surfaces as the existing
ActivityPub parser:

- top-level `candidates`;
- `object.candidates` in an `Offer` reply.

Each candidate is normalized into `exchange_solvers` with:

- `slug` from `shortId`, falling back to a slugified `name`;
- `status = discovered`;
- `countries`, `currencies`, and `rails` from the order corridor;
- `fee_model.source = fmatch`, plus rank/price/quality metadata;
- `risk_score` derived from quality for MVP ranking.

## Fallback order

`POST /api/exchange/orders/:id/discover` uses this order:

1. live fmatch request;
2. Redis candidate cache with 5 minute TTL;
3. local `exchange_solvers` registry filtered by corridor, rails, amount limits,
   and `active|discovered` status.

On success the order advances with guarded status transitions:

```text
created -> discovering -> quoting
```

If the order is already `discovering`, `quoting`, or `quoted`, discovery is
idempotent from the API user's point of view and returns the best available
candidate source.
