# Pay3Flow

Трансграничные платежи через эквайринг с комиссией ниже, чем SWIFT.

## Идея

Получаем платёжный запрос → ищем подходящего эквайера через `fmatch` →
переводим средства через заранее сохранённые данные эквайера.

## Микросервисы

| Сервис | Каталог | Стек | Описание |
|--------|---------|------|----------|
| `frontend` | `frontend/` | Next.js + TypeScript | Веб-интерфейс |
| `fmatch` | `fmatch/` | Rust (Axum, sqlx) | Матчинг запросов к эквайерам (ActivityPub/FEP-0837) |
| `backend` | `backend/` | Rust (Axum, WebSocket) | Ядро: API, WebSocket |
| `searcher` | `searcher/` | SearXNG (Python) | Постоянный поиск новых эквайеров |

> `fmatch` и `searxng` живут в собственных репозиториях. Исходники SearXNG —
> локальный апстрим для последующей обрезки под `searcher`.

## Быстрый старт

```bash
docker compose up -d --build
curl -sS http://localhost:8080/health
```

## Структура

```text
pay3flow/
├── frontend/      # Next.js + TS
├── backend/       # Rust: Axum + WS + gRPC
├── fmatch/        # матчер (отдельный репозиторий)
├── searcher/      # поиск эквайеров на базе SearXNG
├── searxng/       # локальная копия апстрима (для обрезки)
├── docker-compose.yml
└── .github/       # CI
```

## Лицензия

Пока не определена.