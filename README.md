# Pay3Flow

Трансграничные обменные платежи с комиссией ниже, чем SWIFT.

## Идея

Получаем платёжный intent пользователя → подбираем solver'ов и ликвидность →
проводим две локальные денежные ноги через участников сети →
фиксируем статус, комиссии, подтверждения и спорные ситуации.

Целевой поток:

```text
Армения → Pay3Flow → TOKEN exchange → money exchange → Россия
```

Старый контур через эквайеров остаётся как fallback/исторический слой, но
новая целевая архитектура строится вокруг orderbook + solver competition
по модели CoW Protocol.

## Роли Cow и fmatch

`fmatch` остаётся нашим рабочим matcher'ом solver'ов: через ActivityPub он
находит подходящих исполнителей, принимает/отдаёт offers и возвращает
кандидатов.

`cowprotocol-services/` нужен как reference-архитектура, а не замена `fmatch`.
Из CoW берём идеи orderbook, auction window, solver competition, выбор
победителя, settlement/status/proof flow. В Pay3Flow эти идеи адаптируются
так: backend хранит orders, `fmatch` ищет solver'ов, backend собирает quotes
и выбирает маршрут.

## Микросервисы

| Сервис | Каталог | Стек | Описание |
|--------|---------|------|----------|
| `frontend` | `frontend/` | Next.js + TypeScript | Веб-интерфейс |
| `backend` | `backend/` | Rust (Axum, WebSocket, Postgres, Redis) | Ядро: auth, intents, маршрутизация, статусы, споры |
| `cow-services` | `cowprotocol-services/` | Rust | Референс CoW Protocol Services: orderbook, auction/solver flow, settlement patterns |
| `fmatch` | `fmatch/` | Rust (Axum, sqlx) | Федеративный discovery/matching solver'ов и участников сети |

> `cowprotocol-services/` должен быть локальной копией
> `https://github.com/cowprotocol/services` для изучения и адаптации, а не
> прямой production-зависимостью без аудита.

## Архитектура: топология сервисов

```mermaid
graph LR
    client["Клиент<br/>(Browser)"]
    frontend["frontend<br/>:3000<br/>Next.js + TS"]
    backend["backend<br/>:8080<br/>Rust + Axum + WS"]
    postgres["postgres<br/>:5432<br/>Postgres"]
    redis["redis<br/>:6379<br/>Redis"]
    cow["cow-services<br/>local reference<br/>orderbook / solver flow"]
    fmatch["fmatch<br/>:7277<br/>Rust + Axum"]
    fmatch_db["fmatch-postgres<br/>:5432"]
    typesense["fmatch-typesense<br/>:8108"]
    solvers["Solvers<br/>fiat / token liquidity"]

    client <-->|"HTTP / WS"| frontend
    frontend -->|"REST API"| backend
    backend --> postgres
    backend --> redis
    backend -. изучить / адаптировать .-> cow
    backend <-->|"ActivityPub<br/>inbox / outbox"| fmatch
    backend <-->|"quotes / execution / proof"| solvers
    fmatch --> fmatch_db
    fmatch --> typesense
```

## Архитектура: exchange-flow

```mermaid
graph LR
    intent["User intent<br/>AM/AMD -> RU/RUB"]
    orderbook["backend/orderbook<br/>exchange_orders"]
    fmatch["fmatch<br/>solver discovery"]
    auction["backend/auction<br/>quotes + scoring"]
    solver["solver<br/>TOKEN-leg + money-leg"]
    ledger["mock TOKEN ledger"]
    proof["proof verification<br/>audit trail"]
    frontend["frontend<br/>status + history"]

    intent --> orderbook
    orderbook --> fmatch
    fmatch -->|"candidates, not winner"| auction
    auction -->|"quote requests"| solver
    auction -->|"selected route"| orderbook
    orderbook -->|"funding instruction"| frontend
    frontend -->|"user funding confirmation"| orderbook
    orderbook --> solver
    solver --> ledger
    solver --> proof
    proof --> orderbook
    orderbook --> frontend
```

Эквайеры, старый `transactions/routes` контур и acquiring provider adapters
остаются legacy/fallback rail. Новый MVP строится вокруг `exchange_orders`,
solver candidates, quotes, selected route, funding instruction, mock TOKEN
ledger, money-leg и proof.

## Поток данных: exchange order

```mermaid
sequenceDiagram
    autonumber
    participant К as Клиент
    participant FE as frontend
    participant BE as backend
    participant DB as postgres
    participant FM as fmatch
    participant S as Solver

    К->>FE: Создать перевод Армения → Россия
    FE->>BE: POST /api/exchange/orders
    BE->>DB: Записать exchange_order (status=created)
    BE->>FM: Найти solver candidates
    FM-->>BE: Candidates only
    BE->>S: Запросить quotes
    S-->>BE: Rate, fee, ETA, settlement plan
    BE->>DB: Выбрать route, status=quoted
    FE->>BE: POST /api/exchange/orders/:id/confirm
    BE-->>FE: Funding instruction
    FE->>BE: POST /api/exchange/orders/:id/funding/confirm
    BE->>S: TOKEN-leg + money-leg execution
    S-->>BE: Proof
    BE->>DB: Verify proof, status=done/disputed/failed
    BE-->>К: WebSocket: статус обновлён
```

## Поток данных: кратко

```text
Клиент
  │
  ▼
┌──────────┐  POST /api/exchange/orders  ┌──────────┐
│ frontend │ ───────────────────────────▶│ backend  │
└──────────┘                      └────┬─────┘
                                       │ 1. Записать exchange_order
                                       │ 2. Найти solver candidates
                                       ▼
                                 ┌──────────┐
                                 │  fmatch  │ ← ActivityPub inbox/outbox
                                 └────┬─────┘
                                      │ 3. Вернуть candidates, не winner
                                      │ 4. Backend auction / quotes / winner
                                      ▼
                                 ┌──────────┐
                                 │ backend  │
                                 └────┬─────┘
                                      │ 5. Funding instruction + user consent
                                      │ 6. Mock TOKEN-leg + money-leg + proof
                                      ▼
                                 ┌──────────┐
                                 │ Solver   │ (ликвидность)
                                 └──────────┘
```

## Быстрый старт

```bash
docker compose up -d --build
curl -sS http://localhost:8080/health
```

## Структура

```text
pay3flow/
├── frontend/      # Next.js + TS
├── backend/       # Rust: Axum + WS + ActivityPub + Postgres + Redis
├── cowprotocol-services/ # локальный reference CoW Protocol Services
├── fmatch/        # матчер (отдельный репозиторий)
├── docs/          # документация (API, схемы, глоссарий)
├── scripts/       # утилиты (health-check и т.д.)
├── docker-compose.yml
└── .github/       # CI
```

## Порты

| Сервис | Порт | Назначение |
|--------|------|------------|
| `frontend` | `3000` | Веб-интерфейс |
| `backend` | `8080` | REST API + WebSocket |
| `postgres` | `5435` | PostgreSQL (наш backend) |
| `redis` | `6379` | Кэш quotes/candidates |
| `fmatch` | `7277` | ActivityPub-матчер |
| `fmatch-postgres` | `5433` | PostgreSQL (fmatch) |
| `fmatch-typesense` | `8108` | Поиск (Typesense) |

## Лицензия

Пока не определена.
