# PLAN2: Pay3Flow CoW-style exchange architecture

Этот документ написан как инструкция для следующего ИИ-агента. Цель: агент должен понимать продукт, роли сервисов, порядок работ и решения по умолчанию без дополнительных вопросов к владельцу проекта.

## 0. Главная Идея

Pay3Flow больше не строится вокруг эквайеров как центрального способа перевода. Эквайеры остаются fallback/rail, но основной MVP строится как solver-based cross-border exchange.

Пользователь не выбирает провайдера сам. Пользователь формулирует intent:

```text
Хочу отправить деньги из страны A в страну B.
У меня есть source amount/source currency/source method.
Получатель должен получить target currency/target method.
Я хочу понятный курс, комиссию, срок и статус.
```

Backend превращает этот intent в order, ищет solver'ов через `fmatch`, собирает quotes, выбирает лучший route, ведёт settlement и проверяет proofs.

Главная рабочая установка: разработка должна идти быстро и автономно. На текущем этапе владельца проекта не нужно вовлекать в ежедневные технические решения. Агент сам выбирает безопасный, простой и расширяемый вариант, реализует его, проверяет тестами и двигается дальше.

## 1. Целевой Поток

Базовый пример:

```text
Армения
  -> Pay3Flow intent/order
  -> TOKEN exchange
  -> money exchange
  -> Россия
```

Человечески:

1. Пользователь в Армении хочет отправить деньги получателю в России.
2. Pay3Flow сохраняет заявку как exchange order.
3. Backend отправляет задачу в `fmatch`.
4. `fmatch` возвращает подходящих solver'ов.
5. Backend собирает quotes от solver'ов.
6. Backend выбирает лучший route.
7. Solver принимает/подтверждает первую денежную ногу.
8. Solver выполняет TOKEN-leg или внутренний расчёт.
9. Solver выполняет money-leg получателю.
10. Solver присылает proof.
11. Backend проверяет proof, обновляет status и показывает результат в UI.

Вариант funding flow для MVP:

```text
user
  -> Pay3Flow form
  -> user-initiated funding/payment instruction
  -> solver
  -> Pay3Flow wallet / internal TOKEN ledger
  -> solver
  -> recipient
```

Смысл:

- пользователь заполняет простую форму перевода в Pay3Flow;
- Pay3Flow показывает сумму, курс, комиссию, ETA и условия;
- пользователь сам подтверждает действие и сам инициирует funding/payment;
- Pay3Flow не принимает фиат на свой баланс и не распоряжается фиатом пользователя;
- solver выполняет crypto/TOKEN purchase или внутренний расчётный leg как часть settlement;
- Pay3Flow wallet/internal ledger используется для фиксации TOKEN-leg, proof и статусов;
- второй solver leg доставляет деньги получателю через выбранный rail.

Важно для агента: нельзя строить продукт на обмане пользователя. В UI можно не перегружать человека словами про инфраструктуру, но в условиях, подтверждении операции и compliance-документах должно быть прозрачно написано, что маршрут может использовать crypto/TOKEN settlement asset. Формулировка для продукта: "Pay3Flow подбирает маршрут обмена и расчёта; пользователь подтверждает условия, сумму, комиссию и способ исполнения".

## 2. Главное Архитектурное Решение

`fmatch` не заменяется Cow Protocol Services.

Роли:

```text
backend/orderbook
  -> создаёт и хранит exchange orders
  -> отправляет задачу в fmatch

fmatch
  -> ищет solver candidates через ActivityPub
  -> возвращает candidates backend'у
  -> не выбирает финальный route
  -> не исполняет деньги

backend/auction
  -> собирает quotes от solver candidates
  -> сравнивает quotes
  -> выбирает лучший route
  -> фиксирует winner

solver
  -> даёт quote
  -> подтверждает readiness
  -> исполняет TOKEN-leg и money-leg
  -> присылает proof

backend/settlement
  -> ведёт статусную машину
  -> проверяет proof
  -> пишет audit trail
  -> отдаёт статусы frontend'у

cowprotocol-services/
  -> локальный reference
  -> источник идей orderbook, auction, solver competition, settlement flow
  -> не production-зависимость без отдельного решения
```

## 3. Что Берём Из Cow Protocol Services И Meta Matcha

Reference repo:

```text
https://github.com/cowprotocol/services
```

Meta Matcha reference:

```text
https://meta.matcha.xyz/
```

Локальный каталог:

```text
cowprotocol-services/
```

Текущий upstream commit клона:

```text
3017e9400 solana-autopilot: hold in-flight orders out of auction cuts (#4937)
```

Берём идеи:

- `orderbook`: как принимать пользовательские orders, валидировать их, хранить, отдавать состояние и не ломаться при повторных запросах.
- `auction window`: как собрать несколько orders/quotes за короткое окно, чтобы не выбирать route слишком рано.
- `solver competition`: как позволить нескольким solver'ам предложить решение.
- `winner selection`: как выбрать лучший quote по цене, сроку, риску и лимитам.
- `settlement lifecycle`: как вести статусы от создания order до финала.
- `observability`: как логировать критичные финансовые шаги.
- `tests`: как делать unit/smoke/e2e проверки для matching и settlement.
- Meta Matcha UX: простая intent-форма `sell -> buy`, slippage, trade/bridge режимы, route details, intents mode.
- Meta Matcha product pattern: пользователь видит простую форму обмена, а сложный routing/settlement спрятан в деталях маршрута и условиях.

Важно: Meta Matcha не считать "биржей" в плане. Это reference для meta-aggregator / intent UX / route aggregation подхода.

