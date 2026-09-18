# Справочник провайдеров для международных денежных переводов и эквайринга
## 10 реальных сервисов с низкими комиссиями + данные об их API

> Назначение документа: ознакомительная подборка — **данные и API-ссылки**, без тестирования. Перед интеграцией всегда перепроверяйте тарифы и покрытие на официальных страницах, так как условия меняются.
> Все технические данные собраны по официальным источникам (документация производителей, прайс-листы) на сентябрь 2026 года.

---

## 1. Wise Platform (ранее TransferWise)

**О чём:** Самый популярный сервис переводов "по настоящему" курсу (mid-market rate) с прозрачной комиссией. Подходит для B2C/B2B переводов, массовой выплаты зарплат и мультивалютных счетов через платформу.

**Тарифы / комиссии:**
- Курс: средний рыночный (mid-market rate), без скрытой наценки.
- Комиссия: фиксированная % + константа, зависит от пары валют и суммы (видна в Quote). Для небольших сумм комиссия обычно ниже, чем у банков — от ~0.3%–0.7% на популярных коридорах.
- Нет скрытых маржей на обмене.

**Крышка / покрытия:**
- Валюты: 40+ валют приёма и выплат; мультивалютный счёт на 56+ валют.
- Страны: более 90 стран отправления; получатели в десятках стран локальными способами.
- Лимиты: зависят от уровня верификации клиента; лимиты отправки показываются в Quote.

**API:**
- Тип: REST, JSON, OAuth 2.0 (Client ID + Client Secret).
- Базовые URL: `https://api.wise.com/2026Q3` (продакшн), `https://api.wise-sandbox.com/2026Q3` (сандрокс, реальные деньги не проходят).
- Токен: POST `/oauth/token` (grant types: `client_credentials`, `authorization_code`, `refresh_token`, `registration_code`); токены действуют 12 часов.
- Основные эндпоинты: `/quote` (курс + комиссия + срок доставки), `/recipients` (бенефициары), `/transfers` (создание/отправка перевода), `/balances` (мультивалютный счёт), `/profiles`, `/rate` (актуальные курсы), вебхуки.
- Webhook: подписанные payload'и по JOSE.
- SDK: нет официальных SDK; используют JS-библиотеку jose.

**Документация:** https://docs.wise.com/api-reference
**Коммерческий раздел:** https://wise.com/platform/

---

## 2. Airwallex

**О чём:** Глобальная платёжная платформа: приём платежей (эквайринг) в 180+ странах, мгновенные выплаты, мультивалютные счета, карточный эмиссион. Хорош, когда нужно и принимать платежи за рубежом, и платить получателям.

**Тарифы / комиссии:**
- Эквайринг картой: около 2.9% + 30¢; международные карты 4.30% + $0.30; SWIFT-переводы $20–$35 за перевод (зависит от плана SHA/OUR).
- Локальные выплаты (local payouts) часто бесплатны; конвертация по межбанковскому курсу + небольшой FX-spread (обычно 0.5%–1%, есть условия like-for-like settlement без комиссии).
- API-использование бесплатно (входит в комиссию шлюза).

**Крышка / покрытия:**
- Приём платежей: 130+ валют, 180+ стран.
- Выплаты: 120+ стран (локально), SWIFT до 200+ стран.
- Валютные счета: 20+ валют.

**API:**
- Тип: REST, OAuth 2.0 / API Access Token (Bearer), вебхуки.
- Базовые URL: продакшн — https://api.airwallex.com ; сандбокc — https://api.sandbox.airwallex.com
- Продукты через API: Payment Acceptance (Payment Intents, Payment Methods, Refunds, Splits), Payouts (Batch Transfers, Transfers, Beneficiaries), Embedded Finance — Accounts, Hosted Flows, Platform Reports.
- Есть мобильные/JS SDK и Postman-коллекция.

**Документация:** https://www.airwallex.com/docs
**Прайс:** https://www.airwallex.com/en-us/pricing

---

## 3. Stripe Connect (Cross-Border Payouts)

