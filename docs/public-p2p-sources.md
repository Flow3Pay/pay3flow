# Public P2P source policy

Pay3Flow currently uses Binance, Bybit, OKX, Bitget, Rapira, and BestChange as
read-only advertisement sources. Whitebird is read through its public
calculator with an anonymous Playwright workflow. These sources do not create
trades.

The search does not authenticate to third-party services, place orders, reserve
crypto, or collect full payment-card credentials.

## Sources and candidates

| Source | What it provides | Access | Decision |
| --- | --- | --- | --- |
| Binance, Bybit, OKX, Bitget | Public P2P advertisement endpoints | Anonymous read-only requests; formats may change | Enabled through generic Providerfile HTTP mappings |
| Rapira P2P | Public read-only offers through `/otc/offers/page-query/v2` | No auth in the current adapter; format may change | Enabled as read-only; never creates trades |
| Whitebird | Quote from the public `/exchanger` calculator | Anonymous Chromium workflow; no order submission | Enabled through the generic Providerfile workflow engine |
| CoinGecko | Aggregated prices and market data | Plans and limits change; not P2P ads | Reference price only |
| BestChange | Public direction pages with one row per exchanger | Anonymous HTML requests; markup and rate limits may change | Enabled read-only without an API key |
| Exnode | Exchange or merchant APIs | Requires keys and request signing | Do not use without registered access |

Do not describe these sources as settlement executors. Providerfile adapters
only read public offers or quotes. Other research sources remain out of the
live path until a legitimate read-only interface and adapter review exist.

The search UI accepts filtering parameters only. Users must never enter full
card numbers, CVV, passwords, or one-time confirmation codes into Pay3Flow.