Не переносим вслепую:

- Ethereum-only settlement как обязательное ядро.
- ERC20 approvals/funding checks как единственный источник истины.
- CoW smart contracts как обязательную часть MVP.
- Их production-инфраструктуру целиком.
- Всё, что не нужно для fiat/token exchange MVP.

Решение по умолчанию для агента:

```text
Не копировать большие куски Cow в backend без анализа.
Сначала написать docs/cow-services-analysis.md.
Потом переносить только понятные идеи и маленькие паттерны.
```

## 3.1. Route Aggregation Research

Перед тем как писать финальный production routing, агент должен изучить подходы route/liquidity aggregation у следующих систем:

```text
Meta Matcha
0x
1inch
Barter
Bebop
Bitget
Enso
KyberSwap
Lightning
Nordstern
OKX
Velora
Cow Protocol
```

Что нужно выяснить по каждому:

- это intent protocol, solver auction, DEX aggregator, bridge aggregator, CEX/venue API или hybrid;
- как пользователь формулирует intent/order;
- кто ищет route;
- кто исполняет route;
- есть ли solver competition;
- есть ли RFQ/private market makers;
- поддерживается ли bridge/cross-chain;
- как считаются slippage, fees, minimum received;
- есть ли API/SDK, который можно использовать;
- можно ли использовать как источник quotes для TOKEN-leg;
- какие риски: custody, compliance, geo restrictions, API limits, sanctions, KYC.

Deliverable:

```text
docs/route-aggregation-research.md
```

В документе должна быть таблица:

```text
name
category
what_it_does
how_it_routes
how_it_executes
api_or_sdk
useful_for_pay3flow
risks
decision
```

Решения по умолчанию до завершения research:

- Не завязывать core backend на одного внешнего агрегатора.
- Сделать abstraction `RouteQuoteSource`.
- Для MVP оставить fake/mock quote sources.
- Реальный TOKEN-leg later должен подключаться через adapter.
- `fmatch` всё равно остаётся solver matcher для Pay3Flow solver candidates.
- Внешние агрегаторы могут быть quote/liquidity sources внутри solver или backend adapter, но не заменяют весь Pay3Flow flow.

## 4. Словарь

`Intent`: намерение пользователя. Например: "отправить 100 000 AMD из Армении, получатель должен получить RUB в России".

`Order`: сохранённый intent в базе. Имеет id, user_id, суммы, валюты, страны, методы, status, deadline, idempotency_key.

`Solver`: исполнитель/поставщик ликвидности. Он может принять задачу, дать quote и провести settlement.

`Quote`: предложение solver'а: курс, комиссия, срок, лимиты, план исполнения, expires_at.

`Route`: выбранный quote + solver + конкретный план исполнения.

`Auction window`: короткое окно ожидания quotes. Для MVP default: 3 секунды.

`TOKEN-leg`: внутренний расчётный шаг. В MVP это mock ledger, не реальные деньги.

`Money-leg`: локальная доставка денег получателю через доступный rail.

`Proof`: доказательство исполнения: receipt, reference, webhook, файл, ручное подтверждение или machine-readable receipt.

`Dispute`: спорная ситуация, когда proof невалиден, суммы не совпадают, одна нога зависла или получатель не подтвердил деньги.

`Rail`: технический способ движения денег: карта, банк, кошелёк, cash-in/cash-out, эквайер, крипто-онрамп, внутренний ledger.

## 5. Решения По Умолчанию

Если агент не уверен, использовать эти решения:

- Первый продуктовый коридор MVP: `AMD -> RUB`.
- Точная запись первого коридора: `AM/AMD -> RU/RUB`.
- `AM` = Armenia как страна, ISO 3166-1 alpha-2.
- `AMD` = Armenian dram как валюта, ISO 4217.
- `RU` = Russia как страна, ISO 3166-1 alpha-2.
- `RUB` = Russian ruble как валюта, ISO 4217.
- В коде всегда хранить страну и валюту отдельно: country не равен currency.
- Этот коридор нельзя хардкодить в бизнес-логике. Он должен жить в конфиге, seed-данных, таблицах или тестовых fixtures.
- Код должен быть generic по валютам и странам: `source_currency`, `target_currency`, `source_country`, `target_country`.
- В MVP включён только один коридор через данные: Armenia/AMD -> Russia/RUB.
- Новые валюты и страны позже добавляются через БД/admin/config без переписывания core logic.
- TOKEN в MVP: mock ledger внутри backend.
- Funding flow MVP: user -> Pay3Flow form -> solver -> Pay3Flow wallet/internal ledger -> solver -> recipient.
- Pay3Flow не управляет фиатными деньгами пользователя: пользователь сам подтверждает funding/payment instruction, а backend хранит order/status/proof.
- Crypto/TOKEN leg не должен быть "тайным". Для UX можно показывать простую форму перевода, но terms/consent обязаны раскрывать, что settlement может идти через TOKEN/crypto asset.
- Реальные деньги в MVP на этом этапе не включать.
- Первый solver: fake solver в backend или отдельном mock-сервисе.
- Количество fake solver'ов для smoke: минимум 2.
- Auction window: 3 секунды.
- Quote TTL: 5 минут.
- Redis TTL для candidates/quotes: 5 минут.
- Финальный route выбирает backend, не `fmatch`.
- `fmatch` возвращает candidates, не победителя.
- Если `fmatch` недоступен: использовать Redis cache, если есть живая запись.
- Если cache пустой: использовать локальный fallback solver registry.
- Все денежные значения хранить в minor units: копейки/центы/драмы в целых.
- Все изменения статусов делать guarded update через текущий status.
- Все create-запросы делать идемпотентными через `Idempotency-Key`.
- Все важные шаги писать в audit log.
- Frontend сначала минимальный: новый order, quotes, status, история.

