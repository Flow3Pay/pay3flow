# Public quote-source policy

Pay3Flow reads P2P advertisements, direct-exchange quotes, exchanger listings,
and spot tickers. Providerfile adapters are quote sources only; they do not
create trades or settle payments.

Most adapters use anonymous public data. Exnode is optional and signs quote
requests with credentials supplied through environment variables. No adapter
places orders, reserves crypto, or collects full payment-card credentials.

## Sources and candidates

| Source | What it provides | Access | Decision |
| --- | --- | --- | --- |
| Binance, Bybit, OKX, Bitget | Public P2P advertisement endpoints | Anonymous read-only requests; formats may change | Enabled through generic Providerfile HTTP mappings |
| Rapira P2P | Public read-only offers through `/otc/offers/page-query/v2` | No auth in the current adapter; format may change | Enabled as read-only; never creates trades |
| Whitebird | Quote from the public `/exchanger` calculator | Anonymous JSON request; no order submission | Enabled through the generic Providerfile HTTP adapter |
| Cifra Markets | Public calculator and IMEX ticker data | Anonymous JSON requests | Enabled for direct quotes and spot paths |
| SkyLabs | Public homepage rate endpoint | Anonymous JSON requests | Enabled for direct AMD/USD quotes |
| BestChange | Public catalog and direction pages | Anonymous HTML reads; page format may change | Enabled as an aggregated exchanger source |
| Dzengi | Public market ticker | Anonymous JSON requests | Enabled for crypto spot paths |
| Exnode | Merchant quote API | Registered public/private keys and HMAC signing | Enabled only when legitimate credentials are configured |
| ID Pay | Public server-rendered AMD/RUB rate | Anonymous page read | Enabled as a direct fiat route provider |

Do not describe these sources as settlement executors. Providerfile adapters
only read public offers or quotes. Other research sources remain out of the
live path until a legitimate read-only interface and adapter review exist.

The search UI accepts filtering parameters only. Users must never enter full
card numbers, CVV, passwords, or one-time confirmation codes into Pay3Flow.
