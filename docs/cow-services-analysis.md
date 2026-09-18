# Cow Protocol Services Analysis

Дата: 2026-09-18.

Цель документа: зафиксировать, какие идеи из локального
`cowprotocol-services/` стоит переносить в Pay3Flow, а какие остаются только
reference. Cow не заменяет `fmatch`: в Pay3Flow `fmatch` остается matcher'ом
solver candidates, а backend хранит orders, собирает quotes, выбирает winner и
ведет settlement/proof lifecycle.

## Что было изучено

Локальный reference:

- `cowprotocol-services/README.md`
- `cowprotocol-services/docs/ONBOARDING.md`
- `cowprotocol-services/crates/orderbook/src/api.rs`
- `cowprotocol-services/crates/orderbook/src/api/post_order.rs`
- `cowprotocol-services/crates/orderbook/src/api/post_quote.rs`
- `cowprotocol-services/crates/orderbook/openapi.yml`
- `cowprotocol-services/crates/autopilot/src/run_loop.rs`
- `cowprotocol-services/crates/autopilot/src/solvable_orders.rs`
- `cowprotocol-services/crates/autopilot/openapi.yml`
- `cowprotocol-services/crates/driver/README.md`
- `cowprotocol-services/crates/driver/src/domain/quote.rs`
- `cowprotocol-services/crates/driver/src/run.rs`
- `cowprotocol-services/crates/driver/openapi.yml`

Внешний UX/API reference:

- Matcha UI: https://matcha.xyz/
- 0x Docs: https://docs.0x.org/docs/introduction/welcome

## Сборка Cow Локально

Команда, которую нужно использовать для быстрой проверки ключевых crates:

```bash
cd cowprotocol-services
cargo check -p orderbook -p autopilot -p driver --all-targets
```

Результат на текущей машине:

```text
zsh:1: command not found: cargo
```

Вывод: локальная сборка Cow не была выполнена из-за отсутствия `cargo` в
окружении. После установки Rust toolchain стоит повторить команду выше. Для
полного playground-flow Cow README рекомендует:

```bash
cd cowprotocol-services
docker compose -f playground/docker-compose.fork.yml up --build
```

Этот путь требует настроенного `ETH_RPC_URL`, потому для Pay3Flow сейчас
достаточно source-level анализа.

## Orderbook

Главная роль Cow `orderbook`:

- принимает пользовательские orders через HTTP;
- валидирует подписи, app-data, funding/allowance, сроки и token constraints;
- пишет orders и lifecycle data в Postgres;
- отдает пользователю order/status/history;
- отдает solvers/autopilot данные для auction.

Полезные endpoints из `crates/orderbook/openapi.yml`:

- `POST /api/v1/orders` - создание order;
- `GET /api/v1/orders/{UID}` - чтение order;
- `GET /api/v1/orders/{UID}/status` - чтение статуса;
- `GET /api/v1/account/{owner}/orders` - история пользователя;
- `POST /api/v1/quote` и `POST /api/v1/quote/stream` - расчет quote перед order;
- `GET /api/v1/auction` - текущий auction snapshot;
- `GET /api/v2/solver_competition/...` - просмотр solver competition data.

Что переносим в Pay3Flow:

- отдельный orderbook API для exchange orders вместо старого
  `POST /api/payments`;
- typed request/response DTO;
- единый error mapping для validation ошибок;
- запись order lifecycle в БД;
- отдельные endpoints для order, status, quotes, history;
- observability на уровне HTTP route + status + elapsed;
- idempotent order creation.

Что не переносим:

- Ethereum signatures как обязательную модель order;
- ERC20 allowance/funding checks как источник истины;
- on-chain token validation;
- CoW order UID как доменную модель.

Pay3Flow adaptation:

- `exchange_orders` хранят intent: country/currency/method/amount/deadline;
- source/target country и currency всегда разные поля;
- все суммы в minor units;
- idempotency через `(user_id, idempotency_key)`;
- validation сначала простая: amount > 0, ISO-like country/currency,
  enabled corridor, sane deadline.

## Autopilot / Auction

Главная роль Cow `autopilot`:

- периодически строит auction из eligible orders;
- фильтрует неисполняемые orders;
- готовит данные для solver competition;
- собирает bids/solutions;
- ранжирует решения;
- говорит победителю исполнять settlement;
- индексирует on-chain events после settlement.

Важные идеи из `run_loop.rs` и `solvable_orders.rs`:

- auction строится не мгновенно на каждый order, а по окну/событию;
- есть cache solvable orders;
- фильтрация причин неисполнения явная и наблюдаемая;
- winner selection отделен от solver discovery;
- in-flight orders не должны повторно попадать в новые auctions;
- competition metadata полезна для debug/audit.

Что переносим в Pay3Flow:

- auction window 3 секунды для сбора quotes;
- явную фильтрацию quotes/candidates с причинами;
- детерминированный scoring;
- стабильные tie-break rules;
- запрет менять winner после `locked`;
- audit events для selected winner и rejected quotes.

