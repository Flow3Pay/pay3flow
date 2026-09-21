# Pay3Flow documentation

This directory contains the project documentation. The repository is an
experimental MVP; documents describe the current implementation unless they
explicitly say `planned`, `research`, or `legacy`.

## Start here

1. [`../README.md`](../README.md) — product summary and quick start.
2. [`architecture.md`](architecture.md) — service boundaries and the order flow.
3. [`development.md`](development.md) — local setup, checks, and smoke tests.
4. [`configuration.md`](configuration.md) — environment variables and secrets.
5. [`api-overview.md`](api-overview.md) — HTTP and WebSocket route families.

## Domain and protocol references

- [`exchange-domain.md`](exchange-domain.md) — exchange tables, invariants, and status machines.
- [`exchange-risk-compliance.md`](exchange-risk-compliance.md) — MVP safety controls and production gates.
- [`glossary.md`](glossary.md) — shared vocabulary.
- [`backend-to-fmatch.org`](backend-to-fmatch.org) — the backend-to-fmatch ActivityPub request contract.
- [`fmatch-api.md`](fmatch-api.md) — fmatch endpoints and accepted activities.
- [`fmatch-offer-schema.md`](fmatch-offer-schema.md) — the solver offer format.
- [`fmatch-candidates.md`](fmatch-candidates.md) — current demo candidates.
- [`fmatch-exchange-discovery.md`](fmatch-exchange-discovery.md) — discovery flow.

## Product, research, and operations

- [`p2p-search.md`](p2p-search.md) — read-only P2P route search.
- [`public-p2p-sources.md`](public-p2p-sources.md) — source policy and privacy boundaries.
- [`route-aggregation-research.md`](route-aggregation-research.md) — possible future quote sources.
- [`cow-services-analysis.md`](cow-services-analysis.md) — what is useful from the CoW reference checkout.
- [`roadmap-cow.md`](roadmap-cow.md) — migration roadmap and outstanding work.
- [`../cross-border-payments-providers.md`](../cross-border-payments-providers.md) — provider research; verify all commercial details before use.
- [`deployment.md`](deployment.md) — deployment and release checklist.

## Documentation rules

- Describe the behavior that exists in the repository; label proposals and
  research as such.
- Link to source files and endpoint paths when a detail is implementation-specific.
- Never document development credentials as production secrets.
- Keep user-facing settlement, TOKEN, fee, and consent language explicit.
- Update this index when adding a document.