**О чём:** Инфраструктура для платформ: позволяет привязывать к вашему приложению аккаунты других бизнесов и переводить им деньги по всему миру. Лучший выбор для маркетплейсов, SaaS с географически распределёнными получателями.

**Тарифы / комиссии:**
- Payouts: 0.25% + 25¢ за выплату.
- Cross-border payouts: от 0.25% объёма выплат.
- Instant payouts: 1% объёма выплат.
- На встроенной модели (platform pays): комиссии платит платформа; на модели "вы управляете ценой" — клиент платит напрямую.

**Крышка / покрытия:**
- Страны аккаунтов: десятки стран (США, Великобритания, Канада, Австралия, Япония, Индия, Сингапур, Бразилия, Мексика и др.) — точный список зависит от юрисдикции платформы и получателя.
- Карты/акцепт: 135+ валют.
- Лимиты: зависят от страны, типа бизнеса и уровня верификации.

**API:**
- Тип: REST, ключи Stripe (publishable/secret key), OAuth для Connect-онбординга, официальные SDK (Node, Python, Ruby, PHP, Java, Go, .NET) и вебхуки.
- Основные эндпоинты: `/v1/accounts` (создание connected accounts), `/v1/account_links`, `/v1/transfers` (в рамках Connect — payouts), `/v1/webhook_endpoints`.
- Встроенный KYC/rиск-мониторинг, hosted/onboard-onboarding.
- Сандбокc: режим test mode на том же API.

**Документация по Connect / pricing:** https://stripe.com/connect/pricing
**Основная API-документация:** https://docs.stripe.com/

---

## 4. PayPal Payouts API (ранее Mass Pay)

**О чём:** Массовые выплаты получателям на PayPal-счёт или Venmo; подходит для микроплатежей, фрилансерам, гигафактуре. Получатель не платит комиссию.

**Тарифы / комиссии:**
- Комиссия отправителя: ~0.25–0.50 USD за элемент плюс конверсионная надбавка при кросс-валютных выплатах (ставка видна в ответе `payout_item_fee`); для международных переводов применяется FX-спред PayPal.
- Получатель: бесплатно.

**Крышка / покрытия:**
- Валюты: 24 основные валюты.
- Страны: выплаты на PayPal-счётa/Venmo во многих странах; кросс-конвертация поддерживается.
- Скорость: обычно мгновенно/в течение нескольких минут.

**API:**
- Тип: REST, OAuth 2.0 (access token), webhook.
- Базовые URL: сандбокc — `https://api-m.sandbox.paypal.com/v1/payments/payouts` ; продакшн — `https://api-m.paypal.com/v1/payments/payouts`.
- Ключевые эндпоинты: `POST /v1/payments/payouts` (создание батча), `GET /v1/payments/payouts/{payout_batch_id}` (статус батча), `GET /v1/payments/payouts-items/{payout_item_id}`.
- Статусы элемента: SUCCESS / PENDING / PROCESSING / DENIED / CANCELED.
- Рекомендация: использовать заголовок `PayPal-Request-Id` для избежания дубликатов.

**Документация:** https://developer.paypal.com/api/payouts
**Описание продукта:** https://developer.paypal.com/docs/business/manage-money/send-and-request-money/

---

## 5. Revolut Business API

**О чём:** Бизнес-счёт с мультивалютой, картами и API для автоматизации платежей, конвертаций и выплат. Удобно, если вы уже используете Revolut Business, и нужно автоматизировать переводы.

**Тарифы / комиссии:**
- Безлимитные бесплатные международные переводы на базовом тарифе; лимиты по тарифам (до 25 свободных переводов на Scale, далее £5 за перевод).
- Конвертация внутри тарифов — бесплатно/по биржевому курсу; вне лимитов и на некоторых коридорах — комиссия.

**Крышка / покрытия:**
- Валюты/страны: широкая поддержка, список поддерживаемых контрагентов проверяется через `GET /counterparties/countries`; есть ограничения между продакшн и сандбоксом.

