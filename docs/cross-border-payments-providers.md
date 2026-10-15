# Cross-border payment and acquiring provider research

This is an informational comparison of ten real services and their published
API surfaces. It has not been used as an integration test. Pricing, supported
corridors, limits, eligibility, and API versions change; verify every detail
with the provider before making an architectural or commercial decision.

The figures below are research notes based on public provider material checked
in September 2026, not guarantees or quotes.

## 1. Wise Platform

Wise provides international transfers, multi-currency balances, and platform
APIs. It is relevant for transparent B2B/B2C transfers and batch payouts.

- Pricing is quote-based, generally using a mid-market reference rate plus a
  currency- and amount-dependent fee. Public estimates for popular corridors
  are often around 0.3–0.7%, but the live quote is authoritative.
- Coverage includes dozens of currencies and many sending/receiving countries;
  limits depend on verification and corridor.
- REST/JSON API with OAuth 2.0, quote, recipient, transfer, balance, profile,
  rate, and webhook surfaces.
- Production and sandbox URLs, token lifetimes, and supported grants must be
  checked in the current API version.

Documentation: <https://docs.wise.com/api-reference>

Platform: <https://wise.com/platform/>

## 2. Airwallex

Airwallex combines payment acceptance, local payouts, multi-currency accounts,
and embedded-finance products.

- Card acceptance and international-transfer pricing vary by region and
  contract; published examples include percentage-plus-fixed card pricing and
  SWIFT fees.
- Coverage is broad but product availability differs by country, currency, and
  account type.
- REST API with OAuth/API access tokens and webhooks for payment intents,
  payment methods, refunds, splits, transfers, beneficiaries, accounts, and
  reports.

Documentation: <https://www.airwallex.com/docs>

Pricing: <https://www.airwallex.com/en-us/pricing>

## 3. Stripe Connect

Stripe Connect links platform accounts with connected businesses and supports
global payments and payouts. It is most relevant to marketplaces and SaaS
platforms with distributed recipients.

- Payout and instant-payout fees depend on country, account configuration, and
  product; public examples include percentage-plus-fixed pricing.
- Supported countries, currencies, and payout methods depend on the platform
  and connected-account jurisdictions.
- REST API with Stripe keys, Connect OAuth/onboarding, official SDKs, webhooks,
  account, account-link, transfer, and webhook-endpoint resources.
- Includes hosted onboarding and provider-side risk/KYC tooling, but platform
  responsibilities remain contract- and jurisdiction-specific.

Pricing: <https://stripe.com/connect/pricing>

Documentation: <https://docs.stripe.com/>

## 4. PayPal Payouts API

PayPal Payouts supports batched payments to PayPal accounts and, where
available, Venmo. It can fit small payouts and wallet-based recipients.

- Sender fees and cross-currency FX spreads vary by destination and are exposed
  in payout responses; recipients may have different fee treatment.
- Coverage is wallet- and country-dependent.
- REST API with OAuth 2.0, batch and item status endpoints, webhooks, and
  `PayPal-Request-Id` for idempotent retries.
- Typical item states include `SUCCESS`, `PENDING`, `PROCESSING`, `DENIED`, and
  `CANCELED`.

Documentation: <https://developer.paypal.com/api/payouts>

## 5. Revolut Business API

Revolut Business provides multi-currency accounts and APIs for transfers,
exchanges, counterparties, and webhooks.

- Pricing depends on plan, transfer allowance, corridor, and conversion limits.
- Availability of countries, counterparties, and sandbox corridors must be
  verified for the specific business account.
- REST API with OAuth/JWT bearer tokens and scopes such as `READ`, `WRITE`, and
  `PAY`; sensitive-card-data access has additional network requirements.

Documentation: <https://developer.revolut.com/docs/api/business>

Product overview: <https://www.revolut.com/en-US/business/business-api/>

## 6. Adyen

Adyen is a global payment platform for cards, wallets, BNPL, and local payment
methods, with payout and reporting capabilities.

- Interchange++, service fees, FX pricing, and contract terms are merchant- and
  region-specific.
- Supported methods, currencies, and countries depend on the account and
  product configuration.
- REST APIs cover payments, 3DS details, refunds, orders, recurring payments,
  reports, and balance payouts; test and live domains vary by API.

