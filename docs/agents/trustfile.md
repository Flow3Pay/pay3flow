# Trustfile agent guide

Read this guide when declaring or changing a provider's wallet-executable swap
path. The full format reference and field definitions are in
[`../Trustfile.md`](../Trustfile.md).

## What it controls

A Trustfile lives at
`backend/providers/<provider-slug>/Trustfile` beside its Providerfile. It
declares whether a provider has a supported quote, build, submit, and status
flow. Trustfiles are build inputs embedded in the backend binary; they do not
generate SQL or enable execution dynamically at runtime.

## Safety and implementation rules

- Never enable a provider merely because it has a quote API. The complete
  wallet-execution path must already be implemented and tested end to end.
- Inspect existing Trustfiles, the provider execution adapter, and its tests
  before changing the declaration. Match the directory-derived slug exactly.
- Trustfiles contain declarative data, never provider code or secrets. Keep
  credentials in server-side environment variables.
- Do not claim support for unsupported networks, wallet actions, approvals, or
  status transitions. An incomplete provider must remain disabled.
- For an enabled provider, verify that quote data is validated server-side,
  built into an allowed wallet action, submitted by the user's wallet, and
  reconciled into the supported execution statuses.

## Validation

The backend build validates Trustfiles and embeds them. Run:

```bash
cargo check --manifest-path backend/Cargo.toml
cargo test --manifest-path backend/Cargo.toml trustfile
```

For any change that enables execution, add or run an end-to-end test that
covers quote, wallet action, submission reference, and status mapping. There is
no generated Trustfile SQL migration to commit.