Правила автономной разработки:

- Не задавать владельцу вопросы по деталям, которые можно безопасно решить самому.
- Если есть 2-3 нормальных варианта, выбрать самый простой и расширяемый.
- Если выбор влияет на реальные деньги, безопасность или закон, оставить safe default и описать TODO в docs.
- Не блокировать работу из-за отсутствия реального провайдера, банка, TOKEN или юриста: использовать mock/stub слой.
- После каждого значимого блока запускать тесты или smoke.
- План считать выполненным только когда сервис можно пройти end-to-end на fake деньгах.
- Теоретическая готовность к реальным переводам означает: архитектура, статусы, proof, audit, лимиты, kill switch и ручной review готовы, но реальные деньги включаются отдельным решением.

## 6. Топология Сервисов

```text
frontend
  -> backend
      -> postgres
      -> redis
      -> fmatch      (ActivityPub solver discovery/matching)
      -> crw         (gRPC discovery/search)
      -> solvers     (quotes / execution / proofs)

cowprotocol-services/
  локальный reference для изучения orderbook/solver/autopilot
```

Backend остаётся центром принятия решений.

`fmatch`:

- получает задачу на поиск solver'ов;
- возвращает ranked или unordered candidates;
- не отвечает за money movement;
- не пишет наши orders;
- не выбирает финального winner.

Cow reference:

- не является live-сервисом Pay3Flow;
- изучается локально;
- используется как образец.

## 7. Главный Алгоритм MVP

```text
1. User creates exchange order
2. Backend validates input
3. Backend saves exchange_orders row with status=created
4. Backend sends solver discovery task to fmatch
5. fmatch returns solver candidates
6. Backend saves candidates or reads cached candidates
7. Backend asks candidates for quotes
8. Backend waits auction window
9. Backend filters invalid/expired quotes
10. Backend scores quotes
11. Backend picks winner
12. Backend saves route and status=quoted
13. User confirms selected quote and funding instruction
14. Backend locks order and quote
15. User funding/payment instruction goes to selected solver/rail
16. Solver executes crypto/TOKEN purchase or internal TOKEN-leg
17. TOKEN-leg is reflected in Pay3Flow wallet/internal ledger
18. Solver executes money-leg to recipient
19. Solver submits proof
20. Backend verifies proof
21. Backend finalizes done or disputed/failed
22. Frontend receives status through HTTP/WS
```

## 8. Scoring Algorithm Для Quotes

Для MVP использовать простую deterministic формулу:

```text
score =
  received_amount_score
  - fee_penalty
  - time_penalty
  - risk_penalty
  - manual_review_penalty
```

Правила:

- Quote с истёкшим `expires_at` исключается.
- Quote ниже минимальной суммы получателя исключается.
- Solver со status не `active` исключается.
- Solver выше лимита по сумме исключается.
- Solver с risk_score выше лимита исключается.
- Если два quote равны, выбрать solver с меньшим risk_score.
- Если всё ещё равно, выбрать quote с более ранним `created_at`.

Default веса:

```text
received_amount_score = target_amount_minor
fee_penalty = total_fee_minor
time_penalty = eta_minutes * 10
risk_penalty = risk_score * 100
manual_review_penalty = 5000, если route требует ручной проверки
```

Для smoke можно упростить:

```text
выбрать quote с максимальным target_amount_minor после комиссий,
при равенстве выбрать меньший eta_minutes,
при равенстве выбрать меньший risk_score.
```

## 9. Статусы

Order status:

```text
created
discovering
quoting
quoted
locked
token_settling
money_settling
proof_pending
done
failed
expired
cancelled
disputed
```

Разрешённые переходы:

```text
created -> discovering
discovering -> quoting
discovering -> failed
quoting -> quoted
quoting -> failed
quoting -> expired
quoted -> locked
quoted -> expired
quoted -> cancelled
locked -> token_settling
token_settling -> money_settling
token_settling -> disputed
token_settling -> failed
money_settling -> proof_pending
money_settling -> disputed
money_settling -> failed
proof_pending -> done
proof_pending -> disputed
proof_pending -> failed
disputed -> done
disputed -> failed
```

Запрещено:

- terminal status менять без отдельного admin/manual resolution;
- перескакивать `created -> done`;
- менять winner после `locked`, кроме dispute/manual resolution;
- запускать settlement без locked quote.

Terminal statuses:

```text
done
failed
expired
cancelled
```

`disputed` не terminal: спор должен быть закрыт вручную в `done` или `failed`.

## 10. Минимальная Модель БД

### `exchange_orders`

```text
id uuid primary key
user_id uuid not null
idempotency_key text not null
source_country text not null
source_currency text not null
source_amount_minor bigint not null
source_method_type text not null
source_method_ref text null
target_country text not null
target_currency text not null
target_amount_min_minor bigint null
target_method_type text not null
target_method_ref text null
funding_instruction_id uuid null
funding_status text not null default 'not_started'
status text not null
deadline_at timestamptz null
selected_quote_id uuid null
failure_code text null
failure_message text null
created_at timestamptz not null
updated_at timestamptz not null
```

Индексы:

```text
unique(user_id, idempotency_key)
index(user_id, created_at desc)
index(status, created_at)
```

### `exchange_solvers`

