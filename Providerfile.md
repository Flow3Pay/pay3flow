# Providerfile reference

`Providerfile` is the complete provider declaration format used by Pay3Flow.
It can describe a catalog entry, a public P2P or direct-exchange JSON API, a
spot-market ticker API, a BestChange source, a browser calculator, fee
metadata, and trusted Rust code compiled into the backend.

Providerfiles are build inputs, not runtime configuration. The generator
validates their declarative sections and writes
`backend/migrations/providers.sql`; `backend/build.rs` compiles optional
`[code]` sections into the backend crate. The running service reads the
generated database rows and compiled modules, never the Providerfiles.

## Contents

- [Location, slug, and lifecycle](#location-slug-and-lifecycle)
- [Supported combinations](#supported-combinations)
- [Catalog-only provider](#catalog-only-provider)
- [Fee metadata](#fee-metadata)
- [BestChange adapter](#bestchange-adapter)
- [HTTP/JSON P2P adapter](#httpjson-p2p-adapter)
- [Direct-exchange API](#direct-exchange-api)
- [Rate-table API](#rate-table-api)
- [Authenticated API](#authenticated-api)
- [Spot-market adapter](#spot-market-adapter)
- [Browser workflow](#browser-workflow)
- [Compile-time Rust code](#compile-time-rust-code)
- [Complete field reference](#complete-field-reference)
- [Generate and validate](#generate-and-validate)
- [Checked-in examples](#checked-in-examples)

## Location, slug, and lifecycle

Place each file at `backend/providers/<provider-slug>/Providerfile`. Nested
directories are allowed; their path components are joined with `-`. For
example, `providers/acme/eu/Providerfile` gets the slug `acme-eu`.

A slug is lowercased and must contain 1–64 ASCII letters, digits, `-`, or `_`.
Two files cannot declare the same `(slug, operation)` pair.

Start from the template:

```bash
mkdir -p backend/providers/my-provider
cp backend/providers/Providerfile.example \
  backend/providers/my-provider/Providerfile
```

After a change, regenerate and commit the catalog migration:

```bash
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- generate
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- check
```

Adding, changing, or removing a Providerfile changes the generated migration.
The migration upserts current definitions and removes generated rows whose
Providerfiles no longer exist. Hand-written provider rows with another
`source_file` convention are not removed.

Unknown fields are rejected in every section. A misspelled key is an error,
not an ignored setting.

## Supported combinations

Every Providerfile must have `[buy]`, `[sell]`, or both. The remaining sections
are optional and can be combined as follows:

| Section | Purpose | Combination rules |
| --- | --- | --- |
| `[buy]`, `[sell]` | Catalog operations | At least one is required. |
| `[fees]` | Descriptive fee metadata | Can accompany any provider type. |
| `[adapter.p2p]` | P2P ads or direct quotes over JSON | Cannot coexist with `[workflow]`. |
| `[adapter.market]` | Spot bid/ask tickers over JSON | Can coexist with another adapter or workflow. |
| `[adapter.bestchange]` | BestChange direction pages | Can coexist with other adapter sections, though normally used alone. |
| `[workflow]` | Browser-driven calculator | Cannot coexist with `[adapter.p2p]`. |
| `[code]` | Trusted Rust compiled into the backend | Independent of declarative sections. |

If `[buy]` exists, `[adapter.p2p.buy]` or `[workflow.buy]` must exist for the
selected live mechanism. The same rule applies to `sell`. A file containing
only catalog sections is valid but is not a live quote source.

## Catalog-only provider

This is the smallest complete Providerfile:

```toml
[sell]
source_url = "https://provider.example/sell"
name = "Example Sell"
currency = ["usd", "rub", "eur"]
banks = ["Example Bank"]

[buy]
source_url = "https://provider.example/buy"
name = "Example Buy"
currency = ["usd", "rub", "eur"]
banks = []
```

`source_url` must use HTTP or HTTPS. `name` cannot be empty. `currency` must
contain at least one 2–12 character ASCII alphanumeric code. Currency codes are
uppercased, whitespace is trimmed, duplicates are removed, and the result is
sorted. Bank names are trimmed, deduplicated, and sorted; empty names are
discarded.

`buy` means that the customer buys crypto with fiat. `sell` means that the
customer sells crypto for fiat.

## Fee metadata

Add one shared fee description for both operations:

```toml
[fees]
kind = "quote_dependent"
description = "The live quote includes the provider fee; network fees may be separate."
docs_url = "https://provider.example/docs/fees"
```

All three fields are required. `kind` is a non-empty free-form identifier and
is normalized to lowercase. `description` cannot be empty, and `docs_url` must
use HTTP or HTTPS. This metadata is returned by `GET /api/providers`; it does
not calculate or modify a quote.

## BestChange adapter

Append this section to catalog operations to read BestChange direction pages:

```toml
[adapter.bestchange]
endpoint = "https://bestchange.biz"
language = "ru"
timeout_ms = 10000
max_results = 100
```

`endpoint` must use HTTP or HTTPS. `language` defaults to `"en"` and must be
1–8 characters. `timeout_ms` defaults to `10000` and must be 250–30000.
`max_results` defaults to `100` and must be positive.

Complete example: `backend/providers/bestchange/Providerfile`.

## HTTP/JSON P2P adapter

Use `[adapter.p2p]` for a public JSON endpoint returning P2P ads. This complete
shape uses a POST body and a shared offer mapping:

```toml
[sell]
source_url = "https://provider.example/p2p"
name = "Example Sell"
currency = ["usd", "eur"]
banks = []

[buy]
source_url = "https://provider.example/p2p"
name = "Example Buy"
currency = ["usd", "eur"]
banks = []

[adapter.p2p]
kind = "http_json"
endpoint = "https://api.provider.example/v1/ads/search"
method = "POST"
headers = { Origin = "https://provider.example", Referer = "https://provider.example/p2p" }
asset_codes = { USDT = "USDT_TRC" }
supported_assets = ["USDT", "BTC", "ETH"]
supported_fiats = ["USD", "EUR"]
timeout_ms = 5000
max_results = 50

[adapter.p2p.buy]
amount_mode = "query_or_empty"
request_json = '''{"side":"BUY","fiat":"{{fiat}}","asset":"{{asset}}","amount":"{{amount}}","limit":"{{limit_number}}"}'''
items_pointer = "/data/items"
success_pointer = "/code"
success_value = "0"
error_pointer = "/message"

[adapter.p2p.sell]
request_json = '''{"side":"SELL","fiat":"{{fiat}}","asset":"{{asset}}","amount":"{{amount}}","limit":"{{limit_number}}"}'''
items_pointer = "/data/items"
success_pointer = "/code"
success_value = "0"
error_pointer = "/message"

[adapter.p2p.offer]
ad_id_pointer = "/id"
fiat_pointer = "/fiat"
asset_pointer = "/asset"
price_pointer = "/price"
available_asset_pointer = "/available"
min_fiat_pointer = "/limits/min"
max_fiat_pointer = "/limits/max"
payment_methods_pointer = "/paymentMethods"
payment_method_value_pointer = "/name"
payment_method_fallback_pointer = "/code"
pay_time_limit_pointer = "/payMinutes"
advertiser_id_pointer = "/merchant/id"
advertiser_nickname_pointer = "/merchant/name"
advertiser_user_type_pointer = "/merchant/type"
completed_orders_pointer = "/merchant/orders30d"
completion_rate_pointer = "/merchant/completionRate"
positive_rate_pointer = "/merchant/positiveRate"
verified_from_merchant = true
source_url_template = "https://provider.example/ad/{{item:/id}}"
source_url_is_exact = true
advertiser_profile_url_template = "https://provider.example/user/{{item:/merchant/id}}"

[[adapter.p2p.offer.merchant_conditions]]
pointer = "/merchant/type"
operator = "equals_ci"
value = "merchant"

[[adapter.p2p.offer.verified_conditions]]
pointer = "/merchant/verified"
operator = "truthy"
```

For a GET endpoint, put parameters in `query` and omit `request_json`:

```toml
[adapter.p2p]
kind = "http_json"
endpoint = "https://api.provider.example/v1/ads"
method = "GET"

[adapter.p2p.buy]
query = { side = "sell", fiat = "{{fiat}}", asset = "{{asset}}", amount = "{{amount}}", limit = "{{limit}}" }
items_pointer = "/data"
```

### Request templates

The common endpoint, operation endpoint, query values, and JSON string values
can use these placeholders:

| Placeholder | Value |
| --- | --- |
| `{{fiat}}` | Requested canonical fiat code. |
| `{{asset}}` | Requested asset code after applying `asset_codes`. |
| `{{amount}}` | Selected amount as a string, or an empty string. |
| `{{amount_number}}` | A JSON number when it is the whole JSON string value; otherwise rendered as text. |
| `{{limit}}` | Result limit as a string. |
| `{{limit_number}}` | A JSON number when it is the whole JSON string value; otherwise rendered as text. |

`request_json` must be valid JSON before substitution. POST adapters require
it; GET adapters may also send it, although normal GET APIs should use query
parameters. `amount_mode` controls the amount:

| Mode | Behavior |
| --- | --- |
| `query_or_empty` | Use the requested amount, otherwise render empty. This is the default. |
| `fiat_probe` | Use the requested amount or `fiat_probe_amount`. |
| `asset_probe` | Always use `asset_probe_amount`. |

### Response and offer mapping

All response `*_pointer` fields use RFC 6901 JSON Pointer syntax and must begin
with `/`. Without `items_pointer`, the whole response is treated as one object
or an array. `null` yields no offers.

`success_pointer` and `success_value` must be set together. Scalar JSON values
are compared as strings, so JSON `true` matches `success_value = "true"`. When
`success_missing_allowed = true`, a missing success field is accepted. On a
failed condition, `error_pointer` supplies the error text when possible.

An offer mapping must provide either `price_pointer` or both
`fiat_amount_pointer` and `asset_amount_pointer`; the latter calculates
`fiat_amount / asset_amount`. `price_inverted = true` then returns `1 / price`.
Availability and min/max fiat each need either a pointer or the corresponding
adapter default.

The shared `[adapter.p2p.offer]` applies to both directions. Override it for one
direction with `[adapter.p2p.buy.offer]` or `[adapter.p2p.sell.offer]`:

```toml
[adapter.p2p.buy.offer]
fiat_amount_pointer = "/input/amount"
asset_amount_pointer = "/output/amount"

[adapter.p2p.sell.offer]
fiat_amount_pointer = "/output/amount"
asset_amount_pointer = "/input/amount"
```

Offer URL templates accept `{{fiat}}`, `{{asset}}`, `{{fiat_lower}}`,
`{{asset_lower}}`, `{{side}}`, and `{{item:/json/pointer}}`. `side` is `buy` or
`sell` from the customer's perspective.

Condition operators are:

| Operator | Required `value` | Match |
| --- | --- | --- |
| `truthy` | No | Boolean true, nonzero number, or string `true`/`1`. |
| `non_empty` | No | Non-empty string, array, or object; any non-null scalar. |
| `equals` | Yes | Exact scalar string comparison. |
| `equals_ci` | Yes | ASCII case-insensitive comparison. |
| `not_equals` | Yes | Exact inequality. |
| `not_equals_ci` | Yes | ASCII case-insensitive inequality. |

`merchant_default` and `verified_default` set unconditional flags.
`verified_from_merchant` marks every detected merchant as verified. Rates from
`completion_rate_pointer` and `positive_rate_pointer` accept `0..1` or a
percentage up to `100`; values above `1` are divided by 100.

## Direct-exchange API

Set `market = "direct_exchange"` when the response is a provider quote rather
than a public user advertisement. The resulting offer is labeled as a service.

An operation may override the common endpoint, including placeholders:

```toml
[adapter.p2p]
kind = "http_json"
market = "direct_exchange"
endpoint = "https://api.provider.example/rates"
method = "GET"
supported_assets = ["USDT", "BTC"]
default_min_fiat = 1
default_max_fiat = 1000000000
default_available_asset = 1000000000

[adapter.p2p.buy]
endpoint = "https://api.provider.example/rate/{{asset}}/{{fiat}}/sell"
success_pointer = "/status"
success_value = "true"

[adapter.p2p.sell]
endpoint = "https://api.provider.example/rate/{{asset}}/{{fiat}}/buy"
success_pointer = "/status"
success_value = "true"

[adapter.p2p.offer]
price_pointer = "/result"
merchant_default = true
verified_default = true
source_url_template = "https://provider.example/rates"
```

For quote APIs where one input amount produces one output amount, use probe
amounts and per-direction mappings:

```toml
[adapter.p2p]
kind = "http_json"
market = "direct_exchange"
endpoint = "https://api.provider.example/quote"
method = "POST"
fiat_probe_amount = 1000
asset_probe_amount = 1
default_min_fiat = 1
default_max_fiat = 1000000000
default_available_asset = 1000000000

[adapter.p2p.buy]
amount_mode = "fiat_probe"
request_json = '''{"from":"{{fiat}}","to":"{{asset}}","amount":"{{amount_number}}"}'''

[adapter.p2p.buy.offer]
fiat_amount_pointer = "/input/amount"
asset_amount_pointer = "/output/amount"
merchant_default = true
verified_default = true

[adapter.p2p.sell]
amount_mode = "asset_probe"
request_json = '''{"from":"{{asset}}","to":"{{fiat}}","amount":"{{amount_number}}"}'''

[adapter.p2p.sell.offer]
fiat_amount_pointer = "/output/amount"
asset_amount_pointer = "/input/amount"
merchant_default = true
verified_default = true
```

Complete examples: `backend/providers/skylabs/Providerfile` and
`backend/providers/whitebird/Providerfile`.

## Rate-table API

Use `[adapter.p2p.rate_table]` when one response contains a fiat table, an
asset table, and an index-aligned asset-rate array. The adapter selects the
requested entries and multiplies the fiat and asset rates. It emits one
verified direct-exchange offer.

```toml
[adapter.p2p]
kind = "http_json"
endpoint = "https://api.provider.example/calculator"
method = "GET"
supported_assets = ["USDT", "BTC"]
default_min_fiat = 1
default_max_fiat = 1000000000
default_available_asset = 1000000000

[adapter.p2p.buy]
query = { key = "public" }
success_pointer = "/success"
success_value = "true"
error_pointer = "/message"

[adapter.p2p.sell]
query = { key = "public" }
success_pointer = "/success"
success_value = "true"
error_pointer = "/message"

[adapter.p2p.rate_table]
fiat_items_pointer = "/data/fiats"
fiat_code_pointer = "/code"
fiat_rate_pointer = "/rate/value"
asset_items_pointer = "/data/assets"
asset_code_pointer = "/code"
asset_rates_pointer = "/data/assetRates"
fiat_codes = { RUB = "RUR" }
source_url = "https://provider.example/trade"
```

All six pointers are required. `fiat_codes` is optional. `source_url` is
optional and must use HTTP or HTTPS. An offer mapping is not needed for this
mode. Complete example: `backend/providers/cifra-broker/Providerfile`.

## Authenticated API

Static headers may reference an environment variable only when the whole value
uses `{{env:UPPERCASE_NAME}}`:

```toml
headers = { Authorization = "{{env:PROVIDER_API_TOKEN}}" }
```

For timestamp-plus-body HMAC-SHA512 authentication:

```toml
[adapter.p2p.auth]
kind = "hmac_sha512_timestamp_body"
public_key_env = "PROVIDER_API_PUBLIC"
private_key_env = "PROVIDER_API_PRIVATE"
public_key_header = "ApiPublic"
timestamp_header = "Timestamp"
signature_header = "Signature"
```

The three header names are optional and default to the values shown. The
environment variable names must contain only uppercase ASCII letters, digits,
and `_`. The signature is lowercase hex HMAC-SHA512 over the decimal Unix
timestamp followed by the serialized JSON request body. Never store secrets in
a Providerfile. Complete example: `backend/providers/exnode/Providerfile`.

## Spot-market adapter

`[adapter.market]` maps a public ticker response to bid/ask pairs:

```toml
[adapter.market]
kind = "http_json"
endpoint = "https://api.provider.example/v1/tickers"
method = "GET"
headers = { Origin = "https://provider.example" }
query = { category = "spot" }
timeout_ms = 5000
items_pointer = "/data/items"
symbol_pointer = "/symbol"
bid_pointer = "/bid"
ask_pointer = "/ask"
symbol_remove = "-"
success_pointer = "/code"
success_value = "0"
success_missing_allowed = false
error_pointer = "/message"
```

`items_pointer` is optional; without it the root object or array is used.
`symbol_pointer`, `bid_pointer`, and `ask_pointer` are required. Every
occurrence of `symbol_remove` is removed from the symbol. Headers, query,
`request_json`, success handling, methods, and timeouts follow the same rules
as the P2P adapter. POST requires `request_json`. Market requests are normally
static: `fiat`, `asset`, and `amount` render empty, while `limit` renders `100`.

Complete examples are the `[adapter.market]` sections in Binance, Bybit,
Bitget, OKX, Cifra Markets, and Dzengi Providerfiles.

## Browser workflow

Use `[workflow]` for a calculator that has no suitable JSON API. It requires
Playwright, its Node driver, and a compatible Chromium executable.

```toml
[workflow]
source_url = "https://provider.example/"
get_exchange = "https://provider.example/exchange"
timeout_ms = 30000
navigation_retries = 1
navigation_retry_delay_ms = 1000
asset_codes = { USDT = "Tether" }
supported_assets = ["USDT", "BTC"]
fiat_probe_amount = 1000
asset_probe_amount = 1
default_min_fiat = 1
default_max_fiat = 1000000000
default_available_asset = 1000000000
is_merchant = true
is_verified = true

[workflow.buy]
amount_mode = "fiat_probe"

[[workflow.buy.steps]]
action = "wait_for"
selector = 'input[name="fromAmount"]'

[[workflow.buy.steps]]
action = "react_select"
selector = 'div:has(> input[name="currencyFrom"]) > div'
value = "{{fiat}}"

[[workflow.buy.steps]]
action = "react_select"
selector = 'div:has(> input[name="currencyTo"]) > div'
value = "{{asset}}"

[[workflow.buy.steps]]
action = "fill"
selector = 'input[name="fromAmount"]'
value = "{{amount}}"

[[workflow.buy.steps]]
action = "wait"
milliseconds = 800

[workflow.buy.fiat_amount]
selector = 'input[name="fromAmount"]'
property = "value"

[workflow.buy.asset_amount]
selector = 'input[name="toAmount"]'
property = "value"

[workflow.sell]
amount_mode = "asset_probe"

[[workflow.sell.steps]]
action = "fill"
selector = 'input[name="fromAmount"]'
value = "{{amount}}"

[[workflow.sell.steps]]
action = "wait"
milliseconds = 800

[workflow.sell.fiat_amount]
selector = '.fiat-result'
property = "text"

[workflow.sell.asset_amount]
selector = 'input[name="fromAmount"]'
property = "attribute:data-amount"
```

Workflow value templates support only `{{fiat}}`, `{{asset}}`, and
`{{amount}}`. Prefer stable HTML attributes (`name`, `aria-label`, `role`, or
`data-testid`) over generated CSS class names.

Available actions:

| Action | Required fields | Behavior |
| --- | --- | --- |
| `fill` | `selector`, `value` | Replace an input value. |
| `click` | `selector` | Click a visible element. |
| `press` | `selector`, `key` | Send a key such as `Enter`. |
| `select_option` | `selector`, `value` | Select a native `<select>` option. |
| `react_select` | `selector`, `value` | Open a React Select control and choose the matching option. |
| `wait_for` | `selector` | Wait for an element to become visible. |
| `wait` | `milliseconds` | Wait 1–30000 ms for a UI/network update. |
| `evaluate` | `script`, optional `value` | Execute trusted JavaScript in the page. |

Every configured operation needs at least one step and both result readers.
Reader `property` defaults to `value` and accepts `value`, `text`, or
`attribute:<name>`. Localized positive numbers with spaces and decimal commas
are supported.

`timeout_ms` defaults to 10000 and must be 250–30000. `navigation_retries`
defaults to 0 and cannot exceed 3. `navigation_retry_delay_ms` defaults to 500
and must be 100–5000. The three default numeric limits are required and must
be positive; minimum cannot exceed maximum.

Use `p2p_workflow_debug_screenshot` in backend configuration to save a
screenshot when a workflow fails. Complete historical workflow patterns remain
in `backend/providers/Providerfile.example`.

## Compile-time Rust code

Use `[code]` only when a provider cannot be expressed declaratively. The code
is trusted application code with the backend's dependencies and permissions.

Inline source:

```toml
[code]
language = "rust"
source = '''
use async_trait::async_trait;

pub struct ExampleRouteProvider;

// Implement a Pay3Flow provider trait here.
'''
```

External source next to the Providerfile:

```toml
[code]
language = "rust"
source = path["adapter.rs"]
```

The standard TOML equivalent is also accepted:

```toml
source = { path = "adapter.rs" }
```

Only `language = "rust"` is accepted. Inline and external source must be
non-empty. An external path must be relative, point to a `.rs` file, stay
inside the Providerfile directory (no `..`), and may use subdirectories.
Changes to it trigger a Cargo rebuild.

Each code block becomes a module below `crate::compiled_provider_code`.
Hyphens in the slug become underscores, so `my-provider` becomes
`crate::compiled_provider_code::my_provider`. Add third-party dependencies to
`backend/Cargo.toml`; Providerfiles cannot declare dependencies. `[code]` is
not copied into SQL and is never evaluated as a runtime script.

Checked-in Rust examples: CoW Swap, NEAR Intents, and ID Pay.

## Complete field reference

### Catalog and metadata

| Path | Required | Default | Meaning |
| --- | --- | --- | --- |
| `buy.source_url`, `sell.source_url` | Per present operation | — | HTTP/HTTPS user-facing URL. |
| `buy.name`, `sell.name` | Yes | — | Non-empty display name. |
| `buy.currency`, `sell.currency` | Yes | — | Non-empty fiat-code array. |
| `buy.banks`, `sell.banks` | No | `[]` | Bank/payment filters. |
| `fees.kind` | In `[fees]` | — | Non-empty normalized identifier. |
| `fees.description` | In `[fees]` | — | Human-readable disclosure. |
| `fees.docs_url` | In `[fees]` | — | HTTP/HTTPS documentation URL. |

### `adapter.bestchange`

| Field | Required | Default | Meaning |
| --- | --- | --- | --- |
| `endpoint` | Yes | — | HTTP/HTTPS BestChange base URL. |
| `language` | No | `en` | 1–8 character direction-page language. |
| `timeout_ms` | No | `10000` | 250–30000 ms. |
| `max_results` | No | `100` | Positive result limit. |

### `adapter.p2p`

| Field | Required | Default | Meaning |
| --- | --- | --- | --- |
| `kind` | Yes | — | Must be `http_json`. |
| `market` | No | `p2p` | `p2p` or `direct_exchange`. |
| `endpoint` | Yes | — | HTTP/HTTPS endpoint; supports request placeholders. |
| `method` | No | `GET` | `GET` or `POST`. |
| `headers` | No | `{}` | Static headers or whole-value environment references. |
| `asset_codes` | No | `{}` | Canonical-to-provider asset codes. |
| `supported_assets` | No | `[]` | Empty means no adapter-level asset filter. |
| `supported_fiats` | No | `[]` | Empty means no adapter-level fiat filter. Use this when an endpoint is fixed to specific fiat currencies. |
| `timeout_ms` | No | `10000` | 250–30000 ms. |
| `max_results` | No | request limit | 1–100. |
| `fiat_probe_amount` | By mode | — | Positive finite fallback fiat amount. |
| `asset_probe_amount` | By mode | — | Positive finite asset probe amount. |
| `default_min_fiat` | If no pointer | — | Positive fallback minimum. |
| `default_max_fiat` | If no pointer | — | Positive fallback maximum. |
| `default_available_asset` | If no pointer | — | Positive fallback availability. |
| `auth` | No | — | HMAC authentication table. |
| `buy`, `sell` | By catalog operation | — | Direction request tables. |
| `offer` | Normally | — | Shared offer mapping. |
| `rate_table` | No | — | Alternative aligned-table mapping. |

### `adapter.p2p.buy` and `adapter.p2p.sell`

| Field | Required | Default | Meaning |
| --- | --- | --- | --- |
| `endpoint` | No | common endpoint | Direction-specific endpoint. |
| `query` | No | `{}` | Templated query parameters. |
| `request_json` | For POST | — | Valid JSON request template. |
| `amount_mode` | No | `query_or_empty` | Amount selection strategy. |
| `items_pointer` | No | response root | Object/array containing offers. |
| `success_pointer` | No | — | Scalar success field; paired with `success_value`. |
| `success_value` | No | — | Expected scalar string. |
| `success_missing_allowed` | No | `false` | Accept an absent success field. |
| `error_pointer` | No | — | Scalar provider error field. |
| `offer` | No | shared mapping | Direction-specific offer mapping. |

### Offer mapping

All fields below are optional individually, subject to the price and fallback
requirements described earlier.

| Field | Meaning |
| --- | --- |
| `ad_id_pointer` | Offer ID; otherwise a deterministic ID is generated. |
| `fiat_pointer`, `asset_pointer` | Currency codes; otherwise the query codes are used. |
| `price_pointer` | Fiat units per asset. |
| `fiat_amount_pointer`, `asset_amount_pointer` | Alternative price calculation; must appear together. |
| `price_inverted` | Invert the mapped/calculated price. |
| `available_asset_pointer` | Available asset amount. |
| `min_fiat_pointer`, `max_fiat_pointer` | Fiat order limits. |
| `payment_methods_pointer` | Scalar/object/array of payment methods. |
| `payment_method_value_pointer` | Preferred label inside a payment object. |
| `payment_method_fallback_pointer` | Fallback label inside a payment object. |
| `pay_time_limit_pointer` | Payment time in minutes. |
| `advertiser_id_pointer` | Advertiser ID. |
| `advertiser_nickname_pointer` | Nickname; provider display name is the fallback. |
| `advertiser_user_type_pointer` | User type; direct exchanges use `service`. |
| `merchant_conditions` | Any matching condition marks the advertiser as a merchant. |
| `verified_conditions` | Any matching condition marks the advertiser as verified. |
| `merchant_default` | Default merchant flag (`false`). |
| `verified_default` | Default verified flag (`false`). |
| `verified_from_merchant` | Treat detected merchants as verified (`false`). |
| `completed_orders_pointer` | Completed order count. |
| `completion_rate_pointer` | Completion ratio or percent. |
| `positive_rate_pointer` | Positive-feedback ratio or percent. |
| `source_url_template` | Offer/trading URL template. |
| `source_url_is_exact` | Whether the URL addresses this exact offer (`false`). |
| `advertiser_profile_url_template` | Advertiser profile URL template. |

### `adapter.p2p.rate_table`

| Field | Required | Default | Meaning |
| --- | --- | --- | --- |
| `fiat_items_pointer` | Yes | — | Array of fiat records. |
| `fiat_code_pointer` | Yes | — | Fiat code inside each record. |
| `fiat_rate_pointer` | Yes | — | Fiat rate inside each record. |
| `asset_items_pointer` | Yes | — | Array of asset records. |
| `asset_code_pointer` | Yes | — | Asset code inside each record. |
| `asset_rates_pointer` | Yes | — | Array of rates aligned with asset records. |
| `fiat_codes` | No | `{}` | Canonical-to-provider fiat code map. |
| `source_url` | No | catalog URL | HTTP/HTTPS trade or registration URL. |

### `adapter.p2p.auth`

| Field | Required | Default |
| --- | --- | --- |
| `kind` | Yes; `hmac_sha512_timestamp_body` | — |
| `public_key_env`, `private_key_env` | Yes | — |
| `public_key_header` | No | `ApiPublic` |
| `timestamp_header` | No | `Timestamp` |
| `signature_header` | No | `Signature` |

### `adapter.market`

| Field | Required | Default |
| --- | --- | --- |
| `kind` | Yes; `http_json` | — |
| `endpoint` | Yes; HTTP/HTTPS | — |
| `method` | No | `GET` |
| `headers`, `query` | No | `{}` |
| `request_json` | For POST | — |
| `timeout_ms` | No; 250–30000 | `10000` |
| `items_pointer` | No | response root |
| `symbol_pointer`, `bid_pointer`, `ask_pointer` | Yes | — |
| `symbol_remove` | No | — |
| `success_pointer`, `success_value` | No; set together | — |
| `success_missing_allowed` | No | `false` |
| `error_pointer` | No | — |

### `workflow`

| Field | Required | Default |
| --- | --- | --- |
| `source_url`, `get_exchange` | Yes; HTTP/HTTPS | — |
| `timeout_ms` | No; 250–30000 | `10000` |
| `navigation_retries` | No; 0–3 | `0` |
| `navigation_retry_delay_ms` | No; 100–5000 | `500` |
| `asset_codes` | No | `{}` |
| `supported_assets` | No | `[]` |
| `fiat_probe_amount`, `asset_probe_amount` | By amount mode | — |
| `default_min_fiat`, `default_max_fiat`, `default_available_asset` | Yes; positive | — |
| `is_merchant`, `is_verified` | No | `false` |
| `buy`, `sell` | By catalog operation | — |

Each workflow operation has `amount_mode` (default `query_or_empty`), a
non-empty `steps` array, and required `fiat_amount`/`asset_amount` readers.

### `code`

| Field | Required | Meaning |
| --- | --- | --- |
| `language` | Yes | Must be `rust`. |
| `source` | Yes | Non-empty inline string, `path["file.rs"]`, or `{ path = "file.rs" }`. |

## Generate and validate

Generate SQL without applying it:

```bash
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- generate
```

Verify all Providerfiles and ensure the committed migration is current:

```bash
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- check
```

Print generated SQL:

```bash
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- print
```

Then run the backend checks:

```bash
cargo fmt --manifest-path backend/Cargo.toml -- --check
cargo test --manifest-path backend/Cargo.toml --bin providerfile
cargo check --manifest-path backend/Cargo.toml
```

## Checked-in examples

| Capability | Providerfile |
| --- | --- |
| Catalog only | `backend/providers/Providerfile.example` after removing `[workflow]` |
| BestChange | `backend/providers/bestchange/Providerfile` |
| POST P2P ads and rich offer mapping | Binance, Bybit, Bitget |
| GET P2P ads | OKX, Rapira |
| Direct quote with per-direction amount mapping | Whitebird |
| Direct quote with endpoint templates | SkyLabs |
| Aligned rate table | Cifra Markets |
| HMAC-authenticated quote | Exnode |
| Spot tickers | Binance, Bybit, Bitget, OKX, Cifra Markets, Dzengi |
| Browser workflow template | `backend/providers/Providerfile.example` |
| Inline Rust | CoW Swap, NEAR Intents, ID Pay |
| External Rust | Use `source = path["adapter.rs"]`; parsing and build coverage live in `backend/providerfile.rs` |
