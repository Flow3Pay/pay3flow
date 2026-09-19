#!/usr/bin/env bash
# Exercise the self-hosted wallet HTTP API end-to-end against the local test
# node: info, balance, native transfer, ERC-20 transfer. Uses the values the
# provisioning script wrote into data/wallet/setup.env.
#
# Prerequisites:
#   1. scripts/wallet-setup.sh ran successfully
#   2. the backend was (re)created with WALLET_KEYSTORE_PATH/PASSWORD/RPC set
#      (docs/local-test-wallet.md)
#
# Usage:  scripts/wallet-smoke.sh
# Env:    BACKEND_URL (default http://localhost:8080)
#         ADMIN_TOKEN (default dev-admin-token-change-me)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
compose_file="$root/docker-compose.yml"
backend="${BACKEND_URL:-http://localhost:8080}"
admin_token="${ADMIN_TOKEN:-dev-admin-token-change-me}"
keystore_dir="${WALLET_KEYSTORE_DIR:-$root/data/wallet}"
env_file="$keystore_dir/setup.env"

[ -f "$env_file" ] || {
  echo "run scripts/wallet-setup.sh first (no $env_file)" >&2
  exit 2
}
# The env file hosts only test values; sourcing it is fine.
set -a; source "$env_file"; set +a

wallet_addr="${WALLET_ADDRESS:?missing WALLET_ADDRESS in setup.env}"
usdt="${USDT_CONTRACT:?missing USDT_CONTRACT in setup.env}"
recipient="${RECIPIENT:?missing RECIPIENT in setup.env}"
native_amount_wei="1000000000000000" # 0.001 ETH
token_amount_raw="1000000"           # 1.0 USDT (6 decimals)

need() { command -v "$1" >/dev/null || { echo "missing required command: $1" >&2; exit 2; }; }
need curl; need jq

request() {
  local method="$1" path="$2" body="${3:-}"
  local args=(-fsS -w $'\n%{http_code}' -X "$method" "$backend$path" \
    -H 'Accept: application/json' -H "Authorization: Bearer $admin_token")
  [[ -n "$body" ]] && args+=(-H 'Content-Type: application/json' --data "$body")
  local raw
  raw="$(curl "${args[@]}" 2>&1)"
  local code="${raw##*$'\n'}"
  local json="${raw%"$code"}"
  printf '%s\n' "$json"
  [[ "$code" =~ ^2[0-9][0-9]$ ]] || {
    echo "http $code on $method $path" >&2
    return 1
  }
}

expect_eq() {
  [[ "$1" == "$2" ]] || { echo "assertion failed: expected '$2', got '$1' (${3:-})" >&2; exit 1; }
}

cast_in_node() { docker compose -f "$compose_file" exec -T anvil cast "$@"; }

echo "[1/5] backend health"
expect_eq "$(curl -fsS "$backend/health")" "ok" "health"

echo "[2/5] GET /api/admin/wallet"
info="$(request GET /api/admin/wallet)"
expect_eq "$(printf '%s\n' "$info" | jq -er .address)" "$wallet_addr" "wallet address"
require_ok=$(printf '%s\n' "$info" | jq -er .rpc_url)

echo "[3/5] GET /api/admin/wallet/balance"
balance="$(request GET /api/admin/wallet/balance)"
expect_eq "$(printf '%s\n' "$balance" | jq -er .address)" "$wallet_addr" "balance address"
printf '%s\n' "$balance" | jq -e '.wei | tonumber > 0' >/dev/null

echo "[4/5] POST /api/admin/wallet/transfer (native 0.001 ETH)"
native_resp="$(request POST /api/admin/wallet/transfer \
  "$(jq -nc --arg to "$recipient" --arg v "$native_amount_wei" '{to:$to,value_wei:$v}')")"
tx_native="$(printf '%s\n' "$native_resp" | jq -er .tx_hash)"
expect_eq "$(printf '%s\n' "$native_resp" | jq -r .to)" "$recipient" "recipient"
[ -n "$tx_native" ]

echo "[5/5] POST /api/admin/wallet/token-transfer (1.0 USDT)"
token_resp="$(request POST /api/admin/wallet/token-transfer \
  "$(jq -nc --arg tok "$usdt" --arg to "$recipient" --arg a "$token_amount_raw" \
    '{token_contract:$tok,to:$to,amount:$a}')")"
tx_token="$(printf '%s\n' "$token_resp" | jq -er .tx_hash)"

# On-chain proof: the recipient holds at least 1e6 raw USDT.
usdt_of_recipient="$(cast_in_node call --rpc-url "${WALLET_RPC_URL_DOCKER:-http://localhost:8545}" \
  "$usdt" "balanceOf(address)(uint256)" "$recipient" | awk '{print $1}')"
echo "recipient USDT balance: $usdt_of_recipient raw"
[[ "$usdt_of_recipient" =~ ^[0-9]+$ ]]
[ "$usdt_of_recipient" -ge "$token_amount_raw" ]

echo
echo "WALLET SMOKE PASSED: native tx $tx_native, token tx $tx_token"