Documentation: <https://docs.adyen.com/>

API Explorer: <https://docs.adyen.com/api-explorer/>

## 7. Nium

Nium provides B2B cross-border payouts, FX, wallets, verification, and local
payment rails.

- Pricing is normally quote-based and depends on corridor, volume, product,
  and recipient type.
- Public material describes broad country and currency coverage, but actual
  availability is account- and corridor-specific.
- REST/OpenAPI surfaces use client identifiers and API keys; payouts,
  verification, wallets, FX rates, webhooks, and SDKs are available.

Documentation: <https://docs.nium.com/>

Payouts: <https://docs.nium.com/docs/payouts>

## 8. Payoneer

Payoneer provides multi-currency receiving accounts, platform payouts, cards,
and local withdrawals. It can be relevant for platforms and recipients in
markets with limited acquiring coverage.

- Fees for receiving, withdrawal, and FX depend on country, currency, and
  method; public estimates are not a substitute for an account quote.
- Coverage and verification requirements vary by program.
- Developer access includes OAuth-style account and payment-initiation APIs,
  webhooks, and a sandbox subject to enrollment.

Documentation: <https://developer.payoneer.com/psd2/apis>

## 9. Currencycloud (Visa)

Currencycloud provides B2B APIs for FX, transfers, balances, and virtual
accounts. It is a possible infrastructure provider for a regulated platform.

- Pricing is quote-based and depends on volume and corridor.
- Public material describes broad payout coverage and a multi-currency product;
  eligibility must be checked for the specific account.
- REST API v2 includes authentication, currencies, rates, conversions,
  transfers, balances, transactions, and events/webhooks.

Documentation: <https://developer.currencycloud.com/api-reference/>

Developer portal: <https://developer.currencycloud.com/>

## 10. Skrill Payouts (Paysafe)

Skrill Bank Payouts supports JSON-over-HTTP bank payouts, particularly for
European corridors and smaller transfers.

- Merchant-specific pricing and receiving-country fees are not reliably
  represented by a public fixed table.
- EUR, GBP, CHF, and other currencies depend on the contract and destination.
- The integration uses a preparation step followed by execution, merchant
  identifiers, signed requests, status notifications, and configured payout
  access.

Integration guide: <https://www.skrill.com/fileadmin/content/pdf/Integration_guide-Bank_Payouts.pdf>

API documentation: <https://docs.paysafe.com/docs/digital-wallets/api-mqi/introduction>

## Short comparison

| Provider | Strong fit | API/authentication | Pricing model |
| --- | --- | --- | --- |
| Wise Platform | Transparent transfers and batch payouts | OAuth 2.0 | Quote-based |
| Airwallex | Platform acceptance plus payouts | API token/OAuth | Contract and corridor dependent |
| Stripe Connect | Marketplaces and connected accounts | Stripe keys/OAuth | Product and country dependent |
| PayPal Payouts | Wallet-based small payouts | OAuth 2.0 | Per-item and FX dependent |
| Revolut Business | Automation inside Revolut Business | OAuth/JWT bearer | Plan and corridor dependent |
| Adyen | Global acquiring | API key/OAuth | Interchange++/contract |
| Nium | Broad payout corridors | Client ID/API key | Quote-based |
| Payoneer | Platform and multi-currency receiving | OAuth 2.0 | Country/method dependent |
| Currencycloud | B2B FX and transfers | OAuth 2.0/API credentials | Quote-based |
| Skrill Payouts | European bank payouts | Merchant ID/signatures | Merchant contract |

## Selection checklist

1. Confirm the exact source/target corridor, payout method, and legal entity.
2. Compare total delivered cost: fixed fee, FX spread, intermediary charges,
   settlement cost, and failure/return fees.
3. Verify speed, cutoff times, weekend behavior, and finality.
4. Review KYC/AML, sanctions, consumer protection, data residency, and
   chargeback responsibilities.
5. Test sandbox behavior, idempotency, retries, webhooks, reconciliation, and
   account limits.
6. Keep the integration behind a Pay3Flow adapter; do not make a provider the
   orderbook or source of truth for Pay3Flow lifecycle state.

This document is research only. It is not a recommendation, endorsement, or
claim that any provider is currently integrated into Pay3Flow.
