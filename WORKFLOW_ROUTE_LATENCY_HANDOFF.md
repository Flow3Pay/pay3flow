# Pay3Flow / Fmatch: контекст и точка продолжения

Обновлено: 2026-10-01 UTC.

## Главная цель

Асинхронный пайплайн виртуализации маршрутов Pay3Flow должен завершаться не более чем за 1 секунду, даже когда одновременно работает много таких пайплайнов.

Это целевой end-to-end SLO, а не таймаут, после которого незавершённая работа отбрасывается. Результат не должен зависеть по времени от длины цепочки или полного числа теоретически возможных виртуализаций.

Логика пайплайна:

1. Подобрать два обмена узловым методом.
2. Соединить их в маршрут.
3. Локально проверить совместимость и выгодность.
4. Если маршрут не подходит — перейти к следующей перспективной паре.
5. Вернуть нужное число подходящих маршрутов либо корректный пустой результат.

Fmatch должен оставаться универсальным сервисом поиска и раздачи предложений. Нельзя встраивать в него Pay3Flow-специфичные эвристики. Fmatch также не является биржей и не должен попадать в пользовательские результаты как venue/exchange.

## Репозитории и ветки

- Pay3Flow: `/home/lion/workspaces/pay3flow`
  - рабочая ветка: `codex/workflow-route-latency`
  - база ветки: `b8d8d3b660c806ef15ffbdde0b93ef38b2811b9a`;
  - первый этап ускорения: `2068a9e98da98c3ce4e3f0bf2068f4b0a5a72587`.
- Fmatch, основной checkout: `/home/lion/workspaces/lefinepro/fmatch`
- Fmatch, рабочий worktree: `/home/lion/workspaces/fmatch-pay3flow-routing`
  - ветка: `codex/pay3flow-routing-performance`
  - HEAD: `848e0c59b41d28a1e4654db5a1803c614de81a51`
  - worktree был чистым.

В текущей ветке Pay3Flow реализуется секундная архитектура. Код второго этапа находится поверх `2068a9e`: immutable capability snapshot, фоновые quote refresh и немедленная виртуализация workflow без live provider fan-out на критическом пути.

## Что уже исправлено и выложено

### Fmatch

Коммит `848e0c59b41d28a1e4654db5a1803c614de81a51` (`perf: restore broad low-latency discovery`):

- динамический размер страницы кандидатов с пределом 64;
- универсальные facet-фильтры `name=value`, разделённые `;`;
- значения `all`, `any`, `none`, `null` и wildcard;
- фильтрация до усечения результата;
- bounded scan с разнообразием провайдеров;
- сохранение attachments.

Проверки: 138 тестов и 2 benchmark прошли, 1 тест ignored. Release-бенчмарк in-process на 100 000 offers и 64 candidates: p50 391 мкс, p95 445 мкс, p99 481 мкс.

В production развёрнут образ `fmatch:source-6e3dee72e6fe`, pod здоров. Ветка `master` в Crada указывает на `848e0c5`; старый remote `source.lefine.pro` недоступен, локальный `master` теперь отслеживает `crada/master`.

Health: `http://195.123.218.200:30727/health` возвращал `{"status":"ok"}`. Production histogram discovery: 373 запроса, все попали в bucket до 50 мс, суммарное время 1.517145 с — среднее около 4 мс. Ядро discovery Fmatch не объясняет задержку 30–50 секунд.

### Pay3Flow

- `a6faa2a` — `fix: restore efficient Fmatch route discovery`.
- `b8d8d3b660c806ef15ffbdde0b93ef38b2811b9a` — `fix: keep Fmatch out of venue results`.

В `b8d8d3b`:

- `SourceStatus` строится из настоящего `offer.source`;
- stale/fallback больше не добавляет Fmatch в список venue;
- внутренняя metadata discovery хранится отдельно;
- frontend дополнительно фильтрует внутренние маркеры.

Production-проверка AMD → RUB: 3766 маршрутов, в venue были Whitebird, Bybit, BestChange и другие реальные источники, `has_fmatch_venue: false`.

Проверки: backend — 202 passed, 14 ignored; Providerfile — 16 passed; frontend check/build успешны. В production работает образ `live-20261001-b8d8d3b`, pods здоровы, рестартов нет. Основные ветки Pay3Flow и Fmatch уже запушены.

### Реализация секундного pipeline в Pay3Flow

Коммит `2068a9e` и следующий этап в рабочей ветке:

