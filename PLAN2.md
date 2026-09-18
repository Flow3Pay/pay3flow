# PLAN2: Pay3Flow CoW-style exchange architecture

Новый главный план: Pay3Flow больше не строится вокруг эквайеров как
основного способа перевода. Целевая модель — cross-border exchange:
пользователь создаёт intent/order, backend ищет solver'ов и ликвидность,
выбирает лучший quote, проводит TOKEN-leg и money-leg, фиксирует proof и
статус.

Эквайеры остаются как fallback/rail и как один из способов провести отдельную
денежную ногу, но не как центральная архитектура MVP.

## Целевой поток

```text
Армения
  -> Pay3Flow intent/order
  -> TOKEN exchange
  -> money exchange
  -> Россия
```

Пример: пользователь в Армении хочет отправить деньги получателю в России.
Pay3Flow принимает заявку, находит solver'а, который готов принять локальные
деньги в Армении/через доступный rail, выполнить внутренний TOKEN-leg и
доставить рубли получателю в России.

## Что берём из Cow Protocol

Reference repo:

```text
https://github.com/cowprotocol/services
```

Используем как источник архитектурных решений, а не как код, который
слепо встраивается в production.

Полезные части:

- `orderbook` — API и модель пользовательских orders/intents;
- solver flow — как solver'ы получают задачи и возвращают решения;
- auction/competition — выбор лучшего исполнения;
- `autopilot`-подход — отдельный процесс, который двигает matching;
- тесты, observability и storage patterns для критичных финансовых сценариев.

Не переносим вслепую:

- Ethereum-only settlement как обязательное ядро;
- ERC20 approvals/funding checks как единственный источник истины;
- CoW-specific smart contracts, если они не нужны для fiat exchange MVP.

## Новая топология

```text
frontend
  -> backend
      -> postgres
      -> redis
      -> fmatch      (ActivityPub discovery/matching solver'ов)
      -> crw         (gRPC discovery/search)
      -> solvers     (quotes / execution / proofs)

cowprotocol-services/
  локальный reference для изучения orderbook/solver/autopilot
```

## Правила работы

1. Старый acquiring-код не удаляем сразу: сначала изолируем как fallback.
2. Новые фичи MVP строим вокруг exchange orderbook.
3. Каждый этап должен иметь маленькую проверку: unit, smoke или документ.
4. После этапа — коммит с понятным сообщением.
5. Реальные деньги включать только после лимитов, audit trail, KYC/AML и kill switch.

## Фаза EX-0 — Архитектурный разворот

- [x] EX-0.1. Зафиксировать решение: основной продукт теперь solver-based
  cross-border exchange, эквайеры — fallback/rail, не центральная модель.
- [x] EX-0.2. Обновить README под поток
  `Армения -> Pay3Flow -> TOKEN -> money -> Россия`.
- [x] EX-0.3. Обновить глоссарий: intent, order, solver, quote, settlement,
  proof, dispute.
- [ ] EX-0.4. Отметить старые этапы про эквайеров как legacy/fallback в
  основном roadmap.
- [ ] EX-0.5. Обновить архитектурные диаграммы в docs отдельной схемой exchange-flow.

## Фаза EX-1 — Cow Protocol Services как reference

- [x] EX-1.1. Склонировать `cowprotocol/services` в корень монорепо:
  `cowprotocol-services/`. Текущий upstream commit:
  `3017e9400 solana-autopilot: hold in-flight orders out of auction cuts (#4937)`.
- [x] EX-1.2. Держать `cowprotocol-services/` в `.gitignore`, если это внешний
  reference, а не vendored-код.
- [ ] EX-1.3. Собрать upstream локально и записать команды:
  build, tests, docker/playground.
- [ ] EX-1.4. Изучить `orderbook`: API, модель order, статусы, хранение,
  идемпотентность, fee estimation.
- [ ] EX-1.5. Изучить solver/driver/autopilot контур: как solver получает
  задачи, как считается решение, где фиксируется победитель.
- [ ] EX-1.6. Написать `docs/cow-services-analysis.md`: что переиспользуем,
  что переписываем, что вырезаем.

## Фаза EX-2 — Новая доменная модель

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

## Фаза EX-3 — Orderbook и solver matching

- [ ] EX-3.1. Реализовать `POST /api/exchange/orders`: создать intent/order.
- [ ] EX-3.2. Реализовать `GET /api/exchange/orders/:id`: состояние order и quotes.
- [ ] EX-3.3. Реализовать solver API: получить открытые orders, отправить quote,
  обновить доступную ликвидность.
- [ ] EX-3.4. Подключить `fmatch` для discovery solver'ов через ActivityPub.
- [ ] EX-3.5. Реализовать auction window: собрать quotes за короткое окно и
  выбрать лучший по цене, сроку, лимитам и risk score.
- [ ] EX-3.6. Redis-кэш quotes/candidates с TTL 5 минут для повторных запросов.
- [ ] EX-3.7. Smoke: один order Армения -> Россия, два fake solver'а, выбран
  лучший quote, order переходит в `quoted`.

## Фаза EX-4 — TOKEN-leg и money-leg

- [ ] EX-4.1. Описать внутренний `TOKEN`: ledger unit, stablecoin, voucher или
  другой расчётный актив. До решения — mock-ledger.
- [ ] EX-4.2. Реализовать mock token ledger: reserve, lock, release, rollback.
- [ ] EX-4.3. Реализовать money-leg proof: чек/receipt/reference от solver'а,
  ручная проверка в MVP.
- [ ] EX-4.4. Зафиксировать атомарность MVP: что делаем при успехе одной ноги
  и падении второй.
- [ ] EX-4.5. Dispute flow: открыть спор, заморозить settlement, приложить proof,
  закрыть вручную.
- [ ] EX-4.6. Smoke: order проходит token-leg и money-leg на fake solver'е до
  `done`.

## Фаза EX-5 — Вырезать лишнее

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
- [ ] EX-6.3. Anti-fraud checks: повторные карты/кошельки, velocity,
  подозрительные пары, ручной review.
- [ ] EX-6.4. Audit trail: полный лог intent, quotes, выбора solver'а,
  подтверждений и ручных действий.
- [ ] EX-6.5. Kill switch по стране, валюте, solver'у и всему exchange-контуру.
- [ ] EX-6.6. Документ `docs/exchange-risk-compliance.md`: что можно запускать в MVP,
  что нельзя запускать без юриста/комплаенса.

## Ближайшие действия

1. Склонировать `cowprotocol/services` в `cowprotocol-services/`.
2. Собрать его локально и изучить `orderbook`, `driver`, `solver`, `autopilot`.
3. Написать `docs/cow-services-analysis.md`.
4. Спроектировать `exchange_orders`, `exchange_solvers`, `exchange_quotes`,
   `exchange_settlements`.
5. Сделать первый fake-solver smoke: Армения -> Россия без реальных денег.
