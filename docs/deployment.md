# Deployment and release guide

Pay3Flow is currently an experimental MVP. The Compose setup is intended for
development and demonstrations, not as a production deployment blueprint.

For production k3s, use `nix run .#deploy` and
[`deploy/README.md`](../deploy/README.md). This builds the backend and
frontend on the production host, imports their images into k3s, rolls out both
Deployments, and checks the public endpoints. The separate
`nix run .#deploy-k8s` path builds with in-cluster Kaniko Jobs and supports
either persistent in-cluster PostgreSQL and Redis or externally managed URLs.

## Current production build and runtime

`nix run .#deploy` deploys the current committed revision, rather than implicitly
checking out remote master. Push and verify the intended commit first. It creates
a release directory and builds immutable images on the production host, verifies
their k3s image IDs, records source-revision annotations, rolls out backend and
frontend, and checks the public root, health and provider catalog.

Frontend builder and runner inherit the dependency stage. This avoids hashing
and copying the large standalone wallet SDK tree between stages. The build uses
`NODE_OPTIONS=--max-old-space-size=6144`; local `.env*` files are excluded from
the frontend Docker context. Cold dependency installation, image export and
containerd import can still dominate release time on a busy disk.

The Aurora widget needs `PUBLIC_AURORA_INTENTS_API_KEY` in the frontend Node
server environment. Keep the value in the production secret store and use a
Secret reference from the frontend Deployment; do not depend on a local
build-time `.env.local`. This key is distinct from backend `NEAR_INTENTS_JWT`.
See [runtime configuration](configuration.md#frontend-runtime-configuration)
and the [production deployment guide](../deploy/README.md).

## Release sequence

1. Review the code and database migration changes.
2. Run formatting, backend tests, frontend checks/build, and the secret audit.
3. Start an isolated environment and run `ruby scripts/exchange_flow.rb`.
4. Check health endpoints and inspect logs for startup or migration errors.
5. Verify the public frontend displays fees, route details, settlement-asset
   disclosures, and consent before funding.
6. Back up the database and record the image/version identifiers.
7. Deploy with production secrets and feature gates disabled by default.
8. Run a small, non-financial verification flow and monitor errors before
   enabling any additional corridor or solver.

## Required production work

Before real-money use, the operator must complete corridor-specific legal and
compliance review, including regulated roles, KYC/AML, sanctions screening,
source-of-funds rules, consumer disclosures, complaints, chargebacks, data
retention, and suspicious-activity handling.

The technical review must also cover:

- authenticated solver APIs and signed callbacks;
- custody, key management, and reconciliation;
- independent proof sources and dispute procedures;
- rate limits, per-user/per-solver/per-corridor limits, and alerting;
- database backups and restore drills;
- incident response, rollback, and operator runbooks;
- TLS, network isolation, dependency updates, and access review;
- four-eyes approval for admin actions and kill-switch changes.

## Health and observability

The backend exposes `GET /health`. fmatch and Typesense expose their own
health endpoints. The frontend also exposes `/frontend-metrics` through its
production server. Use the curl commands in `docs/development.md` to check
the main services.

Logs may contain IDs, statuses, and reason codes. They must never contain PAN,
CVC, private keys, bearer tokens, or raw sensitive proof payloads.

## Post-rollout verification

Confirm both Deployments are ready, use the intended image tag, and have the
expected `pay3flow.io/source-revision` annotation. Check `/health`, `/`,
`/api/providers` and the frontend metrics route. A successful image build alone
is not a completed deployment.

Use a read-only browser flow to check SWAP/guide switching and widget loading.
For Aurora, confirm the token catalog authenticates and the selected pair and
input appear; no wallet signature or funded transaction is needed for that
check. Provider iframe/storage restrictions remain separate from app readiness.

Documentation-only commits after an application deployment do not require
rebuilding identical application images. Record the deployed application
revision separately from later documentation commits.

## Rollback

Do not roll back a schema or settlement change blindly. First disable new
orders with the global exchange control, preserve audit and settlement data,
assess in-flight orders, and follow the migration's rollback/runbook plan.
Keep backups and image versions long enough to investigate an incident.
