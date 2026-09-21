# Deployment and release guide

Pay3Flow is currently an experimental MVP. The Compose setup is intended for
development and demonstrations, not as a production deployment blueprint.

## Release sequence

1. Review the code and database migration changes.
2. Run formatting, backend tests, frontend checks/build, and the secret audit.
3. Start an isolated environment and run `scripts/exchange-flow.sh`.
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
health endpoints. `scripts/healthcheck.ps1` checks the Compose services from
PowerShell; use the curl commands in `docs/development.md` on Unix-like hosts.

Logs may contain IDs, statuses, and reason codes. They must never contain PAN,
CVC, private keys, bearer tokens, or raw sensitive proof payloads.

## Rollback

Do not roll back a schema or settlement change blindly. First disable new
orders with the global exchange control, preserve audit and settlement data,
assess in-flight orders, and follow the migration's rollback/runbook plan.
Keep backups and image versions long enough to investigate an incident.
