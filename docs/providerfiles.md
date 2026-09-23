# Providerfile catalog

Provider definitions are configuration-as-migration. Put each provider in its
own directory under `backend/providers`; the directory path becomes its slug:

```text
backend/providers/rate-am/Providerfile  ->  rate-am
```

Start from the copyable example:

```bash
mkdir -p backend/providers/my-provider
cp backend/providers/Providerfile.example backend/providers/my-provider/Providerfile
```

A file may contain a `buy` section, a `sell` section, or both:

```toml
[sell]
source_url = "https://rate.am"
name = "Rate Sell"
currency = ["usd", "rub", "eur", "amd"]
banks = [""]

[buy]
source_url = "https://rate.am"
name = "Rate Buy"
currency = ["usd", "rub", "eur", "amd"]
banks = [""]
```

The parser is strict: unknown keys fail migration generation, names and
currency lists must not be empty, and `source_url` must use HTTP or HTTPS.
Currency codes are stored in uppercase, duplicates are removed, and blank bank
names are ignored.

The `providerfile` command uses one generic parser for every
`providers/**/Providerfile` and generates the idempotent
`backend/migrations/providers.sql` catalog migration. There is no Rust module
or registration function for an individual provider.

The generated SQL is embedded into the backend binary and applied after
`schema.sql` and `catalogs.sql` when the built application starts. The running
process never opens or parses a Providerfile.

To validate every Providerfile and regenerate the migration, run from
`backend/`:

```bash
cargo run --bin providerfile
```

CI can verify that the committed migration is current:

```bash
cargo run --bin providerfile -- check
```

Use `cargo run --bin providerfile -- print` to preview SQL without changing
the migration.

Provider rows are available through `GET /api/providers`, with optional
`operation`, `currency`, and `bank` query parameters. Removing a Providerfile
does not delete historical database rows automatically; disable or remove such
rows with an explicit migration.