```text
id uuid primary key
slug text unique not null
actor_id text null
handle text null
display_name text not null
status text not null
countries jsonb not null
currencies jsonb not null
rails jsonb not null
min_amount_minor bigint null
max_amount_minor bigint null
fee_model jsonb not null
risk_score integer not null default 50
last_seen_at timestamptz null
created_at timestamptz not null
updated_at timestamptz not null
```

Status:

```text
discovered
active
paused
broken
blocked
```

### `exchange_quotes`

```text
id uuid primary key
order_id uuid not null
solver_id uuid not null
source_amount_minor bigint not null
target_amount_minor bigint not null
source_currency text not null
target_currency text not null
funding_method_type text not null
requires_user_funding boolean not null default true
rate numeric(24, 12) not null
fee_minor bigint not null
eta_minutes integer not null
expires_at timestamptz not null
status text not null
settlement_plan jsonb not null
risk_score integer not null
score bigint null
raw_response jsonb null
created_at timestamptz not null
updated_at timestamptz not null
```

Status:

```text
received
valid
invalid
selected
expired
rejected
```

### `exchange_settlements`

```text
id uuid primary key
order_id uuid not null
quote_id uuid not null
solver_id uuid not null
status text not null
token_leg_status text not null
money_leg_status text not null
funding_status text not null
pay3flow_wallet_ref text null
token_ledger_ref text null
money_reference text null
proof_id uuid null
failure_code text null
failure_message text null
created_at timestamptz not null
updated_at timestamptz not null
```

### `exchange_proofs`

```text
id uuid primary key
settlement_id uuid not null
solver_id uuid not null
proof_type text not null
proof_payload jsonb not null
verification_status text not null
verified_by text null
verified_at timestamptz null
created_at timestamptz not null
```

### `funding_instructions`

```text
id uuid primary key
order_id uuid not null
quote_id uuid not null
solver_id uuid not null
status text not null
method_type text not null
amount_minor bigint not null
currency text not null
destination_ref text not null
expires_at timestamptz not null
user_confirmed_at timestamptz null
raw_payload jsonb not null
created_at timestamptz not null
updated_at timestamptz not null
```

Status:

```text
created
shown_to_user
user_confirmed
solver_acknowledged
received_by_solver
expired
cancelled
failed
```

Правило: funding instruction описывает действие, которое пользователь подтверждает сам. Pay3Flow не должен автоматически списывать фиат или делать вид, что TOKEN-leg не существует.

### `audit_events`

```text
id uuid primary key
entity_type text not null
entity_id uuid not null
event_type text not null
actor_type text not null
actor_id text null
payload jsonb not null
created_at timestamptz not null
```

Каждый важный шаг пишет audit event.

## 11. API Backend

### User API

Создать order:

```text
POST /api/exchange/orders
Authorization: Bearer <jwt>
Idempotency-Key: <uuid>
```

Body:

```json
{
  "source_country": "AM",
  "source_currency": "AMD",
  "source_amount_minor": 10000000,
  "source_method_type": "card",
  "source_method_ref": "payment_method_id",
  "target_country": "RU",
  "target_currency": "RUB",
  "target_amount_min_minor": 2000000,
  "target_method_type": "bank_card",
  "target_method_ref": "recipient_id",
  "deadline_minutes": 30
}
```

Response:

```json
{
  "id": "uuid",
  "status": "created",
  "source_amount_minor": 10000000,
  "source_currency": "AMD",
  "target_currency": "RUB"
}
```

Получить order:

```text
GET /api/exchange/orders/:id
```

Получить quotes:

```text
GET /api/exchange/orders/:id/quotes
```

Подтвердить выбранный quote:

```text
POST /api/exchange/orders/:id/confirm
```

Ответ confirm должен вернуть funding instruction:

```json
{
  "order_id": "uuid",
  "quote_id": "uuid",
  "funding_instruction": {
    "id": "uuid",
    "method_type": "card_or_bank_or_wallet",
    "amount_minor": 10000000,
    "currency": "AMD",
    "expires_at": "2026-09-18T15:00:00Z",
    "display_text": "Confirm funding for the selected exchange route"
  }
}
```

Подтвердить, что пользователь увидел и принял funding instruction:

```text
POST /api/exchange/orders/:id/funding/confirm
```

Отменить order:

```text
POST /api/exchange/orders/:id/cancel
```

История:

```text
GET /api/exchange/orders?limit=20&cursor=...
```

### Solver API

MVP может быть internal/debug API:

```text
GET /api/solver/orders/open
POST /api/solver/orders/:id/quotes
POST /api/solver/settlements/:id/token-leg
POST /api/solver/settlements/:id/money-leg
POST /api/solver/settlements/:id/proofs
POST /api/solver/funding/:id/ack
```

Позже solver API должен получить auth/signatures.

### Debug API Для Smoke

```text
POST /api/debug/exchange/fake-solver/register
POST /api/debug/exchange/orders/:id/run
GET /api/debug/exchange/orders/:id/audit
```

Debug API должен быть выключаемым через env:

```text
ENABLE_DEBUG_ROUTES=false
```

## 12. Интеграция С fmatch

Backend отправляет в `fmatch` задачу:

```text
Нужны solver'ы для:
source_country=AM
source_currency=AMD
target_country=RU
target_currency=RUB
source_amount_minor=...
target_method_type=...
deadline=...
```

`fmatch` возвращает candidates:

```json
[
  {
    "solver_id": "solver-am-ru-fast",
    "actor_id": "https://solver.example/actor",
    "rank": 1,
    "quality": 0.93,
    "price_hint": null,
    "metadata": {}
  }
]
```

Backend обязан:

