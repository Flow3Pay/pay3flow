# Roadmap: переход Pay3Flow на P2P + CoW-style архитектуру

Этот документ добавляет новый архитектурный поворот к текущему плану:
уходим от основной ставки на эквайеров и строим P2P-обмен ликвидностью.
Эквайеры остаются fallback-слоем и источником отдельных rails, но целевой
продукт — это intent/orderbook + solver competition + локальные денежные ноги.

## Новая целевая модель

Пример:

```text
Пользователь в Армении хочет отправить деньги в Россию

Армения
  -> Pay3Flow intent/order
  -> P2P exchange TOKEN
  -> P2P exchange money
  -> Россия
```

Pay3Flow не должен слепо "переводить через эквайера". Он принимает intent:
кто, откуда, куда, сколько, в какой валюте, каким способом оплаты и каким
получателем. Дальше backend ищет solver'ов, сравнивает цены/лимиты/риски,
фиксирует выбранный маршрут и ведёт исполнение до финального статуса.

## Что берём из Cow Protocol Services

`https://github.com/cowprotocol/services` используем как reference:

- `orderbook` — модель пользовательских ордеров/intents, хранение, API,
  валидация и статусы;
- auction/solver flow — как отдавать открытые ордера solver'ам и выбирать
  лучшее исполнение;
- `autopilot`-подход — отдельный процесс, который режет/публикует раунды
  matching и двигает систему вперёд;
- observability/test patterns — как проверять критичный matching/settlement.

Что не переносим вслепую:

- Ethereum-only settlement как обязательное ядро;
- on-chain ERC20 approvals/funding checks как единственный источник истины;
- всё, что завязано на конкретные CoW contracts, если оно не нужно для fiat P2P.

## Фаза P2P-0 — Архитектурный разворот

- [ ] P2P-0.1. Зафиксировать решение: основной продукт теперь P2P
  cross-border exchange, эквайеры — fallback/rail, не центральная модель.
- [ ] P2P-0.2. Обновить глоссарий: intent, order, solver, liquidity provider,
  TOKEN-leg, money-leg, escrow, proof, dispute.
- [ ] P2P-0.3. Обновить схемы README: поток `Армения -> Pay3Flow -> TOKEN ->
  money -> Россия`, без иллюзии прямого card acquiring как главного пути.
- [ ] P2P-0.4. Отметить старые этапы про эквайеров как legacy/fallback:
  не удалять код сразу, но не строить дальнейший MVP вокруг них.

## Фаза P2P-1 — Cow Protocol Services как reference

- [ ] P2P-1.1. Склонировать `cowprotocol/services` прямо в монорепо:
  `cowprotocol-services/`.
- [ ] P2P-1.2. Добавить `cowprotocol-services/` в `.gitignore`, если решим
  держать его как внешний reference, а не vendored-код.
- [ ] P2P-1.3. Собрать upstream локально и зафиксировать минимальные команды:
  build, tests, docker/playground.
- [ ] P2P-1.4. Изучить `orderbook`: API, модель order, статусы, хранение,
  идемпотентность, fee estimation.
- [ ] P2P-1.5. Изучить solver/driver/autopilot контур: как solver получает
  задачи, как считается решение, где фиксируется победитель.
- [ ] P2P-1.6. Документ `docs/cow-services-analysis.md`: что можно переиспользовать,
  что надо переписать, что вырезать.

## Фаза P2P-2 — Новая доменная модель Pay3Flow

- [ ] P2P-2.1. Таблица `p2p_orders`: user, source country/currency/method,
  target country/currency/method, amount, desired rate, deadline, status.
- [ ] P2P-2.2. Таблица `p2p_solvers`: actor_id, rails, countries, currencies,
  min/max limits, fee model, risk score, status.
- [ ] P2P-2.3. Таблица `p2p_quotes`: order_id, solver_id, rate, fee, expires_at,
  settlement plan, status.
