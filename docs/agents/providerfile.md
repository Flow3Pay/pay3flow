# Providerfile agent guide

Read this guide when adding, modifying, or removing a provider declaration.
The full format reference, field definitions, and examples are in
[`../Providerfile.md`](../Providerfile.md).

## What it controls

A Providerfile is a build-time declaration at
`backend/providers/<provider-slug>/Providerfile`. It describes provider
catalog metadata and may declare quote adapters, market data, browser
workflows, tests, and compiled Rust adapter code. It is not runtime
configuration. The generator produces `backend/migrations/providers.sql`;
`backend/build.rs` compiles supported embedded code.

## Workflow

1. Inspect existing Providerfiles and relevant adapter implementations/tests
   before choosing a format. Reuse the closest supported pattern; do not invent
   schema fields or advertise an adapter that is not implemented.
2. Keep credentials out of Providerfiles. Use the existing server-side
   environment/config mechanisms for secrets.
3. Make the smallest valid declaration. Unknown fields are rejected, and
   `(slug, operation)` pairs must be unique.
4. If the Providerfile has optional `[test]` assertions, keep them meaningful
   and safe to run. Network checks must target the intended public provider
   endpoint; do not add tests that place orders, authenticate with real
   customer credentials, or mutate production data.
5. Regenerate and verify the SQL migration:

   ```bash
   cargo run --manifest-path backend/Cargo.toml --bin providerfile -- generate
   cargo run --manifest-path backend/Cargo.toml --bin providerfile -- check
   ```

6. Run the focused providerfile validation and relevant backend checks:

   ```bash
   cargo run --manifest-path backend/Cargo.toml --bin providerfile -- test
   cargo test --manifest-path backend/Cargo.toml --bin providerfile
   cargo check --manifest-path backend/Cargo.toml
   ```

Include the generated migration with the Providerfile change. Review its diff
to ensure it contains only the intended provider catalog updates.