- сохранить candidates в Redis на 5 минут;
- сопоставить candidate с локальным `exchange_solvers`;
- отфильтровать inactive/broken/blocked solver'ов;
- запросить quote только у допустимых solver'ов;
- при недоступности `fmatch` использовать Redis cache;
- если cache пустой, использовать local fallback registry.

Важно:

```text
fmatch не выбирает winner.
fmatch не исполняет settlement.
fmatch не отвечает за proof.
```

## 13. Mock TOKEN Ledger

Для MVP не нужен реальный токен. Нужен mock ledger.

Таблицы:

```text
ledger_accounts
ledger_entries
ledger_locks
```

Операции:

```text
reserve(order_id, amount)
lock(settlement_id, amount)
release(settlement_id)
rollback(settlement_id)
```

Правила:

- Ledger должен быть идемпотентным.
- Нельзя release без lock.
- Нельзя double release.
- Все операции пишут audit events.
- Для smoke можно выдать fake solver'ам стартовый баланс.

## 14. Proof Verification MVP

Machine-check:

- proof payload валидный JSON;
- settlement_id совпадает;
- solver_id совпадает;
- amount/currency совпадают с quote;
- reference не пустой;
- proof не использован раньше.

Manual-check:

- если `proof_type=manual_receipt`, поставить `verification_status=pending`;
- admin/debug endpoint может перевести в `verified` или `rejected`.

Default для smoke:

```text
fake solver отправляет machine_receipt,
backend автоматически verification_status=verified,
order переходит в done.
```

## 15. Frontend MVP

Минимальный frontend не должен быть маркетинговой страницей. Нужен рабочий кабинет.

Страницы:

- login/register;
- new exchange order;
- quotes/route selection;
- order status;
- history;
- order details.

На форме нового order:

- source country;
- source currency;
- source amount;
- source method;
- target country;
- target currency;
- target method;
- recipient;
- checkbox/consent для условий маршрута и settlement asset;
- submit.

Показывать пользователю:

- сколько отправляет;
- сколько получатель получит;
- комиссия;
- курс;
- ETA;
- статус;
- выбранный route;
- funding instruction после выбора quote;
- что Pay3Flow показывает маршрут и статус, но пользователь сам подтверждает funding/payment;
- если disputed/failed, понятная причина.

Не показывать:

- внутренние solver secrets;
- полный PAN карты;
- приватные actor keys;
- raw proof, если он содержит чувствительные данные.
- misleading текст вроде "мы просто переводим деньги напрямую", если route использует TOKEN/crypto settlement.

## 16. Безопасность И Комплаенс

До реальных денег обязательно:

- user auth работает;
- solver auth работает;
- лимиты по пользователю;
- лимиты по solver'у;
- лимиты по коридору;
- audit trail;
- kill switch;
- manual review;
- KYC/AML решение хотя бы на уровне документа;
- secrets не в репозитории;
- logs не содержат PAN/CVC/token secrets;
- disputes можно закрывать вручную.
- пользовательское согласие на funding/settlement terms сохранено;
- crypto/TOKEN settlement раскрыт в terms/consent, даже если UI остаётся простым.

Запреты:

- не хранить CVC;
- не отдавать полный PAN во frontend;
- не логировать секреты;
- не включать реальные деньги без kill switch;
- не считать mock ledger реальным settlement.
- не скрывать от пользователя, что route может использовать TOKEN/crypto settlement asset;
- не писать в UI/доках, что Pay3Flow управляет фиатом пользователя, если по модели funding делает сам пользователь/solver.

## 17. Фазы Работ

## Фаза EX-0 - Архитектурный Разворот

- [x] EX-0.1. Зафиксировать решение: основной продукт теперь solver-based cross-border exchange, эквайеры - fallback/rail.
- [x] EX-0.2. Обновить README под поток `Армения -> Pay3Flow -> TOKEN -> money -> Россия`.
- [x] EX-0.3. Обновить глоссарий: intent, order, solver, quote, settlement, proof, dispute.
- [x] EX-0.4. Убрать формулировки, которые говорят, что Cow заменяет `fmatch`.
- [ ] EX-0.5. Отметить старые этапы про эквайеров как legacy/fallback в основном roadmap.
- [ ] EX-0.6. Обновить архитектурные диаграммы отдельной схемой exchange-flow.

Приёмка:

- README ясно говорит, что `fmatch` остаётся matcher'ом solver'ов.
- PLAN2 достаточно подробный, чтобы агент не спрашивал базовые вопросы.

## Фаза EX-1 - Cow Protocol Services Как Reference

- [x] EX-1.1. Склонировать `cowprotocol/services` в `cowprotocol-services/`.
- [x] EX-1.2. Держать `cowprotocol-services/` в `.gitignore`.
- [ ] EX-1.3. Собрать upstream локально.
- [x] EX-1.4. Записать команды сборки в `docs/cow-services-analysis.md`.
- [x] EX-1.5. Изучить `orderbook`: API, модель order, статусы, storage.
- [x] EX-1.6. Изучить `autopilot`: как двигается auction/matching.
- [x] EX-1.7. Изучить `driver/solver`: как solver получает задачу и отдаёт решение.
- [x] EX-1.8. Написать `docs/cow-services-analysis.md`.
- [x] EX-1.9. Изучить `https://meta.matcha.xyz/` как reference intent UX: trade/bridge, sell/buy form, slippage, route details, intents mode.
- [x] EX-1.10. Зафиксировать в `docs/cow-services-analysis.md`, что Cow = orderbook/solver reference, Meta Matcha = UX/route aggregation reference, `fmatch` = Pay3Flow solver matcher.

Что написать в `docs/cow-services-analysis.md`:

