# Public P2P source policy

Pay3Flow currently uses Binance, Bybit, OKX, Bitget, and Rapira as read-only
advertisement sources. These adapters do not create trades.

The search does not authenticate to third-party services, place orders, reserve
crypto, or collect full payment-card credentials.

## Sources and candidates

| Source | What it provides | Access | Decision |
| --- | --- | --- | --- |
| Rapira P2P | Public read-only offers through `/otc/offers/page-query/v2` | No auth in the current adapter; format may change | Enabled as read-only; never creates trades |
| CoinGecko | Aggregated prices and market data | Plans and limits change; not P2P ads | Reference price only |
| BestChange | Exchange rates and reserves | Official API requires an API key | Use only with a legitimate key |
| Exnode | Exchange or merchant APIs | Requires keys and request signing | Do not use without registered access |

Do not describe these sources as verified solvers. Rapira is only a public
advertisement source, not a settlement executor. Other research sources remain
out of the live path until a legitimate read-only interface and adapter review
exist.

The search UI accepts filtering parameters only. Users must never enter full
card numbers, CVV, passwords, or one-time confirmation codes into Pay3Flow.
