# Кандидаты в fmatch (текущее состояние)

Источник: `GET /api/debug/acquirers` (backend `:8080`). Сейчас в fmatch 10 вымышленных солверов, все сидятся в таблицу `acquirers` при старте backend.

| slug | name | geo | лимит | комиссия по парам |
|---|---|---|---|---|
| flinger | Flinger Pay | US/EU/Global | 5–250k USD | USD→HKD 3.4%, USD→AUD 2.5%, USD→JPY 3.0%, EUR→USD 1.8% |
| warpgate | WarpGate FX | US/EU/UK/Global | 2–500k EUR | EUR→USD 1.4%, EUR→GBP 2.1%, GBP→USD 1.9%, USD→CHF 2.3% |
| corvus | Corvus Exchange | Global | 1–1M USD | USD→CNY 2.2%, EUR→CNY 2.8%, USD→INR 2.6%, EUR→USD 1.6% |
| vormir | Vormir FX | UK/EU/Global | 10–400k GBP | GBP→USD 1.7%, EUR→GBP 2.0%, GBP→INR 2.9%, USD→GBP 1.9% |
| helixpay | HelixPay | US/EU/Global | 5–300k USD | USD→BRL 3.3%, EUR→BRL 3.6%, USD→MXN 2.9%, EUR→USD 1.5% |
| quicksilver | Quicksilver Transfers | AU/SG/Global | 3–200k USD | USD→SGD 1.8%, EUR→SGD 2.4%, USD→AUD 2.1%, AUD→USD 2.0% |
| tessera | Tessera Pay | EU/Global | 5–400k EUR | USD→NOK 2.5%, EUR→NOK 2.7%, USD→SEK 2.4%, EUR→SEK 2.6% |
| bramba | Bramba Global | Global | 2–600k USD | USD→TRY 3.9%, EUR→TRY 4.1%, USD→AED 2.3%, EUR→AED 2.6% |
| okto | Okto FX | EU/Global | 1–150k EUR | USD→PLN 1.9%, EUR→PLN 1.5%, USD→CZK 2.2%, EUR→CZK 1.8% |
| vanda | Vanda Move | US/EU/UK/CA/Global | 2–800k USD | USD→GBP 1.6%, EUR→USD 1.3%, GBP→EUR 1.7%, USD→CAD 1.9% |

Детали по каждому солверу (API-слой, качество/задержка): `GET /api/debug/acquirers/<slug>` (например `/api/debug/acquirers/vanda`).