- какие crates смотрели;
- какие endpoints у orderbook;
- какие статусы есть в Cow;
- что подходит Pay3Flow;
- что не подходит Pay3Flow;
- какие идеи переносим;
- какие файлы/модули в Cow смотреть повторно при реализации.

Приёмка:

- Cow repo собирается или описано, почему не собирается.
- Есть документ анализа.
- В документе явно написано: Cow reference, `fmatch` matcher.

## Фаза EX-1A - Route Aggregation Research

- [ ] EX-1A.1. Создать `docs/route-aggregation-research.md`.
- [ ] EX-1A.2. Изучить Meta Matcha: какую UX/intent модель можно повторить.
- [ ] EX-1A.3. Изучить 0x: Swap API, RFQ, route/liquidity model.
- [ ] EX-1A.4. Изучить 1inch: aggregation API, pathfinder, supported chains.
- [ ] EX-1A.5. Изучить Barter: категория, API, применимость.
- [ ] EX-1A.6. Изучить Bebop: RFQ/solver/quote model, API.
- [ ] EX-1A.7. Изучить Bitget: API/venue/liquidity role, KYC/custody risk.
- [ ] EX-1A.8. Изучить Enso: route API, DeFi routing model.
- [ ] EX-1A.9. Изучить KyberSwap: aggregator API, routing, fees.
- [ ] EX-1A.10. Изучить Lightning: уточнить, это Lightning Network или конкретный provider; описать только после проверки.
- [ ] EX-1A.11. Изучить Nordstern: уточнить категорию и применимость.
- [ ] EX-1A.12. Изучить OKX: DEX/CEX/Wallet APIs, routing, compliance/custody risk.
- [ ] EX-1A.13. Изучить Velora: aggregator/intent model, API.
- [ ] EX-1A.14. Сравнить всё с Cow Protocol подходом.
- [ ] EX-1A.15. Выбрать оптимальный подход для Pay3Flow MVP.
- [ ] EX-1A.16. Спроектировать abstraction `RouteQuoteSource`.
- [ ] EX-1A.17. Зафиксировать решение: какие источники quotes идут в MVP как mock, какие позже как real adapters.

Приёмка:

- Есть `docs/route-aggregation-research.md`.
- Для каждого источника есть category, API/SDK, применимость, риски и decision.
- В решении явно написано, что внешние aggregators не заменяют `fmatch`, а дают route/liquidity/quote source.
- Для MVP выбран самый быстрый путь: fake/mock adapters + интерфейс для будущего подключения.
- Агент может начать кодить `RouteQuoteSource` без вопросов к владельцу.

## Фаза EX-2 - Домен И Миграции

- [x] EX-2.1. Добавить enum/status-модели в Rust.
- [ ] EX-2.2. Добавить миграции: `exchange_orders`, `exchange_solvers`, `exchange_quotes`, `exchange_settlements`, `exchange_proofs`, `audit_events`.
- [ ] EX-2.3. Добавить repo layer для каждой таблицы.
- [ ] EX-2.4. Добавить guarded status transitions.
- [ ] EX-2.5. Добавить idempotency на create order.
- [x] EX-2.6. Добавить unit tests для transition table.
- [ ] EX-2.7. Добавить `docs/exchange-domain.md`.
- [ ] EX-2.8. Добавить таблицу или seed-конфиг `exchange_corridors`: enabled corridor Armenia/AMD -> Russia/RUB.
- [ ] EX-2.9. Запретить создание order для disabled corridor, но сделать это через данные, а не через hardcoded `AMD`/`RUB` в коде.
- [ ] EX-2.10. Добавить таблицу `funding_instructions` и связать её с order/quote/solver.
- [ ] EX-2.11. Добавить поля funding status в order/settlement модели.

Приёмка:

- `cargo test` проходит по доменным тестам.
- Нельзя сделать запрещённый переход статуса.
- Повторный create с тем же `Idempotency-Key` возвращает тот же order.
- AMD -> RUB работает как seed/config.
- Добавление нового corridor требует только новых данных, а не изменения core logic.
- Funding instruction создаётся только после selected quote.
- Funding instruction не списывает деньги автоматически.

## Фаза EX-3 - Orderbook API

- [ ] EX-3.1. Реализовать `POST /api/exchange/orders`.
- [ ] EX-3.2. Реализовать `GET /api/exchange/orders/:id`.
- [ ] EX-3.3. Реализовать `GET /api/exchange/orders`.
- [ ] EX-3.4. Реализовать `GET /api/exchange/orders/:id/quotes`.
- [ ] EX-3.5. Реализовать `POST /api/exchange/orders/:id/cancel`.
- [ ] EX-3.6. Подключить JWT ownership checks.
- [ ] EX-3.7. Добавить validation: amount > 0, currencies not empty, countries ISO-like, deadline sane.
- [ ] EX-3.8. Добавить tests для auth/ownership/idempotency.
- [ ] EX-3.9. Реализовать `POST /api/exchange/orders/:id/confirm`, который создаёт funding instruction.
- [ ] EX-3.10. Реализовать `POST /api/exchange/orders/:id/funding/confirm`, который фиксирует user consent.

Приёмка:

- Пользователь видит только свои orders.
- Нельзя создать order с нулевой/отрицательной суммой.
- Нельзя отменить чужой order.
- Confirm возвращает funding instruction.
- Без user funding confirmation settlement не стартует.

## Фаза EX-4 - fmatch Solver Discovery

