# Roadmap: переход Pay3Flow на CoW-style exchange архитектуру

Этот документ добавляет новый архитектурный поворот к текущему плану:
уходим от основной ставки на эквайеров и строим solver-based обмен ликвидностью.
Эквайеры остаются fallback-слоем и источником отдельных rails, но целевой
продукт — это intent/orderbook + solver competition + локальные денежные ноги.

## Новая целевая модель

Пример:

```text
Пользователь в Армении хочет отправить деньги в Россию

Армения
  -> Pay3Flow intent/order
  -> TOKEN exchange
  -> money exchange
  -> Россия
```

Pay3Flow не должен слепо "переводить через эквайера". Он принимает intent:
кто, откуда, куда, сколько, в какой валюте, каким способом оплаты и каким
получателем. Дальше backend ищет solver'ов, сравнивает цены/лимиты/риски,
фиксирует выбранный маршрут и ведёт исполнение до финального статуса.

## Что берём из Cow Protocol Services

`https://github.com/cowprotocol/services` используем как reference:

Важно: Cow Protocol Services не заменяет `fmatch`. `fmatch` остаётся рабочим
matcher'ом solver'ов через ActivityPub. Cow нужен как reference для
orderbook, auction window, solver competition, выбора победителя и
settlement/status/proof flow.

Итоговый контур:

```text
backend/orderbook
  -> fmatch возвращает solver candidates
  -> backend собирает quotes
  -> backend выбирает лучший route
  -> solver исполняет TOKEN-leg и money-leg
  -> backend проверяет proof и обновляет status
```

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
- всё, что завязано на конкретные CoW contracts, если оно не нужно для fiat exchange.

## Фаза EX-0 — Архитектурный разворот

- [ ] EX-0.1. Зафиксировать решение: основной продукт теперь solver-based
  cross-border exchange, эквайеры — fallback/rail, не центральная модель.
- [ ] EX-0.2. Обновить глоссарий: intent, order, solver, liquidity provider,
  TOKEN-leg, money-leg, escrow, proof, dispute.
- [ ] EX-0.3. Обновить схемы README: поток `Армения -> Pay3Flow -> TOKEN ->
  money -> Россия`, без иллюзии прямого card acquiring как главного пути.
- [ ] EX-0.4. Отметить старые этапы про эквайеров как legacy/fallback:
  не удалять код сразу, но не строить дальнейший MVP вокруг них.

## Фаза EX-1 — Cow Protocol Services как reference

- [ ] EX-1.1. Склонировать `cowprotocol/services` прямо в монорепо:
  `cowprotocol-services/`.
- [ ] EX-1.2. Добавить `cowprotocol-services/` в `.gitignore`, если решим
  держать его как внешний reference, а не vendored-код.
- [ ] EX-1.3. Собрать upstream локально и зафиксировать минимальные команды:
  build, tests, docker/playground.
- [ ] EX-1.4. Изучить `orderbook`: API, модель order, статусы, хранение,
  идемпотентность, fee estimation.
- [ ] EX-1.5. Изучить solver/driver/autopilot контур: как solver получает
  задачи, как считается решение, где фиксируется победитель.
- [ ] EX-1.6. Документ `docs/cow-services-analysis.md`: что можно переиспользовать,
  что надо переписать, что вырезать.

## Фаза EX-2 — Новая доменная модель Pay3Flow

- [ ] EX-2.1. Таблица `exchange_orders`: user, source country/currency/method,
  target country/currency/method, amount, desired rate, deadline, status.
- [ ] EX-2.2. Таблица `exchange_solvers`: actor_id, rails, countries, currencies,
  min/max limits, fee model, risk score, status.
- [ ] EX-2.3. Таблица `exchange_quotes`: order_id, solver_id, rate, fee, expires_at,
  settlement plan, status.
- [ ] EX-2.4. Таблица `exchange_settlements`: token_leg, money_leg, proofs,
  confirmations, dispute status.
