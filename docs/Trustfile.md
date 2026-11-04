# Trustfile reference

A `Trustfile` describes **how** Pay3Flow executes the swap that a
`Providerfile` advertises. The Providerfile answers "which provider is this and
what does it quote?"; the Trustfile answers "can this provider execute inside a
wallet, and through which quote, build, submit, and status steps?"

Trustfiles are build inputs, not runtime configuration. `backend/build.rs`
parses every `backend/providers/**/Trustfile`, rejects unknown or malformed
fields, validates the declared execution path, and embeds the original TOML in
the backend binary. The runtime registry in `backend/src/trustfile.rs` then
answers `execution_enabled(provider)` from that embedded data. The running
service never reads a Trustfile from disk and never lets a client decide
whether a provider is executable.

## Location and lifecycle

Place each file at `backend/providers/<provider-slug>/Trustfile`, next to its
Providerfile. Nested directories are joined with `-`, exactly like
Providerfile slugs: `providers/acme/eu/Trustfile` declares
`provider = "acme-eu"`.

The `provider` value must match the directory slug. A Trustfile with an
unknown field, a wrong slug, an invalid URL, or an enabled state without a
complete execution path fails the backend build. There is no generated SQL
migration: the validated file itself is compiled into the binary.

Start from this template:

```toml
schema = 1
provider = "my-provider"

[trust]
state = "enabled"
transport = "http-json"

[quote]
method = "POST"
url = "https://api.example.com/quote"
request = "json"
response = "quote"

[build]
kind = "provider-transaction"
fields = ["to", "data", "value"]
approval_field = "approveTo"
expiry_field = "validUntil"

[submit]
kind = "wallet-transaction"
reference = "transaction_hash"

[status]
method = "GET"
url = "https://api.example.com/tx/{reference}"
response = "status"

[networks]
source = ["ethereum", "arbitrum-one"]
destination = ["ethereum", "arbitrum-one", "near"]
```

## Execution flow

A wallet-executable swap needs all four steps:

1. **Quote** — the backend calls the provider HTTP API again for the exact
   amount and addresses. The frontend never supplies a provider transaction
   directly.
2. **Build** — the backend validates the provider response and converts it
   into one of the wallet actions it understands.
3. **Submit** — the user's wallet signs and broadcasts the action (or sends a
   deposit), and the frontend reports the reference back to Pay3Flow.
4. **Status** — the backend polls the provider and normalises the result into
   `submitted`, `completed`, `failed`, `refunded`, `cancelled`, `expired`, or
   `stuck`.

The `Trustfile` declares this path. The checked-in Rust providers implement the
concrete HTTP and wallet details; a Trustfile never contains code.

`execution_enabled(provider)` returns true only when:

- `trust.state = "enabled"`;
- `build.kind` is supported (not `unsupported`); and
- `submit.kind` is supported (not `unsupported`).

Runtime configuration is checked separately. For example, CoW still needs its
API chain/configuration, Symbiosis still needs its execution contracts, and a
provider-specific capability check still has to accept the asset pair. The
Trustfile can only narrow what is allowed; it cannot enable an unconfigured
provider.

## Field reference

### Top level

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `schema` | integer | yes | Must be `1`. |
| `provider` | string | yes | Provider slug, matching the parent directory. |
| `trust` | table | yes | Execution trust decision. |
| `quote` | table | yes | Quote endpoint metadata. |
| `build` | table | yes | How a quote becomes a wallet action. |
| `submit` | table | yes | What the wallet returns after signing. |
| `status` | table | yes | Status endpoint metadata. |
| `networks` | table | yes | Source and destination networks the path supports. |

### `[trust]`

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `state` | `"enabled"` or `"disabled"` | yes | Only enabled files may execute. |
| `transport` | string | yes | Must currently be `"http-json"`. |
| `note` | string | no | Human-readable reason or caveat. |

A disabled Trustfile documents a provider that quotes but must not receive a
swap widget. `0x` is checked in as disabled until a generic EVM provider
transaction action exists; it stays quote-only and the guide links to Matcha.

### `[quote]` and `[status]`

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `method` | `"GET"` or `"POST"` | yes | HTTP method. |
| `url` | string | yes | `http://` or `https://` endpoint. |
| `request` | `"json"` or `"query"` | no | Request encoding; omit when unused. |
| `response` | string | no | Expected response body or field name. |

Status URLs may contain `{reference}`, `{chain_id}`, `{api_chain}`, or
`{deposit_address}` placeholders. They are descriptive; the Rust provider
performs the actual substitution.

### `[build]`

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `kind` | enum | yes | `provider-transaction`, `deposit-address`, `http-order`, or `unsupported`. |
| `fields` | string array | no | Response fields the backend must read. |
| `approval_field` | string | conditional | Required for `provider-transaction`; names the spender or token to approve. |
| `expiry_field` | string | no | Response field that carries the quote expiry. |

`provider-transaction` files must list at least `to`, `data`, and `value` in
`fields`. `deposit-address` files must name the deposit address field.

### `[submit]`

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `kind` | enum | yes | `wallet-transaction`, `wallet-deposit`, `wallet-order`, or `unsupported`. |
| `reference` | enum | yes | `transaction_hash` or `order_uid`. |

The backend uses `reference` to validate the submission kind instead of
hard-coding a provider list. An executable Trustfile may not use
`submit.kind = "unsupported"`.

### `[networks]`

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `source` | string array | yes | Source network ids, slug format. |
| `destination` | string array | yes | Destination network ids, slug format. |

Both arrays must be non-empty. Network ids use the same lowercase slug format
as provider ids.

## Checked-in Trustfiles

| Provider | State | Path | Notes |
| --- | --- | --- | --- |
| `symbiosis` | enabled | `providers/symbiosis/Trustfile` | Provider transaction plus optional approval. |
| `near-intents` | enabled | `providers/near-intents/Trustfile` | Deposit address; backend notifies 1Click after the wallet sends. |
| `cow-swap` | enabled | `providers/cow-swap/Trustfile` | EIP-712 wallet order posted to the CoW order book. |
| `0x` | disabled | `providers/0x/Trustfile` | Quote-only until a generic EVM provider transaction action exists. |

Do not create a Trustfile for a provider whose wallet path is not implemented
and tested end to end. A disabled file is honest documentation; an enabled
file advertises a swap widget.

## Validate and test

The backend build validates Trustfiles automatically:

```bash
cargo check --manifest-path backend/Cargo.toml --all-targets
```

The registry tests also assert that every embedded file parses, matches its
slug, validates, and produces the expected enabled/disabled answer:

```bash
cargo test --manifest-path backend/Cargo.toml trustfile
```

Before enabling a provider, add an end-to-end test that covers the quote,
wallet action, submission reference, and status mapping. Never place API keys,
private keys, or other secrets in a Trustfile; provider credentials belong in
server environment variables.
