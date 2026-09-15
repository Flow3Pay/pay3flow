# Pay3Flow

Трансграничные платежи через эквайринг с комиссией ниже, чем SWIFT.

## Идея

Получаем платёжный запрос → ищем подходящего эквайера через `fmatch` →
переводим средства через заранее сохранённые данные эквайера.

## Микросервисы

| Сервис | Каталог | Стек | Описание |
|--------|---------|------|----------|
| `frontend` | `frontend/` | Next.js + TypeScript | Веб-интерфейс |
| `backend` | `backend/` | Rust (Axum, WebSocket, Postgres) | Ядро: auth, платежи, маршрутизация, ActivityPub |
| `fmatch` | `fmatch/` | Rust (Axum, sqlx) | Матчинг запросов к эквайерам (ActivityPub/FEP-0837) |
| `searcher` | `searcher/` | SearXNG → Rust | Поиск новых эквайеров в интернете |

> `fmatch` и `searxng` живут в собственных репозиториях. Исходники SearXNG —
> локальный апстрим для последующей обрезки под `searcher`.

## Архитектура: топология сервисов

```mermaid
graph LR
    client["Клиент<br/>(Browser)"]
    frontend["frontend<br/>:3000<br/>Next.js + TS"]
    backend["backend<br/>:8080<br/>Rust + Axum + WS"]
    postgres["postgres<br/>:5432<br/>Postgres"]
    fmatch["fmatch<br/>:7277<br/>Rust + Axum"]
    fmatch_db["fmatch-postgres<br/>:5432"]
    typesense["fmatch-typesense<br/>:8108"]
    searcher["searcher<br/>:8081<br/>SearXNG → Rust"]

    client <-->|"HTTP / WS"| frontend
    frontend -->|"REST API"| backend
    backend --> postgres
    backend <-->|"ActivityPub<br/>inbox / outbox"| fmatch
    fmatch --> fmatch_db
    fmatch --> typesense
```

## Поток данных: обработка платежа

```mermaid
sequenceDiagram
    autonumber
    participant К as Клиент
    participant FE as frontend
    participant BE as backend
    participant DB as postgres
    participant FM as fmatch
    participant EQ as Эквайер

    К->>FE: Создать платёж
    FE->>BE: POST /api/payments
    BE->>DB: Записать транзакцию (status=pending)
    BE->>FM: Proposal (request) via ActivityPub
    FM->>FM: Поиск кандидатов-эквайеров
    FM-->>BE: Offer(Agreement) + candidates
    BE->>DB: Обновить маршрут (status=matched)
    BE->>EQ: Выполнить платёж через эквайера
    EQ-->>BE: Статус операции
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
                                       │ 1. Записать транзакцию (pending)
                                       │ 2. Отправить Proposal в fmatch
                                       ▼
                                 ┌──────────┐
                                 │  fmatch  │ ← ActivityPub inbox/outbox
                                 └────┬─────┘
                                      │ 3. Найти кандидатов-эквайеров
                                      │ 4. Отправить Offer(Agreement) + candidates
                                      ▼
                                 ┌──────────┐
                                 │ backend  │
                                 └────┬─────┘
                                      │ 5. Исполнить платёж через эквайера
                                      │ 6. Обновить статус
                                      ▼
                                 ┌──────────┐
                                 │ Эквайер  │ (реальный провайдер)
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
├── backend/       # Rust: Axum + WS + ActivityPub + Postgres
├── fmatch/        # матчер (отдельный репозиторий)
├── searcher/      # поиск эквайеров (SearXNG → Rust)
├── searxng/       # локальная копия апстрима (для обрезки)
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
| `fmatch` | `7277` | ActivityPub-матчер |
| `fmatch-postgres` | `5433` | PostgreSQL (fmatch) |
| `fmatch-typesense` | `8108` | Поиск (Typesense) |
| `searcher` | `8081` | Поиск эквайеров (SearXNG) |

## Лицензия

Пока не определена.