- fiat entry × exit строится lazy best-first и ограничивается требуемым top-K вместо полного декартова произведения; bounded diversity seeds сохраняют лучший валидный маршрут каждого venue, даже если он ниже плотной P2P-выдачи;
- API явно сообщает `routes_exhaustive`, поэтому ограниченный поиск не маскируется под полный подсчёт;
- capability провайдеров загружаются один раз в immutable snapshot при старте;
- live provider quotes и direct-fiat quotes обновляют 30-секундные кэши только фоновыми bounded-задачами;
- cache miss немедленно даёт capability-based workflow estimate с явным требованием перепроверить quote перед исполнением;
- фоновые обновления дедуплицируются, берут semaphore через `try_acquire` и не создают скрытую очередь ожидания;
- повторные USDT-запросы для каждой сети сведены к одному запросу на символ;
- режим `all` запрашивает у Fmatch разделы `p2p` и `direct_exchange` параллельно и объединяет их с source diversity, поэтому редкие direct-источники вроде Whitebird не вытесняются плотной P2P-выдачей;
- свежие partitioned Fmatch-ответы переиспользуются из короткого 5-секундного memory cache, поэтому workflow-фаза не повторяет те же сетевые leg-запросы;
- Symbiosis корректно усекает вход до precision токена;
- добавлены structured phase timings и тест 100 параллельных больших виртуализаций с пределом 1 секунда.

Локальная проверка второго этапа: backend `206 passed, 14 ignored`, frontend `0 errors, 0 warnings`, `git diff --check` чистый. Production p95/p99 нужно вписать ниже после деплоя.

## Воспроизводимые production-замеры

Основной URL:

```text
https://pay3flow.lefine.pro/api/p2p/routes?source_fiat=AMD&target_fiat=RUB&source_amount=100000&intermediary_assets=USDT&exchange_mode=all&allow_cross_venue=true&limit=40
```

Результаты двух запусков:

- 52.6985 с в backend log;
- 49.3399 с total, 49.2718 с до первого байта, response 154 807 bytes;
- `routes_found: 3692`, видимых routes: 40;
- workflow/provider routes: 32, в том числе BestChange и Symbiosis.

P2P-only с теми же параметрами и `exchange_mode=p2p`:

- 3.8792 с total, 3.8074 с до первого байта;
- `routes_found: 2460`, видимых routes: 40;
- провайдеры Binance, Bitget, Bybit, MEXC, OKX;
- response source `fmatch`, `stale: false`.

Отдельные вызовы `/api/p2p/search` заняли 6.68 с и 5.14 с, но они намеренно обходят Fmatch и возвращали `source=provider`; их нельзя использовать как измерение route path.

Запуск websocket-бенчмарка был прерван пользователем через 14.6 с. Изменений файлов или кода он не оставил; его следует повторить после продолжения работы.

## Найденная причина задержки

Основные 49–52 секунды возникают после Fmatch внутри Pay3Flow: синхронный fan-out live quote-запросов к внешним провайдерам находится на критическом пути построения workflow routes.

В `backend/src/p2p/routes.rs` используются пределы:

```rust
MAX_PROVIDER_ASSETS = 12
MAX_PROVIDER_NETWORK_PAIRS = 32
MAX_PROVIDER_OFFERS_PER_LEG = 8
```

В `backend/src/p2p/service.rs` quote semaphore ограничен `MAX_CONCURRENT_PROVIDER_QUOTES = 16`. `quote_provider` и `quote_provider_many` сначала неограниченно ждут semaphore, а 12-секундный timeout начинается лишь вокруг внешнего вызова после получения permit. Сотни задач проходят очередями, поэтому суммарное ожидание запроса не ограничено.

`search_fiat_provider_routes` перебирает entry offers × providers × network pairs, затем соединяет каждый quote с exit offers. В итоге рассчитываются тысячи маршрутов, хотя клиент просит только 40.

Production logs подтверждают quote flood и rate limiting:

- BestChange: HTTP 429;
- NEAR Intents: `ThrottlerException: Too Many Requests`;
- Symbiosis: `limit of requests exceeded`;
- множество запросов Symbiosis сразу отклоняются с `amount has more precision than the token supports`.

У Symbiosis есть отдельная ошибка нормализации: `backend/providers/symbiosis/symbiosis.rs` передаёт `Amount::from_f64` прямо в `decimal_to_atomic`, хотя число может иметь больше знаков, чем decimals токена. Перед конвертацией нужно применять уже существующий `truncate_decimal`, а возвращаемый quote input должен отражать фактически усечённое значение.

Дополнительные проблемы:

