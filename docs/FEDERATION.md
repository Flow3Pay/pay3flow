# Federation

Pay3Flow uses federation for marketplace discovery through a configured fmatch
service. This document describes the implementation in this repository, which
is an experimental MVP.

## Supported federation protocols and standards

- [ActivityPub](https://www.w3.org/TR/activitypub/) (Server-to-Server): a
  limited marketplace profile for actor discovery, proposals, and replies.
- [ActivityStreams 2.0](https://www.w3.org/TR/activitystreams-core/): actor,
  activity, and collection representations with marketplace extensions.
- [WebFinger](https://www.rfc-editor.org/rfc/rfc7033): account discovery at
  `GET /.well-known/webfinger?resource=acct:{handle}@{domain}`.
- [HTTP Signatures (Cavage draft)](https://datatracker.ietf.org/doc/html/draft-cavage-http-signatures):
  RSA-SHA256 signatures for outbound activities and optional inbound verification.

## Supported FEPs

- [FEP-67ff: FEDERATION.md](https://fep.swf.pub/fep/67ff/fep-67ff.html):
  this file provides federation documentation at the repository root.
- [FEP-0837: Federated Marketplace](https://fep.swf.pub/fep/0837/fep-0837.html):
  partial support for `Proposal` and `Intent` objects describing offers and
  requests. The discovery commands and candidate replies are fmatch extensions;
  the general inbox does not implement the full agreement lifecycle.
- [FEP-8fba](https://w3id.org/fep/8fba#): experimental exchange interface
  metadata, input/output shapes, preview, and invocation links. See the
  [implementation](../backend/src/activitypub/fep8fba.rs) for the exact vocabulary
  and payloads supported by Pay3Flow.

## Actors and discovery

The backend publishes two actors under its configured `ap_origin`:

| Actor | Identifier | Capabilities |
| --- | --- | --- |
| Primary | `/actor/{ap_handle}` | Exchange and acquiring discovery |
| P2P | `/actor/{ap_handle}-p2p` | P2P offer advertisements |

Both actors have types `Service` and `Application`, share an RSA signing key,
and advertise `/inbox` as their inbox and shared inbox. Each actor publishes
its own `publicKey.id` (`{actor_id}#main-key`), `owner`, and `publicKeyPem`.
`GET /actor` lists their identifiers in an `OrderedCollection`.

WebFinger returns the actor identifier in a `self` link with type
`application/activity+json`. Lookup currently selects the P2P actor when the
resource contains its handle and otherwise returns the primary actor; it does
not strictly validate the requested account or domain.

The actor and discovery handlers currently return `application/json` through
Axum's JSON responses, without ActivityPub content negotiation. Actor documents
advertise `/outbox`, but the backend does not serve that collection. NodeInfo,
ActivityPub Client-to-Server, and general social timelines are not implemented.

## Activities and marketplace extensions

On startup, both actors send `Follow` activities to the configured
`fmatch_actor_id` through `fmatch_inbox`. After the primary Follow succeeds,
Pay3Flow publishes its exchange proposal in a `Create` activity. Provider
advertisements may be sent individually as proposals or batched in a
`Create(OrderedCollection)` using the P2P actor.

Proposals use the contexts `https://www.w3.org/ns/activitystreams` and
`https://w3id.org/fep/0837`. They contain `purpose`, `attributedTo`, and a
`publishes` intent with `action`, `resourceConformsTo`, and `resourceQuantity`.
Discovery requests add `command`, `content`, and `resultInbox`; P2P requests
also include `candidatePageSize`. The request contract and examples are in
[backend-to-fmatch.org](backend-to-fmatch.org).

`POST /inbox` and `POST /inbox/{handle}` use the same handlers:

- `Follow` returns an `Accept` inline in the HTTP response. It does not maintain
  a followers collection or deliver the acceptance to the remote inbox.
- Candidate arrays at `candidates` or `object.candidates` are stored in memory.
  `Offer`, `Accept`, and `Reject` receive basic handling and logging.
- A `Proposal` with `purpose=request` and `command=quote` is accepted only when
  its author is the configured fmatch actor. Its `content` identifies the route,
  amount, and asset; the response is an inline `Offer` containing a `RouteQuote`.
- Other activity types receive an accepted acknowledgement without further
  processing. An acknowledgement does not indicate support for that activity.

The exchange interface is exposed at `/marketplace/interfaces/exchange`, with
its proposal at `/marketplace/proposals/exchange`, resource at
`/marketplace/resources/exchange`, and shapes at
`/marketplace/shapes/exchange-input` and `/marketplace/shapes/exchange-output`.
`POST /marketplace/preview` computes a preview; the invocation link targets the
separate `/api/exchange/orders` application workflow.

The optional OTC service uses relay-hosted participant actors and the custom
`pay3flow-otc-fep0837-v1` profile for private negotiations and settlement.
It is disabled by default and restricted to the configured relay and desk.
Its agreement messages are separate from the general inbox described above;
see [OTC service documentation](otc-service.md) for its contract and limits.

## Signatures and delivery

Outbound delivery uses `Content-Type: application/activity+json` and a
`Signature` header with `algorithm="rsa-sha256"`. The signature covers
`(request-target) host date digest`; `Digest` is the Base64-encoded SHA-256 of
the request body. The request target includes the query string when present.

Inbound verification is controlled by `ap_require_signatures` (default `false`
in [config.toml](../config.toml)). When enabled, all four signed components and a
date within five minutes of the server clock are required. The signing key ID
must identify the activity author (`attributedTo` for proposals, `actor` for
other activities). Remote keys are fetched from the actor document, whose
`publicKey.id` and `publicKey.owner` must match the requested key and actor.

Processed inbound activity IDs are recorded in PostgreSQL; repeated IDs return
an `exists` response. Normal outbound deliveries track successful activity-ID
and target pairs. Ephemeral P2P discovery requests skip the outbound ledger,
but retain the same activity ID across retries within a call. Delivery uses a
15-second per-request timeout and up to three retries for network errors,
HTTP 429, and server errors, except recognized no-candidate rejections.

## Additional documentation

- [Configuration](configuration.md) and [config.toml](../config.toml):
  `ap_origin`, `ap_handle`, `ap_key_path`, `ap_require_signatures`,
  `fmatch_inbox`, and `fmatch_actor_id`.
- [API overview](api-overview.md): Pay3Flow HTTP and WebSocket routes.
- [Exchange discovery](fmatch-exchange-discovery.md) and
  [offer schema](fmatch-offer-schema.md): fmatch-specific payloads.
- [fmatch API](fmatch-api.md): the separate matcher's endpoints, including
  its NodeInfo and outbox support; these are not Pay3Flow backend endpoints.
- [ActivityPub source](../backend/src/activitypub) and
  [HTTP handlers](../backend/src/server/routing/activitypub/mod.rs).
