# Providerfile

`Providerfile` is the only provider-specific configuration in Pay3Flow. A
provider does not need its own Rust file or registration function. The generic
engine supports catalog-only providers, public HTTP/JSON APIs, and browser
workflows for calculators without a suitable public API.

The application never reads Providerfiles at runtime. The generator validates
them and writes JSON configuration into `backend/migrations/providers.sql`.
That SQL is embedded during the backend build and applied to PostgreSQL when
the built application starts.

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
`currencyTo`, and `amount_mode = "asset_probe"`. See the complete, live-tested
example in `backend/providers/whitebird/Providerfile`.

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
`PLAYWRIGHT_CHROMIUM_EXECUTABLE` is configured automatically. The image also
copies the Playwright Node driver from the build stage and exposes it through
`PLAYWRIGHT_DRIVER_PATH`; both the driver and browser are required. For local
runs, point the Chromium variable to a compatible binary and, when the driver
is stored outside Cargo's build directory, set `PLAYWRIGHT_DRIVER_PATH` to its
directory. Set
`P2P_WORKFLOW_DEBUG_SCREENSHOT` to a file path to save a screenshot when a
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

## BestChange public offer adapter

BestChange is different from a calculator: its public direction page contains
one row per monitored exchanger. Pay3Flow requests that page with the selected
`fromAmount`, reads each exchanger link, name, and displayed pair of amounts,
and converts every row into a separate offer. The route search can therefore
rank and combine the actual exchangers found for the requested amount.

```toml
[adapter.bestchange]
endpoint = "https://bestchange.biz"
language = "ru"
timeout_ms = 10000
max_results = 100
```

No API key is required. The adapter first reads the public exchange-unit
catalog to resolve payment methods and networks such as `visa-mastercard-rub`
and `tether-trc20`, then requests URLs such as
`/ru/visa-mastercard-rub-to-tether-trc20?fromAmount=10000`.

This intentionally depends on BestChange's public HTML and may need adjustment
if their markup changes or their site rate-limits automated requests.

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