Что не переносим:

- block-based run loop;
- EVM settlement deadlines;
- on-chain event indexing;
- calldata/simulation model.

Pay3Flow adaptation:

- auction запускается для одного exchange order или небольшого batch позже;
- `fmatch` возвращает candidates, но winner выбирает backend;
- если `fmatch` недоступен: Redis cache -> local solver registry -> failed;
- MVP scoring:
  `target_amount_minor - fee_minor - eta_minutes * 10 - risk_score * 100 -
  manual_review_penalty`.

## Driver / Solver

Главная роль Cow `driver`:

- принимает auction/quote request;
- вызывает solver engine;
- выбирает или конвертирует solution в quote;
- кодирует settlement;
- публикует settlement, если solution победила.

В `driver/src/domain/quote.rs` quote строится через synthetic single-order
auction: driver вызывает solver, получает solutions и превращает выбранное
solution в quote. Это полезная архитектурная граница: quote source не обязан
быть тем же компонентом, который хранит orderbook.

Что переносим в Pay3Flow:

- trait/abstraction для источников quotes;
- fake solver'ы как первые implementations;
- разделение:
  `solver discovery` -> `quote collection` -> `winner selection` -> `settlement`;
- timeout/reject не валит весь auction;
- solver-specific raw response хранится отдельно от normalized quote.

Что не переносим:

- EVM transaction simulation;
- calldata encoding;
- settlement contract buffers;
- fast-path on-chain exclusivity.

Pay3Flow adaptation:

- добавить `RouteQuoteSource`:
  `name()`, `health()`, `quote(request)`;
- MVP implementation: `MockRouteQuoteSource`;
- позже external aggregators могут стать adapters для TOKEN-leg quote/liquidity,
  но они не заменяют `fmatch` и не становятся core backend.

## Status / Lifecycle Mapping

Cow lifecycle больше завязан на signed orders, auctions и on-chain settlement.
Pay3Flow lifecycle должен быть fiat/token exchange specific:

```text
created -> discovering -> quoting -> quoted -> locked -> token_settling
-> money_settling -> proof_pending -> done
```

Failure/dispute branches:

```text
discovering -> failed
quoting -> failed | expired
quoted -> expired | cancelled
token_settling -> disputed | failed
money_settling -> disputed | failed
proof_pending -> disputed | failed
disputed -> done | failed
```

Обязательное правило для EX-2: все transitions делаются guarded update через
текущий status. Terminal statuses не меняются без admin/manual resolution.

## Matcha / Meta Matcha UX Reference

Matcha useful pattern:

- пользователь начинает с простой формы swap/search;
- сложная маршрутизация скрыта за понятной ценой, сетью, route details и
  предупреждениями;
- route/liquidity aggregation воспринимается как backend capability, а не как
  ручной выбор провайдера пользователем;
- токены, networks, liquidity score и security warnings показываются как
  decision support, но не перегружают основной intent.

0x useful API pattern:

- единая trading API family для swap/gasless/cross-chain;
- route/liquidity source может быть adapter за Pay3Flow solver или
  `RouteQuoteSource`;
- API/provider dependency должна быть optional и env-gated.

Что переносим в Pay3Flow frontend:

- первый экран должен быть рабочим кабинетом обмена, не landing;
- intent-form: sell/source -> receive/target;
- показывать amount, rate, fee, ETA, selected route и funding instruction;
- TOKEN/crypto settlement раскрывать в terms/consent/details;
- currencies/corridors тянуть из backend, не хардкодить на frontend.

## Решения Для Pay3Flow

1. Cow остается локальным reference, не runtime dependency.
2. `fmatch` остается matcher'ом solver candidates.
3. Backend выбирает winner и хранит audit trail.
4. EX-2 начинается с typed domain models, migrations и transition tests.
5. Все реальные money/token integrations остаются mock/stub до отдельного
   legal/compliance решения.
6. Первый corridor живет в seed/config/table: `AM/AMD -> RU/RUB`.
7. Следующий coding target: `exchange_orders`, `exchange_solvers`,
   `exchange_quotes`, `exchange_settlements`, `exchange_proofs`,
   `funding_instructions`, `audit_events`, `exchange_corridors`.

## Файлы Cow Для Повторного Просмотра

При реализации EX-2/EX-3:

- `crates/orderbook/src/api.rs`
- `crates/orderbook/src/api/post_order.rs`
- `crates/orderbook/src/api/get_order_status.rs`
- `crates/orderbook/src/database/orders.rs`
- `crates/orderbook/src/database/quotes.rs`

При реализации EX-5/EX-6:

- `crates/autopilot/src/run_loop.rs`
- `crates/autopilot/src/solvable_orders.rs`
- `crates/driver/src/domain/quote.rs`
- `crates/driver/README.md`

При реализации observability/debug:

- `crates/orderbook/src/api/get_solver_competition_v2.rs`
- `crates/orderbook/src/database/solver_competition_v2.rs`
- `crates/autopilot/src/database/order_events.rs`
