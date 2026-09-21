# fmatch offer schema

In fmatch, a solver or acquirer is represented as a FEP-0837 offer. It is a
capability profile: identity, supported resources, currencies, geography, fees,
limits, and status. fmatch stores offers in `marketplace_proposals` with
`purpose='offer'` and indexes them in Typesense.

## Follow comes first

An offer is not accepted until its actor has an active
`marketplace_subscriptions` entry for the same `resourceConformsTo` and
`action` (`has_active_marketplace_subscription` in `entry.rs`).

The sequence is:

1. Publish an actor document whose `attachment` declares the capability,
   resource, action, and purpose.
2. Post a `Follow` to `/inbox/actra` with `actor` set to the actor IRI.
3. After fmatch fetches the actor and creates the subscription, post the offer
   proposal with `purpose: "offer"`.

## Example offer

```json
{
  "@context": [
    "https://www.w3.org/ns/activitystreams",
    "https://w3id.org/fep/0837"
  ],
  "type": "Proposal",
  "id": "https://pay3flow.dev/solvers/demo#offer",
  "purpose": "offer",
  "attributedTo": "https://pay3flow.dev/actor/pay3flow",
  "name": "Demo solver (US, USD/EUR, 3.1% card)",
  "content": "geo:US|EU; currencies:USD,EUR; fee:3.1% + 0.50 USD; limits:min 1, max 100000 USD",
  "publishes": {
    "type": "Intent",
    "id": "https://pay3flow.dev/solvers/demo#intent",
    "action": "deliverService",
    "resourceConformsTo": "https://pay3flow.dev/marketplace/resources/acquiring",
    "resourceQuantity": {"hasUnit": "one", "hasNumericalValue": "1"}
  },
  "to": ["https://www.w3.org/ns/activitystreams#Public"]
}
```

Only a subset is validated as protocol structure. Domain details such as
currencies, geography, fee, limits, and status are currently carried in
structured `name`/`content` text and preserved in `source_activity`.

## Matching semantics

| Concept | Field | Meaning |
| --- | --- | --- |
| Resource | `publishes.resourceConformsTo` | The acquiring resource contract |
| Action | `publishes.action` | `deliverService` |
| Unit | `publishes.resourceQuantity.hasUnit` | `one` or `currencyAmount` |
| Audience | `to` | Usually public for an offer |
| Description | `name`, `content` | Human-readable and domain-specific details |

The request and offer use the same resource contract:
`https://pay3flow.dev/marketplace/resources/acquiring`. fmatch compares the
resource path; the backend performs the detailed currency, geography, limit,
and ranking checks.

## Updates and deactivation

- Re-posting the same `proposal.id` updates the offer.
- To disable a solver, mark its local status inactive; fmatch has no dedicated
  deactivation endpoint.
- Repeating the same offer ID is idempotent and results in an existing/upserted
  record.