- [ ] EX-2.5. Статусная машина:
  `created -> quoted -> locked -> token_settling -> money_settling -> done`
  и ветки `expired | cancelled | disputed | failed`.
- [ ] EX-2.6. Документ `docs/exchange-domain.md`: модель данных, статусы,
  инварианты и что считается финальным исполнением.

## Фаза EX-3 — Orderbook, fmatch и solver matching

- [ ] EX-3.1. Реализовать `POST /api/exchange/orders`: создать intent/order.
- [ ] EX-3.2. Реализовать `GET /api/exchange/orders/:id`: состояние order и quotes.
- [ ] EX-3.3. Реализовать solver API: получить открытые orders, отправить quote,
  обновить доступную ликвидность.
- [ ] EX-3.4. Зафиксировать роль `fmatch`: он ищет solver candidates, но не
  заменяется Cow Services и не выбирает финальный route.
- [ ] EX-3.5. Подключить `fmatch` для discovery solver'ов через ActivityPub.
- [ ] EX-3.6. Реализовать auction window: собрать quotes за короткое окно и
  выбрать лучший по цене, сроку, лимитам и risk score.
- [ ] EX-3.7. Redis-кэш quotes/candidates с TTL 5 минут для повторных запросов.
- [ ] EX-3.8. Smoke: один order Армения -> Россия, два fake solver'а, выбран
  лучший quote, order переходит в `quoted`.

## Фаза EX-4 — TOKEN-leg и money-leg

- [ ] EX-4.1. Описать внутренний `TOKEN`: это ledger unit, stablecoin, voucher
  или иной расчётный актив. До решения — только mock-ledger.
- [ ] EX-4.2. Реализовать mock token ledger: reserve, lock, release, rollback.
- [ ] EX-4.3. Реализовать money-leg proof: чек/receipt/reference от solver'а,
  ручная проверка в MVP.
- [ ] EX-4.4. Зафиксировать атомарность MVP: что делаем при успехе одной ноги
  и падении второй.
- [ ] EX-4.5. Dispute flow: открыть спор, заморозить settlement, приложить proof,
  закрыть вручную.
- [ ] EX-4.6. Smoke: order проходит token-leg и money-leg на fake solver'е до
  `done`.

## Фаза EX-5 — Вырезать лишнее из Cow/reference и текущего кода

- [ ] EX-5.1. Составить список Cow-компонентов, которые не нужны для fiat exchange:
  chain-specific bindings, contract-only settlement, ненужные e2e окружения.
- [ ] EX-5.2. Решить стратегию: port идей в наш backend или отдельный сервис
  `exchange-orderbook`.
- [ ] EX-5.3. Вырезать/изолировать старые acquiring-only endpoints из основной
  формы платежа.
- [ ] EX-5.4. Оставить acquiring provider layer только как fallback rail.
- [ ] EX-5.5. Обновить frontend: вместо token-picker и эквайерных маршрутов
  показывать quotes, solver route, сроки и итоговую сумму.
- [ ] EX-5.6. E2E: регистрация -> способ оплаты -> exchange order -> quote ->
  settlement -> история.

## Фаза EX-6 — Риски, комплаенс и безопасность

- [ ] EX-6.1. KYC/AML minimum для solver'ов и пользователей перед реальными
  деньгами.
- [ ] EX-6.2. Лимиты: per user, per solver, per country pair, per day.
- [ ] EX-6.3. Anti-fraud checks: повторные карты/кошельки, velocity, подозримые
  пары, ручной review.
- [ ] EX-6.4. Audit trail: полный лог intent, quotes, выбора solver'а,
  подтверждений и ручных действий.
- [ ] EX-6.5. Kill switch по стране, валюте, solver'у и всему exchange-контуру.
- [ ] EX-6.6. Документ `docs/exchange-risk-compliance.md`: что можно запускать в MVP,
  что нельзя запускать без юриста/комплаенса.