- [ ] EX-4.1. Описать payload задачи для `fmatch`.
- [ ] EX-4.2. Реализовать отправку discovery task из backend в `fmatch`.
- [ ] EX-4.3. Реализовать парсинг solver candidates из ответа.
- [ ] EX-4.4. Сопоставить candidates с `exchange_solvers`.
- [ ] EX-4.5. Добавить Redis cache на candidates, TTL 5 минут.
- [ ] EX-4.6. Добавить fallback: Redis cache -> local solver registry -> failed.
- [ ] EX-4.7. Добавить smoke: `fmatch` живой, возвращает candidates.
- [ ] EX-4.8. Добавить smoke: `fmatch` выключен, backend берёт cache/fallback.

Приёмка:

- `fmatch` участвует именно как matcher.
- Backend не ждёт от `fmatch` финального winner.
- При падении `fmatch` create order не ломает весь backend.

## Фаза EX-5 - Solver API И Fake Solvers

- [ ] EX-5.1. Создать fake solver model.
- [ ] EX-5.2. Seed минимум двух fake solver'ов: `fast-low-limit` и `slow-better-rate`.
- [ ] EX-5.3. Реализовать internal solver quote interface.
- [ ] EX-5.3a. Реализовать trait/interface `RouteQuoteSource`: `quote(request) -> route quote`, `health()`, `name()`.
- [ ] EX-5.3b. Реализовать `MockRouteQuoteSource` для MVP.
- [ ] EX-5.3c. Не подключать real 0x/1inch/etc в MVP без research decision и env-gated adapter.
- [ ] EX-5.4. Реализовать `GET /api/solver/orders/open`.
- [ ] EX-5.5. Реализовать `POST /api/solver/orders/:id/quotes`.
- [ ] EX-5.6. Fake solver должен уметь вернуть quote success.
- [ ] EX-5.7. Fake solver должен уметь вернуть quote reject.
- [ ] EX-5.8. Fake solver должен уметь симулировать timeout.
- [ ] EX-5.9. Unit tests: quote validation, expiration, solver status.

Приёмка:

- Один order получает минимум два quote.
- Invalid quote отбрасывается.
- Timeout solver не валит весь auction.

## Фаза EX-6 - Auction И Выбор Winner

- [ ] EX-6.1. Реализовать auction window 3 секунды.
- [ ] EX-6.2. Собрать quotes от candidates.
- [ ] EX-6.3. Отфильтровать expired/invalid quotes.
- [ ] EX-6.4. Посчитать score.
- [ ] EX-6.5. Выбрать winner.
- [ ] EX-6.6. Сохранить selected quote.
- [ ] EX-6.7. Перевести order в `quoted`.
- [ ] EX-6.8. Добавить deterministic tests на scoring.
- [ ] EX-6.9. Добавить tie-break tests.

Приёмка:

- При двух quotes выбирается ожидаемый winner.
- При равных quotes tie-break стабильный.
- Winner не меняется после `locked`.

## Фаза EX-7 - Mock TOKEN Ledger

- [ ] EX-7.1. Добавить таблицы ledger.
- [ ] EX-7.2. Реализовать reserve.
- [ ] EX-7.3. Реализовать lock.
- [ ] EX-7.4. Реализовать release.
- [ ] EX-7.5. Реализовать rollback.
- [ ] EX-7.6. Добавить idempotency для ledger operations.
- [ ] EX-7.7. Добавить tests против double release/double rollback.

Приёмка:

- Нельзя release без lock.
- Нельзя списать больше баланса fake solver'а.
- Повтор операции не удваивает движение ledger.

## Фаза EX-8 - Settlement И Proof

- [ ] EX-8.1. Реализовать создание `exchange_settlements` после confirm/lock.
- [ ] EX-8.2. Реализовать funding instruction lifecycle: created -> shown_to_user -> user_confirmed -> solver_acknowledged.
- [ ] EX-8.3. Реализовать token-leg execution через mock ledger после user funding confirmation.
- [ ] EX-8.4. Отразить TOKEN-leg в Pay3Flow wallet/internal ledger.
- [ ] EX-8.5. Реализовать money-leg fake execution.
- [ ] EX-8.6. Реализовать proof submit.
- [ ] EX-8.7. Реализовать proof verification.
- [ ] EX-8.8. Реализовать переход `proof_pending -> done`.
- [ ] EX-8.9. Реализовать failure paths.
- [ ] EX-8.10. Реализовать dispute paths.

Приёмка:

- Happy path order доходит до `done`.
- Settlement не стартует до user funding confirmation.
- Funding instruction сохраняется и видна в audit trail.
- Bad proof переводит order в `disputed`.
- Failed money-leg переводит order в `failed` или `disputed` по правилу.

## Фаза EX-9 - Audit Trail И Observability

- [ ] EX-9.1. Все create/update/status события пишут `audit_events`.
- [ ] EX-9.2. Добавить correlation_id для order.
- [ ] EX-9.3. Логи структурированные.
- [ ] EX-9.4. Не логировать секреты и полные реквизиты.
- [ ] EX-9.5. Добавить debug endpoint для просмотра audit по order.

Приёмка:

- По одному order можно восстановить всю историю.
- В логах нет CVC/PAN/secrets.

## Фаза EX-10 - Frontend MVP

- [ ] EX-10.1. Страница создания exchange order.
- [ ] EX-10.2. Отображение quotes.
- [ ] EX-10.3. Подтверждение quote.
- [ ] EX-10.4. Страница статуса order.
- [ ] EX-10.5. История orders.
- [ ] EX-10.6. Live status через WebSocket или polling.
- [ ] EX-10.7. Ошибки backend показываются понятно.
- [ ] EX-10.8. Русская локализация основных статусов.
- [ ] EX-10.9. Список доступных corridors тянуть с backend, не хардкодить валюты на frontend.
- [ ] EX-10.10. Для MVP backend отдаёт один enabled corridor: AMD -> RUB.
- [ ] EX-10.11. После выбора quote показать funding instruction и consent.
- [ ] EX-10.12. UI должен быть простым: пользователь видит перевод, сумму, курс, комиссию, ETA и условия; technical TOKEN details можно раскрывать в details/terms.

