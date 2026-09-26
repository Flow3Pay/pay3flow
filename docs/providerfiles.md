# Providerfile

`Providerfile` is the only provider-specific configuration in Pay3Flow. A
provider does not need its own Rust file or registration function. The generic
engine supports catalog-only providers, public HTTP/JSON APIs, and browser
workflows for calculators without a suitable public API.

The application never reads Providerfiles at runtime. The generator validates
their declarative sections and writes JSON configuration into
`backend/migrations/providers.sql`. The build script compiles an optional Rust
`[code]` section directly into the backend crate. The SQL and generated Rust
modules are therefore already part of the binary when it starts.

## Add a provider

From the repository root:

```bash
mkdir -p backend/providers/my-provider
cp backend/providers/Providerfile.example backend/providers/my-provider/Providerfile
```

The directory path becomes the slug. For example,
`backend/providers/my-provider/Providerfile` becomes `my-provider`.

Every file needs `[buy]`, `[sell]`, or both:

```toml
[sell]
source_url = "https://provider.example"
name = "Example Sell"
currency = ["usd", "rub", "eur"]
banks = []

[buy]
source_url = "https://provider.example"
name = "Example Buy"
currency = ["usd", "rub", "eur"]
banks = []
```

`currency` contains fiat currencies accepted by the operation. Codes are
normalized to uppercase, duplicates are removed, and empty bank names are
ignored. A file with only these sections is visible in the catalog but is not
searched for live quotes.

### Fee metadata

Catalog-only or quote-based venues may describe their fee model with a shared
`[fees]` section:

```toml
[fees]
kind = "quote_dependent"
description = "Fees are included in the live quote and can vary by execution."
docs_url = "https://provider.example/docs/fees"
```

The metadata is returned by `GET /api/providers` as `fee_model`. It is
descriptive only; a live adapter must provide the actual fee for a specific
quote. Do not encode a fixed percentage unless the provider guarantees one.

## Compile-time Rust code

Use `[code]` only when a provider cannot be expressed by the generic HTTP or
browser adapters. The source is trusted application code, not a runtime script:

```toml
[code]
language = "rust"
source = '''
use async_trait::async_trait;

pub struct ExampleRouteProvider;

// Implement the Pay3Flow provider traits here.
'''
```

`backend/build.rs` scans every `providers/**/Providerfile` during compilation.
Each code block becomes a module below `crate::compiled_provider_code`; hyphens
in the provider slug become underscores, so `my-provider` is emitted as
`crate::compiled_provider_code::my_provider`. Changing a Providerfile causes
Cargo to rebuild the generated module. Invalid Rust fails the backend build,
just like invalid code in a normal library.

Only `language = "rust"` is accepted and `source` must not be empty. The code
runs with the backend's dependencies and permissions, so review it exactly as
code under `src` and never put credentials in it. Add any required third-party
crate to `backend/Cargo.toml`; Providerfiles cannot declare dependencies on
their own. CoW Swap, NEAR Intents, and ID Pay are checked-in examples.

The `[code]` body is not copied into the providers SQL and is never compiled or
evaluated at runtime.

## Browser workflow

Use `[workflow]` when the rate is calculated by an interactive page. Describe
actions using stable HTML attributes such as `name`, `aria-label`, `role`, or
`data-testid`. Do not copy the whole HTML page and avoid generated CSS class
names: they change between frontend builds.

```toml
[workflow]
source_url = "https://provider.example/"
get_exchange = "https://provider.example/exchange"
timeout_ms = 30000
navigation_retries = 1
navigation_retry_delay_ms = 1000
supported_assets = ["USDT", "BTC", "ETH"]
fiat_probe_amount = 1000
asset_probe_amount = 100
default_min_fiat = 1
default_max_fiat = 1000000000
default_available_asset = 1000000000
is_merchant = true
is_verified = true

[workflow.buy]
amount_mode = "fiat_probe"

[[workflow.buy.steps]]
action = "wait_for"
selector = 'input[name="amountFrom"]'

[[workflow.buy.steps]]
action = "react_select"
selector = 'div:has(> input[name="currencyFrom"]) > div'
value = "{{fiat}}"

[[workflow.buy.steps]]
action = "wait"
milliseconds = 800

[[workflow.buy.steps]]
action = "react_select"
selector = 'div:has(> input[name="currencyTo"]) > div'
value = "{{asset}}"

[[workflow.buy.steps]]
action = "fill"
selector = 'input[name="amountFrom"]'
value = "{{amount}}"

[[workflow.buy.steps]]
action = "wait"
milliseconds = 800

[workflow.buy.fiat_amount]
selector = 'input[name="amountFrom"]'
property = "value"

[workflow.buy.asset_amount]
selector = 'input[name="amountTo"]'
property = "value"
```

