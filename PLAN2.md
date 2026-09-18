# PLAN2: Pay3Flow P2P + CoW-style architecture

Новый главный план: Pay3Flow больше не строится вокруг эквайеров как
основного способа перевода. Целевая модель — P2P cross-border exchange:
пользователь создаёт intent/order, backend ищет solver'ов и ликвидность,
выбирает лучший quote, проводит TOKEN-leg и money-leg, фиксирует proof и
статус.

Эквайеры остаются как fallback/rail и как один из способов провести отдельную
денежную ногу, но не как центральная архитектура MVP.

## Целевой поток

```text
Армения
  -> Pay3Flow intent/order
  -> P2P exchange TOKEN
  -> P2P exchange money
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
- CoW-specific smart contracts, если они не нужны для fiat P2P MVP.

## Новая топология

```text
frontend
  -> backend
      -> postgres
      -> redis
      -> fmatch      (ActivityPub discovery/matching solver'ов)
      -> crw         (gRPC discovery/search)
      -> p2p solvers (quotes / execution / proofs)

cowprotocol-services/
  локальный reference для изучения orderbook/solver/autopilot
```

## Правила работы

1. Старый acquiring-код не удаляем сразу: сначала изолируем как fallback.
2. Новые фичи MVP строим вокруг P2P orderbook.
3. Каждый этап должен иметь маленькую проверку: unit, smoke или документ.
4. После этапа — коммит с понятным сообщением.
5. Реальные деньги включать только после лимитов, audit trail, KYC/AML и kill switch.

## Фаза P2P-0 — Архитектурный разворот

- [x] P2P-0.1. Зафиксировать решение: основной продукт теперь P2P
  cross-border exchange, эквайеры — fallback/rail, не центральная модель.
- [x] P2P-0.2. Обновить README под поток
  `Армения -> Pay3Flow -> TOKEN -> money -> Россия`.
- [x] P2P-0.3. Обновить глоссарий: intent, order, solver, quote, settlement,
  proof, dispute.
- [ ] P2P-0.4. Отметить старые этапы про эквайеров как legacy/fallback в
  основном roadmap.
- [ ] P2P-0.5. Обновить архитектурные диаграммы в docs отдельной схемой P2P-flow.

## Фаза P2P-1 — Cow Protocol Services как reference

- [x] P2P-1.1. Склонировать `cowprotocol/services` в корень монорепо:
  `cowprotocol-services/`. Текущий upstream commit:
  `3017e9400 solana-autopilot: hold in-flight orders out of auction cuts (#4937)`.
- [x] P2P-1.2. Держать `cowprotocol-services/` в `.gitignore`, если это внешний
  reference, а не vendored-код.
- [ ] P2P-1.3. Собрать upstream локально и записать команды:
  build, tests, docker/playground.
- [ ] P2P-1.4. Изучить `orderbook`: API, модель order, статусы, хранение,
  идемпотентность, fee estimation.
- [ ] P2P-1.5. Изучить solver/driver/autopilot контур: как solver получает
  задачи, как считается решение, где фиксируется победитель.
- [ ] P2P-1.6. Написать `docs/cow-services-analysis.md`: что переиспользуем,
  что переписываем, что вырезаем.

## Фаза P2P-2 — Новая доменная модель

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

- [ ] P2P-4.1. Описать внутренний `TOKEN`: ledger unit, stablecoin, voucher или
  другой расчётный актив. До решения — mock-ledger.
- [ ] P2P-4.2. Реализовать mock token ledger: reserve, lock, release, rollback.
- [ ] P2P-4.3. Реализовать money-leg proof: чек/receipt/reference от solver'а,
  ручная проверка в MVP.
- [ ] P2P-4.4. Зафиксировать атомарность MVP: что делаем при успехе одной ноги
  и падении второй.
- [ ] P2P-4.5. Dispute flow: открыть спор, заморозить settlement, приложить proof,
  закрыть вручную.
- [ ] P2P-4.6. Smoke: order проходит token-leg и money-leg на fake solver'е до
  `done`.

## Фаза P2P-5 — Вырезать лишнее

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
- [ ] P2P-6.3. Anti-fraud checks: повторные карты/кошельки, velocity,
  подозрительные пары, ручной review.
- [ ] P2P-6.4. Audit trail: полный лог intent, quotes, выбора solver'а,
  подтверждений и ручных действий.
- [ ] P2P-6.5. Kill switch по стране, валюте, solver'у и всему P2P-контуру.
- [ ] P2P-6.6. Документ `docs/p2p-risk-compliance.md`: что можно запускать в MVP,
  что нельзя запускать без юриста/комплаенса.

## Ближайшие действия

1. Склонировать `cowprotocol/services` в `cowprotocol-services/`.
2. Собрать его локально и изучить `orderbook`, `driver`, `solver`, `autopilot`.
3. Написать `docs/cow-services-analysis.md`.
4. Спроектировать `p2p_orders`, `p2p_solvers`, `p2p_quotes`,
   `p2p_settlements`.
5. Сделать первый fake-solver smoke: Армения -> Россия без реальных денег.
