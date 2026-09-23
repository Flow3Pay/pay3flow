# Providerfile catalog

Providerfile is the single source of truth for the provider catalog. Adding a
provider does not require a provider-specific Rust module, a registration
function, or handwritten SQL. One generic command reads every Providerfile and
generates the database migration.

## Add a provider

Run these commands from the repository root:

```bash
mkdir -p backend/providers/my-provider
cp backend/providers/Providerfile.example backend/providers/my-provider/Providerfile
```

The directory path becomes the provider slug:

```text
backend/providers/rate-am/Providerfile       -> rate-am
backend/providers/group/my-provider/Providerfile -> group-my-provider
```

Edit the copied file. It may contain a `buy` section, a `sell` section, or
both, but at least one section is required:

A file may contain a `buy` section, a `sell` section, or both:

```toml
[sell]
source_url = "https://rate.am"
name = "Rate Sell"
currency = ["usd", "rub", "eur", "amd"]
banks = []

[buy]
source_url = "https://rate.am"
name = "Rate Buy"
currency = ["usd", "rub", "eur", "amd"]
banks = []
```

## Fields

| Field | Required | Meaning |
| --- | --- | --- |
| `source_url` | Yes | Public provider URL using `http` or `https`. |
| `name` | Yes | Client-facing name for this operation. |
| `currency` | Yes | Non-empty array of 2–12 character currency or asset codes. |
| `banks` | No | Bank or payment-method names supported by this operation. |

The parser rejects unknown keys and invalid URLs. Currency codes are stored in
uppercase, duplicate values are removed, and empty bank names are ignored.

## Generate the migration

After adding or changing a Providerfile, generate the catalog migration:

```bash
cargo run --manifest-path backend/Cargo.toml --bin providerfile
```

This validates all files named `Providerfile` below `backend/providers` and
rewrites:

```text
backend/migrations/providers.sql
```

The generated migration contains idempotent UPSERT statements keyed by
`(slug, operation)`. Re-running it updates existing provider metadata without
creating duplicate rows. The command only writes SQL; it does not connect to
or modify a database.

Commit both the Providerfile and the regenerated `providers.sql` migration.

## Verify without changing files

Use this command in CI or before committing:

```bash
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- check
```

It fails if a Providerfile is invalid or if `providers.sql` is stale. To print
the generated SQL without writing it, run:

```bash
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- print
```

## Build and runtime behavior

The `providerfile` command uses one generic parser for every
`providers/**/Providerfile` and generates the idempotent
`backend/migrations/providers.sql` catalog migration. There is no Rust module
or registration function for an individual provider.

The generated SQL is embedded into the backend binary and applied after
`schema.sql` and `catalogs.sql` when the built application starts. The running
process never opens or parses a Providerfile.

The complete flow is:

1. Add or edit `backend/providers/<slug>/Providerfile`.
2. Run the generator.
3. Commit the Providerfile and `backend/migrations/providers.sql`.
4. Build the backend; the generated SQL is embedded in the binary.
5. Start the backend; it applies the embedded migration to the database.

Changing source files after the build does not affect a running application.
Regenerate the migration and rebuild it.

## Reading and removing providers

Provider rows are available through `GET /api/providers`, with optional
`operation`, `currency`, and `bank` query parameters. Each response item also
contains `searchable`: it is `true` only when the corresponding live P2P or
market source is enabled in the running backend. Catalog-only providers remain
selectable in the frontend. If a search contains only sources without a live
adapter, the backend returns an empty result instead of rejecting the request;
live quotes require a compatible adapter to be configured.

Removing a Providerfile does not delete historical database rows automatically;
disable or remove such rows with an explicit migration.