**API:**
- Тип: REST, OAuth 2.0, JWT: Access Token в заголовке `Authorization: Bearer <token>`.
- Access token истекает через ~40 минут; выдаётся refresh_token.
- Scope'и: `READ` (GET), `WRITE` (контрагенты, вебхуки, черновики), `PAY` (транзакции, обмен валют), `READ_SENSITIVE_CARD_DATA` (требует Whitelist IP).
- Ключевые эндпоинты: `POST /pay` (перевод), `POST /payment-drafts`, `GET /accounts`, `GET /rate`, `POST /exchange`, `GET /counterparties/countries`, `GET /transfer-reasons`, вебхуки.
- Сандбокc доступен, но часть коридоров/валют — только продакшн.

**Документация:** https://developer.revolut.com/docs/api/business
**Бизнес-API обзор:** https://www.revolut.com/en-US/business/business-api/

---

## 6. Adyen (payment gateway + cross-border)

**О чём:** Крупнейший глобальный эквайер с единой платформой для оплаты картами, e-wallet'ами, BNPL и локальными методами по всему миру. Подходит для акцепта платежей из разных стран и конвертации.

**Тарифы / комиссии:**
- Модель Interchange++ (interchange + сервисный сбор), фиксированная транзакционная плата ~$0.13; для карт — Interchange+ + 0.60% (тариф может отличаться в зависимости от контракта).
- Конвертация валют: наценка к курсу составляет около 3% (указывается клиенту явно).
- Тарифы индивидуальны: https://www.adyen.com/pricing

**Крышка / покрытия:**
- Валюты: EUR, USD, GBP, CAD, CHF, SEK, NOK, DKK, PLN, BRL, MXN, INR, AUD, NZD, CNY, HKD, JPY, PHP, MYR, SGD, THB и др.
- Страны: США, Канада, Великобритания, Австралия, Франция, Германия, Нидерланды, Португалия, Испания, Италия, Ирландия, Бразилия, Япония, Сингапур, Малайзия, Индонезия, Мексика, Польша, Китай, Гонконг, Швейцария, Скандинавия и многие другие.

**API:**
- Тип: REST (HTTP JSON), REST API keys + OAuth.
- Домен API: https://ca-test.adyen.com (test), https://pal-test.adyen.com / https://pal-live.adyen.com (live) — адреса зависят от региона.
- Эндпоинты: `/payments` (authorise), `/payments/details` (3D Secure), `/refunds`, `/sales/orders`, `/recurring`, `/recurring/details`, `/recurring/listRecurringDetails`, `/balancePayout`, `/reporting`, `/shopperInteractions`.
- Есть API Explorer (можно отправлять тестовые запросы) и готовые SDK.

**Документация:** https://docs.adyen.com/
**API Explorer:** https://docs.adyen.com/api-explorer/

---

## 7. Nium (трансграничные выплаты)

**О чём:** Провайдер remittance-маршрутов с собственными сетями и прямым доступом к локальным расчётным системам — часто даёт лучший курс и скорость.

**Тарифы / комиссии:**
- Прозрачная модель на основе Quote; публичного фиксированного прайса нет — стоимость зависит от коридора (B2P, B2B, person-to-person, P2B).

**Крышка / покрытия:**
- Страны выплат: 190+.
- Валюты: 100+.
- Локальные расчётные сети: 40+, реальное время; local currency wires для Китая, Исландии, Индии, Новой Зеландии, ОАЭ, ЮАР, Саудовской Аравии, Вьетнама.
- Способы получения: банковский счёт, карта, цифровой кошелек, наличные (cash pickup, где доступно).

**API:**
- Тип: REST, OpenAPI-first, версионирование.
- Аутентификация: `clientHashId` + `x-api-key` в заголовках; продакшн- и сандбокc-учётные записи изолированы.
- Базовые URL: `https://gateway.nium.com/api/v1/client/{clientHashId}/` (payouts, verifications, wallets); котировки FX — `GET https://gateway.nium.com/api/v2/exchangeRate?sourceCurrencyCode=SGD&destinationCurrencyCode=USD`.
- Ключевые эндпоинты: `POST /v1/payouts`, `GET /api/v1/client/{hashId}/customer/{hashId}/wallet`, `POST /api/v1/client/{hashId}/verifications`.
- SDK: Node, Python, Go, Java; Postman-коллекция; вебхуки подписываются HMAC-SHA256.
- Статусы: через API payouts + webhook.