Приёмка:

- Пользователь может пройти fake exchange flow из UI.
- На мобильном форма не ломается.
- UI не содержит зашитого списка будущих валют.
- Пользователь не может запустить settlement без подтверждения funding instruction.

## Фаза EX-11 - Safety Before Real Money

- [ ] EX-11.1. Документ `docs/exchange-risk-compliance.md`.
- [ ] EX-11.2. Kill switch по всему exchange flow.
- [ ] EX-11.3. Kill switch по country pair.
- [ ] EX-11.4. Kill switch по solver.
- [ ] EX-11.5. Daily limits.
- [ ] EX-11.6. Manual review status.
- [ ] EX-11.7. Admin/manual dispute resolution.
- [ ] EX-11.8. Secret audit.
- [ ] EX-11.9. Terms/consent документирует, что route может использовать TOKEN/crypto settlement asset.
- [ ] EX-11.10. Audit trail хранит факт user consent без хранения лишних sensitive данных.

Приёмка:

- Можно остановить весь flow без деплоя.
- Можно заблокировать одного solver'а.
- Dispute можно закрыть вручную.
- Нельзя запустить route, если terms/consent не подтверждены.

## Фаза EX-12 - End-to-end Smoke

- [ ] EX-12.1. Скрипт: register/login -> create order -> discover solver -> collect quotes.
- [ ] EX-12.2. Скрипт: choose winner -> lock -> token-leg -> money-leg -> proof -> done.
- [ ] EX-12.3. Скрипт: bad proof -> disputed -> manual resolve.
- [ ] EX-12.4. Скрипт: fmatch down -> Redis/local fallback.
- [ ] EX-12.5. Скрипт: repeated Idempotency-Key -> same order.
- [ ] EX-12.6. Скрипт: disabled corridor -> понятная ошибка.
- [ ] EX-12.7. Скрипт: enabled AMD -> RUB corridor -> полный happy path.
- [ ] EX-12.8. Playwright smoke: login -> create exchange -> quotes -> status -> history.
- [ ] EX-12.9. Скрипт: no funding consent -> settlement не стартует.
- [ ] EX-12.10. Скрипт: funding consent -> solver ack -> TOKEN-leg -> money-leg -> done.

Приёмка MVP:

```text
Один пользователь создаёт order AM/AMD -> RU/RUB.
Backend находит fake solver'ов через fmatch или fallback.
Backend получает минимум два quotes.
Backend выбирает winner.
Пользователь подтверждает funding instruction.
Settlement проходит через mock TOKEN ledger.
Fake money-leg завершается proof.
Order получает status=done.
История показывает финальный результат.
Audit trail показывает все шаги.
```

Финальная приёмка всего PLAN2:

```text
Сервис поднимается одной командой через docker compose.
Backend health зелёный.
Frontend открывается.
Пользователь может зарегистрироваться/войти.
Пользователь может создать exchange order для включённого corridor AMD -> RUB.
Валюты и corridor приходят из backend, а не зашиты в frontend.
Backend находит solver candidates через fmatch или fallback.
Backend получает quotes.
Backend выбирает winner.
Funding instruction создана, показана пользователю и подтверждена.
Settlement проходит через mock TOKEN ledger.
Money-leg проходит через fake/manual rail.
Proof проверяется.
Order получает done.
История показывает операцию.
Audit trail полный.
Повторный Idempotency-Key не создаёт дубль.
Падение fmatch покрыто cache/fallback.
Есть kill switch.
Есть лимиты.
Есть dispute/manual review.
Есть тесты и smoke-скрипты.
После этого сервис теоретически готов к реальным переводам: для боевого запуска останется подключить реальные rails/solver'ов, пройти юридический и комплаенс-чек, включить production secrets и лимиты.
```

## 18. Что Агенту Делать Прямо Следующим Шагом

Следующий агент должен идти так:

1. Открыть `PLAN2.md`.
2. Открыть `README.md`.
3. Открыть `docs/glossary.md`.
4. Открыть `cowprotocol-services/README.md`.
5. Открыть `cowprotocol-services/docs/ONBOARDING.md`, если файл есть.
6. Изучить Cow `orderbook`, `autopilot`, `driver/solver`.
7. Создать `docs/cow-services-analysis.md`.
8. Потом начинать EX-2: доменные модели и миграции.

Не спрашивать владельца:

- использовать ли `fmatch`: да, использовать;
- заменяет ли Cow `fmatch`: нет;
- делать ли реальные деньги сразу: нет;
- какой первый corridor: Armenia/AMD -> Russia/RUB;
- как отличать страну от валюты: `AM`/`RU` это страны, `AMD`/`RUB` это валюты;
- хардкодить ли AMD/RUB в коде: нет, только seed/config/data;
- делать ли mock ledger: да;
- сколько fake solver'ов: минимум два;
- кто выбирает winner: backend;
- какой TTL cache: 5 минут;
- какой auction window: 3 секунды.

Спрашивать владельца только если:

- нужно выбрать реальный юридический/комплаенс-подход;
- нужно включить реальные деньги;
- нужно выбрать реальный расчётный TOKEN/stablecoin;
- нужно подключить реального solver'а или провайдера.