- [ ] P2P-2.4. Таблица `p2p_settlements`: token_leg, money_leg, proofs,
  confirmations, dispute status.
- [ ] P2P-2.5. Статусная машина:
  `created -> quoted -> locked -> token_settling -> money_settling -> done`
  и ветки `expired | cancelled | disputed | failed`.
- [ ] P2P-2.6. Документ `docs/p2p-domain.md`: модель данных, статусы,
  инварианты и что считается финальным исполнением.

## Фаза P2P-3 — Orderbook и solver matching

- [ ] P2P-3.1. Реализовать `POST /api/p2p/orders`: создать intent/order.
- [ ] P2P-3.2. Реализовать `GET /api/p2p/orders/:id`: состояние order и quotes.
- [ ] P2P-3.3. Реализовать solver API: получить открытые orders, отправить quote,
  обновить доступную ликвидность.
- [ ] P2P-3.4. Подключить `fmatch` для discovery solver'ов через ActivityPub.
- [ ] P2P-3.5. Реализовать auction window: собрать quotes за короткое окно и
  выбрать лучший по цене, сроку, лимитам и risk score.
- [ ] P2P-3.6. Redis-кэш quotes/candidates с TTL 5 минут для повторных запросов.
- [ ] P2P-3.7. Smoke: один order Армения -> Россия, два fake solver'а, выбран
  лучший quote, order переходит в `quoted`.

## Фаза P2P-4 — TOKEN-leg и money-leg

- [ ] P2P-4.1. Описать внутренний `TOKEN`: это ledger unit, stablecoin, voucher
  или иной расчётный актив. До решения — только mock-ledger.
- [ ] P2P-4.2. Реализовать mock token ledger: reserve, lock, release, rollback.
- [ ] P2P-4.3. Реализовать money-leg proof: чек/receipt/reference от solver'а,
  ручная проверка в MVP.
- [ ] P2P-4.4. Зафиксировать атомарность MVP: что делаем при успехе одной ноги
  и падении второй.
- [ ] P2P-4.5. Dispute flow: открыть спор, заморозить settlement, приложить proof,
  закрыть вручную.
- [ ] P2P-4.6. Smoke: order проходит token-leg и money-leg на fake solver'е до
  `done`.

## Фаза P2P-5 — Вырезать лишнее из Cow/reference и текущего кода

- [ ] P2P-5.1. Составить список Cow-компонентов, которые не нужны для fiat P2P:
  chain-specific bindings, contract-only settlement, ненужные e2e окружения.
- [ ] P2P-5.2. Решить стратегию: port идей в наш backend или отдельный сервис
  `p2p-orderbook`.
- [ ] P2P-5.3. Вырезать/изолировать старые acquiring-only endpoints из основной
  формы платежа.
- [ ] P2P-5.4. Оставить acquiring provider layer только как fallback rail.
- [ ] P2P-5.5. Обновить frontend: вместо token-picker и эквайерных маршрутов
  показывать P2P quotes, solver route, сроки и итоговую сумму.
- [ ] P2P-5.6. E2E: регистрация -> способ оплаты -> P2P order -> quote ->
  settlement -> история.

## Фаза P2P-6 — Риски, комплаенс и безопасность

- [ ] P2P-6.1. KYC/AML minimum для solver'ов и пользователей перед реальными
  деньгами.
- [ ] P2P-6.2. Лимиты: per user, per solver, per country pair, per day.
- [ ] P2P-6.3. Anti-fraud checks: повторные карты/кошельки, velocity, подозримые
  пары, ручной review.
- [ ] P2P-6.4. Audit trail: полный лог intent, quotes, выбора solver'а,
  подтверждений и ручных действий.
- [ ] P2P-6.5. Kill switch по стране, валюте, solver'у и всему P2P контуры.
- [ ] P2P-6.6. Документ `docs/p2p-risk-compliance.md`: что можно запускать в MVP,
  что нельзя запускать без юриста/комплаенса.