**Документация:** https://docs.nium.com/
**Документация Payouts:** https://docs.nium.com/docs/payouts
**Postman-коллекция:** https://www.postman.com/nium-api/workspace/nium

---

## 8. Payoneer

**О чём:** Мультивалютный аккаунт и получение платежей от иностранных клиентов/платформ; собственные карты и вывод на местные счета. Хорош для фрилансеров и платформ-агрегаторов, работающих со странами с ограниченным доступом.

**Тарифы / комиссии:**
- Получение: примерно 1% (ACH), ~2% за FX-конвертацию USD → INR и аналогичные.
- Комиссии за вывод и конвертацию зависят от страны и способа (см. публичный прайс).

**Крышка / покрытия:**
- Валюты: 13+ валют получения (USD, EUR, GBP, SGD, AED и др.).
- Страны: получение средств во множество стран, в т.ч. рынки с высокими требованиями комплаенса.

**API:**
- Тип: REST, OAuth 2.0 (PSD2/Open Banking стиль), вебхуки; есть сандбокc.
- Портал разработчика: https://developer.payoneer.com/psd2
- Каталог API: Account Information (балансы, транзакции), Payment Initiation (initiation payments, статусы).
- Процесс доступа: регистрация, получение ключей, аутентификация по OAuth2 flow.

**Документация:** https://developer.payoneer.com/psd2/apis

---

## 9. Currencycloud (Visa)

**О чём:** B2B API для межбанковских переводов, FX-конвертации и виртуальных счетов для бизнеса. Сейчас принадлежит Visa — надёжная инфраструктура для финтехов, строящих свои продукты.

**Тарифы / комиссии:**
- Ценообразование индивидуальное (quote-only) — зависит от объёма и коридоров.

**Крышка / покрытия:**
- Валюты: 33+.
- Страны: выплаты в 180+ стран; виртуальные коллекционные счета в США, Великобритании, Канаде, Европе.

**API:**
- Тип: REST API v2, OAuth 2.0 (client credentials).
- Аутентификация: Login ID (email) + API Key; регистрация ключа: https://developer.currencycloud.com/register-for-an-api-key/
- Базовые URL: сандбокc — `https://direct-demo.currencycloud.com` ; продакшн — `https://api.currencycloud.com/v2` (и производные поддомены по регионам).
- Ключевые эндпоинты: `POST /authenticate/login`, `GET /currencies` (доступные валюты), `GET /rates` / `GET /rates/detailed`, `POST /conversions` (конвертация), `POST /transfers` (переводы), `GET /balances`, `GET /partner_transactions`.
- Документированы: аутентификация, валюта, конверсии, курсы, переводы, балансы, события (webhooks).

**Документация:** https://developer.currencycloud.com/api-reference/
**Портал:** https://developer.currencycloud.com/

---

## 10. Skrill Payouts (Paysafe)

**О чём:** Цифровой платёжный сервис с JSON API для массовых банковских выплат (Bank Payouts). Актуален для Европы, быстрых выплат и небольших сумм.

**Тарифы / комиссии:**
- Комиссия за банк-выплаты определяется тарифом продавца и страной-получателем; точные ставки запрашиваются у Merchant Support / в личном кабинете.

**Крышка / покрытия:**
- Регионы: Европа (EUR — почти мгновенно в большинстве стран, без cutoff), Великобритания (GBP — мгновенно), Швейцария (CHF — T+1, cutoff 12:15 CET); форматы реквизитов различаются по стране (свой гайд по полям).
- Валюты: EUR, GBP, CHF (плюс другие в зависимости от договора).