- `provider_assets_for` и `provider_capabilities` последовательно и повторно загружают supported assets для одного поиска;
- `compose_fiat_routes` материализует все entry × exit комбинации, вызывает `merge_routes`, сортирует и клонирует весь набор, а лишь затем обрезает response до limit;
- точный `routes_found` сейчас требует полного перебора всех валидных комбинаций;
- даже P2P-only путь занимает 3.9 с, поэтому устранить только provider fan-out недостаточно: нужно измерить Fmatch roundtrip, локальную генерацию, reputation/DB, сортировку и сериализацию отдельно.

## Требуемая архитектура

Просто поставить timeout на 1–3 секунды и отбросить незавершённую работу недостаточно: это не выполнит поставленную цель.

Критический search/virtualization path должен работать только с актуальным immutable snapshot предложений, возможностей, сетей и курсов, уже доступным локально или полученным из Fmatch:

1. Лениво выбрать следующую наиболее перспективную пару узлов/обменов.
2. Соединить её в маршрут.
3. Локально проверить amount bounds, payment method, network compatibility, доходность и target amount.
4. Если пара не подходит — продвинуть frontier.
5. Остановиться после нахождения запрошенного числа маршрутов, не строя весь декартов продукт.

Live quote-запросы внешним провайдерам должны обновлять snapshot независимо в фоне. Проверка актуальной котировки непосредственно перед исполнением — отдельный execution phase, а не часть поиска маршрутов.

Для entry × exit подходит lazy top-K / best-first Cartesian traversal:

- entry offers отсортировать по наиболее дешёвой покупке;
- exit offers — по наиболее выгодной продаже;
- хранить heap/frontier и множество уже посещённых пар;
- проверять по одной перспективной паре;
- при необходимости давать bounded provider-diverse seeds;
- сложность должна зависеть от запрошенного K (`limit <= 100`), а не от полного числа комбинаций.

Для cross-chain/provider routes нужен background snapshot quote edges либо разделение search-time discovery и execution-time live confirmation. Нельзя синхронно отправлять сотни provider-запросов на один пользовательский search.

Семантику `routes_found` придётся определить явно: точное полное число несовместимо с независимостью от размера пространства поиска. Допустимые варианты — число реально найденных/evaluated snapshot routes или отдельный признак `at_least`/estimated. Нельзя молча возвращать неточное значение как точное.

## План продолжения

1. Добавить structured timings/metrics по фазам: Fmatch roundtrip, загрузка локальных данных/reputation, pair generation, validation, provider enrichment, sorting/serialization и общий end-to-end p50/p95/p99.
2. Разложить текущие 3.879 с P2P-only запроса по фазам — без этого нельзя подтвердить секундный SLO после отключения live enrichment.
3. Переписать fiat pair virtualization на lazy top-K, не создавая 2000–4000 объектов ради ответа из 40 маршрутов.
4. Убрать synchronous external provider enrichment с критического search path; использовать immutable cache/snapshot и background refresh через channel/task.
5. Исправить precision Symbiosis через `truncate_decimal` и добавить unit test.
6. Убрать повторные последовательные загрузки provider capabilities/assets: snapshot/cache, а где необходимо — корректная конкурентная загрузка.
7. Добавить async benchmark/regression test на большой набор offers и не менее 100 параллельных pipelines; целевой p99 — меньше 1 секунды. Отдельно проверить, что critical virtualization path не ждёт внешние provider calls.
8. Решить и задокументировать новую семантику `routes_found`.
9. Запустить полный backend test suite, Providerfile generate/check, frontend check/build и сравнить benchmark до/после.
10. После деплоя повторить точно те же production URL, измерить HTTP и websocket first/final result, проверить исчезновение quote flood/429. Менять Fmatch только при новых доказательствах проблемы именно в нём.

## Ограничения и правила продолжения

- Не добавлять Fmatch как биржу/обменник ни в API, ни в UI.
- Не подстраивать универсальные алгоритмы Fmatch под Pay3Flow.
- Не выдавать timeout/cutoff за выполнение секундного SLO.
- Не жертвовать качеством подбора ради произвольного малого candidate cap: искать top-K лениво и прекращать работу после достаточного результата.
- Сохранять корректность async cancellation и не выполнять блокирующую/сетевую работу на критическом пути виртуализации.
- Перед изменениями Fmatch перечитать его `AGENTS.md`, архитектуру, protocol docs и ADR, относящиеся к меняемой семантике; соблюдать ограничение Fmatch в 300 строк на `.rs` файл.

## Инфраструктура для проверки

- Pay3Flow production: `https://pay3flow.lefine.pro`, host `82.118.18.183`, namespace `pay3flow`.
- Fmatch production: host `195.123.218.200`, namespace `lefine`, health NodePort `30727`.

Секреты, токены и пути к ключам в этом документе намеренно не сохранены.
