# Georgian banks, cash and exchange discovery

Checked on 2026-11-11. Bank presence in the directory is not proof of a bank
account's eligibility, an available exchange offer, or payment initiation.
The bank catalog covers the 18 entries in the [National Bank of Georgia's
commercial-bank register](https://nbg.gov.ge/en/licensed-commercial-banks).
Microbanks and payment wallets are separate and are not included.

## Account and cash catalog

The backend-owned payment-method Providerfile supplies the picker through
`GET /api/banks?picker_visible=true`. Each bank uses a stable `ge-` identity
and a Georgia-specific currency group. Account variants share their bank's
group and payment query. Unknown transfer fees are left unspecified.

| Bank | Account choices | Primary account/currency evidence |
| --- | --- | --- |
| TBC Bank | GEL, USD, EUR | [NBG package comparison](https://nbg.gov.ge/en/package-comparison) |
| Bank of Georgia | GEL, USD, EUR | [Bank account manual](https://ramad.bog.ge/s3/cms/static/files/5d033a73-a17f-49da-9318-0762d76a1900.pdf) |
| Liberty Bank | GEL, USD, EUR | [Universal account](https://www.libertybank.ge/en/produqtebi/chemtvis/angarishebi-da-baratebi/universaluri-angarishi) |
| Basisbank | GEL, USD, EUR | [Service packages](https://bb.ge/en/banking/service-package) |
| Credo Bank | GEL, USD, EUR | [Current accounts](https://credobank.ge/en/products/current-account) |
| Terabank | GEL, USD, EUR | [Current accounts](https://terabank.ge/en/retail/services/1-mimdinare-angarishis-gakhsna-tsarmoeba), [fees](https://terabank.ge/en/retail/servisefees) |
| Silk Bank | GEL, USD, EUR | [Banking terms](https://silkbank.ge/uploads/2026/03/TC-Individuals-Eng-20.03.2026_Clear.pdf) |
| ProCredit Bank Georgia | GEL, USD, EUR | [Direct account](https://procreditbank.ge/en/node/3607) |
| Cartu Bank | GEL, USD, EUR | [Account conversion rules](https://www.cartubank.ge/uploads/eng_instruqcia.pdf) |
| Halyk Bank Georgia | GEL, USD, EUR | [Service packages](https://halykbank.ge/en/individuals/cards/5/10), [tariffs](https://halykbank.ge/en/individuals/rates) |
| PASHA Bank Georgia | GEL, USD, EUR | [Banking agreement](https://www.pashabank.ge/m/u/ck/files/General%20Agreement%20for%20Rendering%20Banking%20Services%20to%20Individuals_The%20contract%20is%20valid%20from%2006_04_2020.pdf) |
| Isbank Georgia | GEL, USD, EUR | [Account terms](https://isbank.ge/files/legal_and_security/Standard%20Terms%20of%20Banking%20Services%20-%20Valid%20Since%2001.09.2020.pdf), [NBG bank account FX comparison](https://nbg.gov.ge/en/currency-rates) |
| Ziraat Bank Georgia | GEL, USD, EUR | [Accounts](https://www.ziraatbank.ge/en/accounts) |
| Paysera Bank Georgia | GEL, USD, EUR | [Card account currency rules](https://www.paysera.ge/v2/ka-GE/samartleblivi/valutebi-da-konvertaciis-skemebi?output=pdf) |
| Hash Bank | GEL, USD, EUR | [Banking service tariff](https://hashbank.ge/api/media/file/%E1%83%93%E1%83%90%E1%83%9C%E1%83%90%E1%83%A0%E1%83%97%E1%83%98%201%20-%20%E1%83%9E%E1%83%90%E1%83%99%E1%83%94%E1%83%A2%E1%83%A3%E1%83%A0%E1%83%98%20%E1%83%9B%E1%83%9D%E1%83%9B%E1%83%A1%E1%83%90%E1%83%AE%E1%83%A3%E1%83%A0%E1%83%94%E1%83%91%E1%83%98%E1%83%A1%20%E1%83%AC%E1%83%94%E1%83%A1%E1%83%94%E1%83%91%E1%83%98%20%E1%83%93%E1%83%90%20%E1%83%9E%E1%83%98%E1%83%A0%E1%83%9D%E1%83%91%E1%83%94%E1%83%91%E1%83%98%20-%2015.10.2025.pdf) |
| Pave Bank Georgia | GEL, USD, EUR | [API currency documentation](https://docs.pavebank.com/docs/currencies) |
| VTB Bank Georgia | GEL directory entry only | [NBG register](https://nbg.gov.ge/en/licensed-commercial-banks); current foreign-currency service availability unverified |
| Neo Georgian Bank | GEL directory entry only | [NBG register](https://nbg.gov.ge/en/licensed-commercial-banks); public account-currency terms unverified |

VTB and Neo's GEL entries identify banks in the local-currency directory;
they do not assert that an account can currently be opened or that any quote
provider accepts transfers through them. USD/EUR variants are deliberately
omitted until current service availability is verified. Individual account
eligibility can depend on the bank's terms, residency and account product.

GEL and EUR cash are explicit choices. Existing USD cash remains available.
Bank logos are original registry assets with provenance in the frontend's
`static/icons/banks/SOURCES.md`; unavailable logos use initials.

## P2P discovery

Existing Binance, Bybit, OKX, Bitget and MEXC adapters already accept requested
fiat currencies. Georgian aliases normalize named bank labels; opaque numeric
payment IDs remain unverified. TBC uses `TBC Bank Georgia` as its canonical
query. Halyk, VTB, PASHA, Isbank, Ziraat and ProCredit also retain a regional
identity. Region-qualified labels require an exact canonical match, preventing
another country's similarly named bank from satisfying a Georgian selection.

[Binance documents Georgian payment methods](https://www.binance.com/en/support/announcement/detail/d8519d99edb4406fbab21e2ab483a362).
This historical announcement establishes method identities, not present
liquidity. No numeric payment IDs or bank transfer acceptance are invented.
Each live response and the requested amount determine returned coverage.

## Solunex

The compiled `solunex` adapter reads the same public calculator interfaces as
[Solunex's website](https://solunex.ge/en):

- `GET /api/offices`: active offices with `comingSoon=false` only.
- `GET /api/currencies?officeId=…&detailed=1`: per-office directional pairs
  and currency/network metadata, intersected with Pay3Flow's network catalog.
- `POST /api/rates/calculate`: office, give/receive currency codes,
  `mode=give` and the exact `giveAmount` as a decimal string.

Calculator requests carry the public site's Origin and Referer headers, as
required by that endpoint; anonymous POSTs without them return HTTP 403.

At verification Batumi was available and Tbilisi was marked coming soon.
Catalogs refresh after five minutes. Quotes are requested for actual amounts;
`receiveAmount` is the commission-inclusive net output. The adapter validates
the returned input and expiry and never scales a quote from a probe amount.
Crypto composition requotes each intermediate amount. The provider's quote
token stays private; the public quote identifier identifies the office.

Fiat endpoints require an explicit `Cash` payment-method selection. The public
calculator does not establish Georgian bank-transfer support. Directions can
include cash/crypto, cash/cash and crypto/crypto; only returned, recognized
network pairs are enabled. Route instructions identify the office and address,
disclose cash settlement and require provider confirmation. External handoff
opens Solunex, where registration, verification and order placement occur.

Solunex is not a P2P advertisement source and has no native wallet execution
descriptor. An exchange estimate does not book an office visit or reserve a
rate. Bank/blockchain charges not established by the calculator remain outside
the quoted commission.

## NBG reference rates

`GET /api/reference-rates` accepts GEL and EUR in addition to AMD, RUB, USD,
BYN, UAH and KZT. The NBG JSON feed is fetched server-side with the current
Georgia date (UTC+4) because the undated feed can contain tomorrow's rates.
Each observation is `rate / quantity` GEL per foreign currency unit. Future
effective dates, invalid quantities and observations older than seven days
are rejected. Refresh also occurs when the Georgia calendar date changes.

GEL pairs and EUR/USD use NBG directly. Other pairs retain existing USD cross
rates, with EUR's USD leg sourced from NBG. Cash and account searches share
this informational official benchmark, not a bank's cash buy/sell rate.
Attribution, stale cache behavior and unavailable-source reporting remain
visible. Benchmarks never substitute for an exchanger quote.

## Other exchangers considered

Cryptal's public market data currently calls its GEL/USD/EUR market balances
TOGEL/TOUSD/TOEUR (TOL tokens). It is deferred until redemption rails and fees
can be modeled separately from actual bank currency. MyCoins public access
returned HTTP 403 and Bitnet quote access was not established. Coinmania also
advertises tokenized fiat. None is enabled merely from a homepage rate.

## Verification

Provider tests use a local HTTP calculator fixture and the real route pipeline;
they cover cash eligibility, office availability, net commissions, exact input
amounts, network mapping, expiry and intermediate requoting. A separate ignored
live calculator test performs only public quote reads. Catalog tests parse the
actual Providerfile and check bank groups, currency variants and logo assets.
NBG tests cover unit quantities, effective dates, inversion, attribution and
missing rates. Browser tests use the actual payment-method catalog with mocked
route/reference APIs to cover selection, sharing and provider instructions.

The production release is rebased onto current `master`; its checks and deployed revision are recorded separately in the release receipt.
