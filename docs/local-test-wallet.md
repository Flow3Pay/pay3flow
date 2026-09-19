# Local test crypto wallet

Самодостаточное локальное тестовое окружение для self-hosted EVM wallet
(`docs/self-hosted-wallet.md`). Без реальных средств, без интернета после
первого pull образа Foundry, полностью сбрасываемое.

Что поднимается:

| Компонент | Что это |
|-----------|---------|
| `anvil` (docker compose service, порт `8545`) | локальный тестовый EVM-узел, chain-id `31337`, состояние в памяти |
| `data/wallet/UTC--*.json` | зашифрованный keystore кошелька (алгоритм из `backend/src/bin/wallet-init.rs`) |
| Mock ERC-20 `USDT`, `USDC`, `TOKEN` | контракты `contracts/mock/MockERC20.sol`, mint по 1 000 000 в кошелёк |
| `data/wallet/setup.env` | все значения для подключения backend |

## Проверить это за 2 минуты

```bash
# 1. развернуть узел + кошелёк + токены
./scripts/wallet-setup.sh            # Windows: .\scripts\wallet-setup.ps1

# 2. подключить backend к кошельку (docker)
#    значения берутся из data/wallet/setup.env
export WALLET_KEYSTORE_PATH="$(grep ^WALLET_KEYSTORE_PATH_DOCKER= data/wallet/setup.env | cut -d= -f2)"
export WALLET_KEYSTORE_PASSWORD='dev-local-test-password'
export WALLET_RPC_URL='http://anvil:8545'          # anvil — имя compose-сервиса
docker compose up -d backend

# 3. прогнать API-смоук (info → balance → native transfer → token transfer)
./scripts/wallet-smoke.sh            # Windows: .\scripts\wallet-smoke.ps1
```

`docker compose up -d` без шага (2) просто не активирует wallet-роуты
(`/api/admin/wallet*` отвечают 501), всё остальное работает как раньше.

## wallet-setup

`scripts/wallet-setup.ps1` / `.sh` — идемпотентный provisioning:

1. `docker compose up -d anvil` — поднимает тестовый узел (первый запуск тянет
   образ `ghcr.io/foundry-rs/foundry:stable`, ~2 ГБ).
2. Создаёт keystore через `cargo run --bin wallet-init -- data/wallet`, если его ещё нет.
3. Фандит кошелёк тестовым ETH из anvil dev-аккаунта #0 (по умолчанию 50 ETH).
4. Деплоит `USDT` (6 decimals), `USDC` (6), `TOKEN` (18) через `forge create`
   внутри контейнера (`cast`/`forge` входят в образ). Адреса переиспользуются,
   пока узел жив; после рестарта anvil контракты разворачиваются заново.
5. Минтит по 1 000 000 каждого токена в кошелёк.
6. Пишет `data/wallet/setup.env`:

```text
WALLET_KEYSTORE_PATH=C:\...\data\wallet\<uuid>
WALLET_KEYSTORE_PATH_DOCKER=/app/wallet/<uuid>     # тот же файл внутри контейнера
WALLET_KEYSTORE_PASSWORD=dev-local-test-password
WALLET_RPC_URL=http://localhost:8545               # для локального cargo run
WALLET_RPC_URL_DOCKER=http://anvil:8545            # для контейнера backend
WALLET_ADDRESS=0x...
ANVIL_BLOCKS=<последний номер блока>               # защита от переиспользования адресов
USDT_CONTRACT=0x...
USDC_CONTRACT=0x...
TOKEN_CONTRACT=0x...
RECIPIENT=0x...
```

Переопределить поведение: `WALLET_KEYSTORE_PASSWORD`, `RPC_URL`,
`ANVIL_DEV_KEY`, `WALLET_KYSTORE_DIR`, `WALLET_NATIVE_FUND_ETH` (env-переменные)
или параметры `-Password`, `-DevKey`, `-Recipient`, `-NativeFundEth` (ps1).

> Keystore-файл в alloy V3 называется просто `<uuid>` (без префикса `UTC--`) и
> не содержит поля `address` — скрипт достаёт адрес дешифровкой через
> `cast wallet` внутри контейнера.
>
> `ANVIL_BLOCKS` защищает reuse-логику: после `docker compose restart anvil`
> цепь обнуляется, и адреса из прошлого `setup.env` становятся мусором.
> Если текущий номер блока меньше записанного, скрипт игнорирует старые
> адреса и разворачивает токены заново.

## wallet-smoke

`scripts/wallet-smoke.ps1` / `.sh` — полный путь через админ-API backend
(все руты требуют `Authorization: Bearer $ADMIN_TOKEN`):

```text
GET  /api/admin/wallet               → адрес совпадает с setup.env
GET  /api/admin/wallet/balance       → native wei > 0
POST /api/admin/wallet/transfer      → 0.001 ETH → RECIPIENT, tx_hash
POST /api/admin/wallet/token-transfer→ 1.0 USDT  → RECIPIENT, tx_hash
cast call balanceOf(RECIPIENT)       → on-chain баланс USDT вырос
```

## Запуск локально, без docker (cargo run)

```bash
WALLET_KEYSTORE_PATH="$PWD/data/wallet/<uuid>" \
WALLET_KEYSTORE_PASSWORD='dev-local-test-password' \
WALLET_RPC_URL='http://127.0.0.1:8545' \
cargo run --bin pay3flow-backend
```

Узел при этом всё равно нужен: `docker compose up -d anvil`.

## Ручной smoke через curl

```bash
export ADMIN_TOKEN='dev-admin-token-change-me'
ADDR=$(grep ^WALLET_ADDRESS= data/wallet/setup.env | cut -d= -f2)
USDT=$(grep ^USDT_CONTRACT= data/wallet/setup.env | cut -d= -f2)
RECV=$(grep ^RECIPIENT=     data/wallet/setup.env | cut -d= -f2)

curl -s http://localhost:8080/api/admin/wallet -H "Authorization: Bearer $ADMIN_TOKEN"
curl -s http://localhost:8080/api/admin/wallet/balance -H "Authorization: Bearer $ADMIN_TOKEN"
curl -s -X POST http://localhost:8080/api/admin/wallet/transfer \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' \
  -d "{\"to\":\"$RECV\",\"value_wei\":\"1000000000000000\"}"
curl -s -X POST http://localhost:8080/api/admin/wallet/token-transfer \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' \
  -d "{\"token_contract\":\"$USDT\",\"to\":\"$RECV\",\"amount\":\"1000000\"}"
```

## Перезапуск / сброс

```bash
docker compose restart anvil     # сбросить цепочку (состояние в памяти)
docker compose up -d anvil       # повторный provision заново развернёт токены
./scripts/wallet-setup.sh        # снова (ANVIL_BLOCKS сам заметит сброс)
```

Полный сброс к чистому состоянию: `docker compose rm -sf anvil` (или
`--force-recreate anvil`), затем `./scripts/wallet-setup.sh`.

## Безопасность

- Все средства тестовые, цепочка в памяти, ничего не тянется из мейннета.
- Ключ anvil dev-аккаунта — публичный тестовый ключ Foundry, используется
  только provisioning.
- Keystore зашифрован; приватный ключ не покидает хост. `data/` в `.gitignore`.
- Для production-выплат signer выносится на отдельный хост/HSM
  (см. `docs/self-hosted-wallet.md`).