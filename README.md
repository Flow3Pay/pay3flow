# Pay3Flow

Трансграничные P2P-платежи с комиссией ниже, чем SWIFT.

## Идея

Получаем платёжный intent пользователя → подбираем P2P-ликвидность и
solver'ов → проводим две локальные денежные ноги через участников сети →
фиксируем статус, комиссии, подтверждения и спорные ситуации.

Целевой поток:

```text
Армения → Pay3Flow → P2P exchange TOKEN → P2P exchange money → Россия
```

Старый контур через эквайеров остаётся как fallback/исторический слой, но
новая целевая архитектура строится вокруг P2P orderbook + solver competition
по модели CoW Protocol.

## Микросервисы

| Сервис | Каталог | Стек | Описание |
|--------|---------|------|----------|
| `frontend` | `frontend/` | Next.js + TypeScript | Веб-интерфейс |
| `backend` | `backend/` | Rust (Axum, WebSocket, Postgres, Redis) | Ядро: auth, intents, P2P-маршрутизация, статусы, споры |
| `cow-services` | `cowprotocol-services/` | Rust | Референс CoW Protocol Services: orderbook, auction/solver flow, settlement patterns |
| `fmatch` | `fmatch/` | Rust (Axum, sqlx) | Федеративный discovery/matching solver'ов и участников сети |
| `crw` | `crw/` | Rust + gRPC | Поиск и discovery новых solver'ов/провайдеров ликвидности |

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
    crw["crw<br/>:3030/:3031<br/>Rust + gRPC"]
    p2p["P2P solvers<br/>fiat / token liquidity"]

    client <-->|"HTTP / WS"| frontend
    frontend -->|"REST API"| backend
    backend --> postgres
    backend --> redis
    backend -. изучить / адаптировать .-> cow
    backend <-->|"ActivityPub<br/>inbox / outbox"| fmatch
    backend <-->|"gRPC discovery"| crw
    backend <-->|"quotes / execution / proof"| p2p
    fmatch --> fmatch_db
    fmatch --> typesense
```

## Поток данных: P2P-перевод

```mermaid
sequenceDiagram
    autonumber
    participant К as Клиент
    participant FE as frontend
    participant BE as backend
    participant DB as postgres
    participant FM as fmatch
    participant OB as P2P orderbook
    participant S as Solver/P2P участник

    К->>FE: Создать перевод Армения → Россия
    FE->>BE: POST /api/payments
    BE->>DB: Записать intent/order (status=pending)
    BE->>OB: Разместить P2P order
    BE->>FM: Найти solver'ов/ликвидность
    FM-->>BE: Кандидаты solver'ов
    BE->>S: Запросить quote / proof / лимиты
    S-->>BE: Цена, комиссия, сроки, условия
    BE->>DB: Зафиксировать выбранный маршрут
    BE->>S: Исполнение локальных ног + token settlement
    S-->>BE: Подтверждения / ошибки / доказательства
    BE->>DB: Обновить статус (done / failed)
    BE-->>К: WebSocket: статус обновлён
```

## Поток данных: кратко

```text
Клиент
  │
  ▼
┌──────────┐  POST /api/payments  ┌──────────┐
│ frontend │ ────────────────────▶│ backend  │
└──────────┘                      └────┬─────┘
                                       │ 1. Записать P2P intent/order
                                       │ 2. Найти solver'ов и ликвидность
                                       ▼
                                 ┌──────────┐
                                 │  fmatch  │ ← ActivityPub inbox/outbox
                                 └────┬─────┘
                                      │ 3. Вернуть кандидатов solver'ов
                                      │ 4. Quote / auction / route selection
                                      ▼
                                 ┌──────────┐
                                 │ backend  │
                                 └────┬─────┘
                                      │ 5. Исполнить P2P fiat/token/money ноги
                                      │ 6. Проверить proofs, обновить статус
                                      ▼
                                 ┌──────────┐
                                 │ Solver   │ (P2P ликвидность)
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
├── crw/           # discovery/search через gRPC
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
| `crw` | `3030 / 3031` | HTTP + gRPC discovery |

## Лицензия

Пока не определена.