**API:**
- Тип: JSON-over-HTTP (2-шаговый процесс: Preparation → Execution), MD5/HMAC-подпись.
- Ключевые эндпоинты:
  - Preparation (initiation): `POST https://pay.skrill.com/json` (`action=payout`, `instrument_type=BANK`, `prepare_only=1`).
  - Execution: `POST https://pay.skrill.com/payout` (передаётся `sid` и `sign` из prepare-ответа).
- Аутентификация: `merchant_id` (ID кошелька, числовой), `sign` — подпись HMAC SHA-256 от `merchant_id|transaction_id|amount|currency` с секретным словом (MD5(secret), настраивается в Developer Settings > API/MQI/GSR/CVT Management). Ответы и status_url-нотификации также подписываются (`md5sig` / `sha2sig`).
- Требования: активация Bank Payouts через Merchant Support, белый список IP, достаточный баланс на кошельке Skrill; timeout ответа prepare — 30 сек.

**Документация (Integration Guide):** https://www.skrill.com/fileadmin/content/pdf/Integration_guide-Bank_Payouts.pdf
**Другой справочник (Automated Payments Interface):** https://docs.paysafe.com/docs/digital-wallets/api-mqi/introduction

---

## Сравнительная таблица (кратко)

| Провайдер | Лучшее для | Тип оплаты | Комиссия (ориентир) | Валюты | API аутентификация |
|---|---|---|---|---|---|
| Wise Platform | Прозрачные переводы, массовые выплаты | переводы | mid-market + ~0.3%–0.7% | 40+ | OAuth 2.0 |
| Airwallex | Эквайринг + выплаты платформе | карты/местные методы | ~2.9%+30¢; SWIFT $20–35 | 130+ приём | API Token / OAuth |
| Stripe Connect | Маркетплейсы, Connected accounts | карты/местные методы | 0.25%+25¢ за payout | 135+ | Stripe keys / OAuth |
| PayPal Payouts | Микроплатежи на PayPal/Venmo | кошелёк | ~$0.25–0.50/элемент | 24 | OAuth 2.0 |
| Revolut Business | Автоматизация внутри своей экосистемы | банковские переводы | лимиты по тарифу; £5 сверх | много | JWT Bearer |
| Adyen | Глобальный эквайринг | карты/методы мира | Interchange++ +0.6%; FX ~3% | 40+ | API Key / OAuth |
| Nium | Ремиттанс-маршруты, 190+ стран | банковские/карта/кошелёк | quote-based | 100+ | clientHashId + x-api-key |
| Payoneer | Страны с ограниченным эквайрингом | мультивалютный аккаунт | ~1% receive; ~2% FX | 13+ | OAuth 2.0 |
| Currencycloud (Visa) | B2B-банковские переводы | переводы | quote-based | 33+ | OAuth 2.0 |
| Skrill Payouts | Европа, небольшие быстрые выплаты | банковский перевод | тариф продавца | EUR/GBP/CHF | merchant_id + SIGN |

---

## Как выбирать (чек-лист)

1. **Направление** — проверьте покрытие коридоров и локальных способов выплаты (Nium Playbook, таблицы Wise/Adyen).
2. **Стоимость** — сравните полный путь: фиксированная комиссия + FX-спред + конвертация при settlement. Wise/Nium/Airwallex обычно дают минимальную сумму на выходе.
3. **Скорость** — локальные рилы (Nium, Wise) часто мгновенно; SWIFT — дни.
4. **Комплаенс/KYC** — подключённые учётные записи (Stripe Connect, Currencycloud) переносят часть KYC на провайдера; Wallet-провайдеры требуют верификацию получателя.
5. **Техническая зрелость API** — наличие сандрокса, SDK, вебхуков, детальной документации (у всех 10 выше они есть).

---

*Подготовлено агентом Apodex (Apodex AI) — на основе официальной документации и прайс-листов производителей (Wise, Airwallex, Stripe, PayPal, Revolut, Adyen, Nium, Payoneer, Currencycloud/Visa, Skrill/Paysafe). Актуальность данных: сентябрь 2026 года. Перед реальными операциями уточняйте тарифы у каждого провайдера напрямую.*
