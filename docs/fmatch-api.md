# fmatch API reference

fmatch is an ActivityPub matcher. Its PostgreSQL tables
`marketplace_subscriptions` and `marketplace_proposals` are the source of truth
for advertised capabilities and proposals. The HTTP paths are ActivityPub
transport surfaces, not conventional REST resources.

Local development uses `http://localhost:7277`, fmatch PostgreSQL on `5433`,
and Typesense on `8108`.

## Surfaces

| Path | Method | Purpose |
| --- | --- | --- |
| `/health` | `GET` | Return `{"status":"ok"}` |
| `/typesense/health` | `GET` | Check the Typesense connection |
| `/actor`, `/actors` | `GET` | List actors as an `OrderedCollection` |
| `/actor/:handle` | `GET` | Return an ActivityStreams actor document |
| `/inbox`, `/inbox/:handle` | `POST` | Accept activities in the shared or actor inbox |
| `/outbox/:handle` | `GET` | Return pending activities |
| `/follow` | `POST` | Follow a remote actor on behalf of fmatch |
| `/.well-known/webfinger?resource=acct:...` | `GET` | Resolve an account |
| `/.well-known/nodeinfo`, `/nodeinfo/:ver` | `GET` | Return NodeInfo 2.0/2.1 |
| `/marketplace/resources/:name` | `GET` | Describe a resource contract |
| `/activitypub/object?iri=...` | `GET` | Fetch a remote ActivityPub object |

The default actor is `actra` (`https://<domain>/actor/actra`) and its inbox is
`/inbox/actra`.

## Accepted activity types

Validation is performed by `validate_marketplace_inbox_entry`.

- `Follow` subscribes to a remote actor and creates a marketplace subscription.
- `Proposal` and `Note` carry FEP-0837 offers or requests.
- `Create` and `Offer` may wrap `Proposal`, `Note`, `OrderedCollection`, or
  ForgeFed ticket-like objects.
- `Update(Proposal)` and `Update(Agreement)` update existing records.
- `OfferAgreement`, `AcceptAgreement`, and `RejectAgreement` handle settlement.
- `Accept`, `Reject`, and `Undo` provide settlement acknowledgements.

## Submit a payment request

Post a JSON-LD FEP-0837 proposal to `/inbox/actra`:

```json
{
  "@context": [
    "https://www.w3.org/ns/activitystreams",
    "https://w3id.org/fep/0837"
  ],
  "type": "Proposal",
  "id": "https://pay3flow.local/proposals/req-1",
  "purpose": "request",
  "attributedTo": "https://pay3flow.local/actor/pay3flow",
  "name": "Send 100 USD to a TR bank account",
  "content": "100 USD, from US, to TR, bank transfer",
  "publishes": {
    "type": "Intent",
    "id": "https://pay3flow.local/proposals/req-1#intent",
    "action": "deliverService",
    "resourceConformsTo": "https://pay3flow.dev/marketplace/resources/acquiring",
    "resourceQuantity": {"hasUnit": "one", "hasNumericalValue": "1"}
  },
  "to": ["https://pay3flow.local/actor/pay3flow"]
}
```

The validator requires ActivityStreams and FEP-0837 contexts, a valid proposal
identity and purpose, `attributedTo`, `to`, and an `Intent` with an action,
resource contract, and quantity unit. Quantities may be strings.

## Responses and commands

fmatch returns an ActivityPub object rather than a REST wrapper:

- successful matching returns `Offer(Agreement)` with a ranked `candidates`
  list;
- intermediate work returns `Accept` with a status;
- no candidates returns `Reject` with `status=no-candidates` and HTTP 503;
- execution failure returns `Reject` with `execution-failed` and HTTP 502.

For asynchronous results, provide `resultInbox`. Discovery commands
`new`, `candidates`, and `findbestpath` return candidates without execution;
`execute[:N]` selects candidate `N` for execution.