Define `[workflow.sell]` the same way, with crypto in `currencyFrom`, fiat in
`currencyTo`, and `amount_mode = "asset_probe"`. The generic workflow template
is available in `backend/providers/Providerfile.example`.

Available workflow actions are:

| Action | Fields | Purpose |
| --- | --- | --- |
| `fill` | `selector`, `value` | Replace an input value. |
| `click` | `selector` | Click a visible element. |
| `press` | `selector`, `key` | Send a key such as `Enter`. |
| `select_option` | `selector`, `value` | Select a native HTML `<select>` option. |
| `react_select` | `selector`, `value` | Open a React Select control and click its matching `role=option`. |
| `wait_for` | `selector` | Wait until an element is visible. |
| `wait` | `milliseconds` | Wait for a debounced calculator/network update. |
| `evaluate` | `script`, optional `value` | Run trusted build-time JavaScript for an exceptional UI. |

Templates in step values are `{{fiat}}`, `{{asset}}`, and `{{amount}}`.
Result properties are `value`, `text`, or `attribute:<name>`. Numbers using a
decimal comma and spaces as grouping separators are supported.

`navigation_retries` is useful for SPAs whose JavaScript chunks occasionally
fail during the first navigation. TLS verification remains enabled. In the
provided Docker image Chromium is installed and
`playwright_chromium_executable` is configured in `config.toml`. The image also
copies the Playwright Node driver from the build stage and exposes it through
`PLAYWRIGHT_DRIVER_PATH`; both the driver and browser are required. For local
runs, set `playwright_chromium_executable` to a compatible binary. Set
`p2p_workflow_debug_screenshot` to a file path to save a screenshot when a
workflow step fails.

## HTTP/JSON adapter

Use `[adapter.p2p]` for a read-only JSON endpoint. The checked-in Binance,
Bybit, OKX, Bitget, and Rapira Providerfiles are complete examples.

```toml
[adapter.p2p]
kind = "http_json"
endpoint = "https://provider.example/api/offers"
method = "POST"
timeout_ms = 5000
market = "direct_exchange" # omit for ordinary P2P advertisements
supported_assets = ["USDT"]
default_min_fiat = 1
default_max_fiat = 1000000000
default_available_asset = 1000000000

[adapter.p2p.buy]
amount_mode = "query_or_empty"
request_json = '''{"fiat":"{{fiat}}","asset":"{{asset}}","amount":"{{amount}}"}'''
items_pointer = "/data/items"

[adapter.p2p.offer]
ad_id_pointer = "/id"
fiat_pointer = "/fiat"
asset_pointer = "/asset"
price_pointer = "/price"
available_asset_pointer = "/available"
min_fiat_pointer = "/min"
max_fiat_pointer = "/max"
advertiser_nickname_pointer = "/merchant/name"
```

Request templates support `{{fiat}}`, `{{asset}}`, `{{amount}}`,
`{{amount_number}}`, `{{limit}}`, and `{{limit_number}}`. Response fields use
JSON Pointer syntax. A direction may override the common mapping with
`[adapter.p2p.buy.offer]` or `[adapter.p2p.sell.offer]`. Headers may reference a
runtime secret as `Authorization = "{{env:PROVIDER_API_TOKEN}}"`; never commit
the secret itself.

`[adapter.market]` uses the same HTTP fields and maps ticker arrays with
`items_pointer`, `symbol_pointer`, `bid_pointer`, and `ask_pointer`. A
Providerfile may contain a browser workflow plus a market adapter, but it may
not define both `[workflow]` and `[adapter.p2p]`.

For public calculators that return fiat rates and asset rates in separate,
index-aligned arrays, add `[adapter.p2p.rate_table]`. Point it at the fiat item
array (including its code and rate fields), the asset item array and the
parallel asset-rate array. `fiat_codes` maps local codes to provider codes such
as `RUB = "RUR"`; `source_url` can lead to the provider's account or trading
flow. The adapter emits a verified direct-exchange quote and uses the product
of the selected fiat and asset rates as fiat units per asset. See the complete
Cifra Markets example in `backend/providers/cifra-broker/Providerfile`.

## Generate and verify the migration

Generate SQL without applying it to a database:

```bash
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- generate
```

Verify all Providerfiles and ensure the committed migration is current:

```bash
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- check
```

To inspect generated SQL without writing a file:

```bash
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- print
```

Commit the Providerfile together with `backend/migrations/providers.sql`, then
rebuild the backend. Removing a Providerfile does not delete an existing row;
that requires an explicit removal migration.
