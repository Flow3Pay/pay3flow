#!/usr/bin/env bash
# Provision a local test crypto wallet:
#   - starts the local test EVM node (Anvil, docker compose service)
#   - creates/loads an encrypted keystore wallet   (backend/src/bin/wallet-init.rs)
#   - funds it with test ETH and mocks USDT/USDC/TOKEN (contracts/mock/MockERC20.sol)
#   - writes data/wallet/setup.env with everything backend needs
#
# Usage:  scripts/wallet-setup.sh
# Env:    WALLET_KEYSTORE_PASSWORD (default: dev-local-test-password)
#         RPC_URL                  (default: http://localhost:8545)
#         ANVIL_DEV_KEY            (default: anvil dev account #0 test key)
#         WALLET_NATIVE_FUND_ETH   (default: 50)
#
# Requires: docker, cargo, jq
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
compose_file="$root/docker-compose.yml"
compose() { docker compose -f "$compose_file" "$@"; }

# --- overridable configuration --------------------------------------------
keystore_dir="${WALLET_KEYSTORE_DIR:-$root/data/wallet}"
password="${WALLET_KEYSTORE_PASSWORD:-dev-local-test-password}"
rpc_url="${RPC_URL:-http://localhost:8545}"
# Anvil dev account #0 — deterministic, publicly documented test key.
dev_key="${ANVIL_DEV_KEY:-0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80}"
# Anvil dev account #1 — the destination used by wallet-smoke.
recipient="${SMOKE_RECIPIENT:-0x70997970C51812dc3A010C7d01b50e0d17dc79C8}"
native_fund_eth="${WALLET_NATIVE_FUND_ETH:-50}"

need() { command -v "$1" >/dev/null || { echo "missing required command: $1" >&2; exit 2; }; }
need docker; need cargo; need jq

step() { echo; echo "== $1 =="; }

cast_in_node() { compose exec -T anvil cast "$@"; }

# --- 1. local test EVM node --------------------------------------------------
step "anvil test node ($rpc_url)"
compose up -d anvil >/dev/null
for i in $(seq 1 30); do
  if cast_in_node chain-id --rpc-url http://localhost:8545 >/dev/null 2>&1; then
    break
  fi
  sleep 1
  if [ "$i" -eq 30 ]; then
    echo "anvil did not become ready in time" >&2
    exit 1
  fi
done
chain_id="$(cast_in_node chain-id --rpc-url http://localhost:8545)"
echo "chain-id: $chain_id"

# --- 2. encrypted keystore ---------------------------------------------------
mkdir -p "$keystore_dir"
keystore="$({ find "$keystore_dir" -maxdepth 1 -type f ! -name setup.env -print -quit; } 2>/dev/null)"
if [ -z "$keystore" ]; then
  step "create encrypted keystore"
  (
    cd "$root/backend"
    WALLET_KEYSTORE_PASSWORD="$password" cargo run --quiet --bin wallet-init -- "$keystore_dir"
  )
  keystore="$({ find "$keystore_dir" -maxdepth 1 -type f ! -name setup.env -print -quit; })"
fi
# The alloy V3 keystore stores no address field; derive it by decrypting
# with cast inside the anvil container (private key never leaves the node).
addr_from_keystore() {
  local ks="$1" pk
  pk="$(cat "$ks" | compose exec -T -e KP="$password" anvil sh -c \
    'cat > /tmp/ks && cast wallet private-key --keystore /tmp/ks --password "$KP"')"
  pk="$(printf '%s\n' "$pk" | tail -n 1 | tr -d '[:space:]')"
  case "$pk" in
    0x[0-9a-fA-F]*) cast_in_node wallet address "$pk" ;;
    *) echo "failed to decrypt keystore $ks: got '$pk'" >&2; exit 1 ;;
  esac
}
wallet_addr="$(addr_from_keystore "$keystore")"
echo "wallet address: $wallet_addr"
echo "keystore file:  $keystore"

# --- 3. native ETH balance ---------------------------------------------------
step "fund native ETH (${native_fund_eth} eth)"
current_native="$(cast_in_node balance --rpc-url http://localhost:8545 "$wallet_addr" | tr -d '[:space:]')"
if [ "${current_native:-0}" = "0" ]; then
  cast_in_node send --rpc-url http://localhost:8545 --private-key "$dev_key" \
    --value "${native_fund_eth}ether" "$wallet_addr" >/dev/null
else
  echo "already funded: $(cast_in_node balance --rpc-url http://localhost:8545 "$wallet_addr") wei"
fi

# --- 4. mock ERC-20 tokens ---------------------------------------------------
# Anvil keeps state in memory and resets the chain on restart (block number
# drops to 0). If the block number recorded by a previous run is still at or
# below the current one, the chain is a strict continuation and the stored
# contract addresses may be reused; otherwise they are stale and we deploy
# fresh contracts.
prev_blocks="$(grep -E '^ANVIL_BLOCKS=' "$keystore_dir/setup.env" 2>/dev/null | tail -n 1 | cut -d= -f2 || true)"
cur_blocks="$(cast_in_node block-number --rpc-url http://localhost:8545 2>/dev/null || true)"
can_reuse=false
if [ -n "$prev_blocks" ] && [ -n "$cur_blocks" ] \
  && [ "$prev_blocks" -eq "$prev_blocks" ] 2>/dev/null \
  && [ "$cur_blocks" -ge "$prev_blocks" ] 2>/dev/null; then
  can_reuse=true
fi

declare -A contracts
for spec in USDT USDC TOKEN; do
  decimals="6"; [ "$spec" = "TOKEN" ] && decimals="18"
  addr=""
  if [ "$can_reuse" = true ]; then
    addr="$(grep -E "^${spec}_CONTRACT=" "$keystore_dir/setup.env" 2>/dev/null | tail -n 1 | cut -d= -f2 || true)"
  fi
  if [ -n "$addr" ] \
    && [ "$(cast_in_node code --rpc-url http://localhost:8545 "$addr" 2>/dev/null || true)" != "0x" ] \
    && [ "$(cast_in_node code --rpc-url http://localhost:8545 "$addr" 2>/dev/null || true)" != "" ]; then
    echo "reuse mock $spec: $addr"
  else
    step "deploy mock $spec (decimals=$decimals)"
    out="$(compose exec -T \
      -e TOKEN_NAME="$spec" -e TOKEN_SYMBOL="$spec" -e TOKEN_DECIMALS="$decimals" -e DEV_KEY="$dev_key" \
      anvil sh -c 'rm -rf /tmp/f && mkdir -p /tmp/f && cp /contracts/mock/MockERC20.sol /tmp/f/ && cd /tmp/f && forge create MockERC20.sol:MockERC20 --broadcast --rpc-url http://localhost:8545 --private-key "$DEV_KEY" --constructor-args "$TOKEN_NAME" "$TOKEN_SYMBOL" "$TOKEN_DECIMALS"' \
      2>&1)"
    addr="$(printf '%s\n' "$out" | grep -oE 'Deployed to: 0x[0-9a-fA-F]{40}' | awk '{print $3}' | head -n 1)"
    if [ -z "$addr" ]; then
      echo "mock $spec deployment failed:" >&2
      printf '%s\n' "$out" >&2
      exit 1
    fi
    echo "deployed $spec: $addr"
  fi
  contracts["$spec"]="$addr"
done

# --- 5. mint tokens to the wallet --------------------------------------------
mint_amount() { awk -v d="$1" 'BEGIN{ print 1000000 * (10 ^ d) }'; }
for spec in USDT USDC TOKEN; do
  decimals="6"; [ "$spec" = "TOKEN" ] && decimals="18"
  addr="${contracts[$spec]}"
  step "mint 1,000,000 $spec to wallet"
  cast_in_node send --rpc-url http://localhost:8545 --private-key "$dev_key" \
    "$addr" "mint(address,uint256)" "$wallet_addr" "$(mint_amount "$decimals")" >/dev/null
done

# --- 6. persistence file ------------------------------------------------------
step "write $keystore_dir/setup.env"
cur_blocks="$(cast_in_node block-number --rpc-url http://localhost:8545)"
{
  echo "# Local test wallet generated by scripts/wallet-setup.sh"
  echo "WALLET_KEYSTORE_PATH=$keystore"
  echo "# Same keystore as seen from inside the backend docker container"
  echo "WALLET_KEYSTORE_PATH_DOCKER=/app/wallet/$(basename "$keystore")"
  echo "WALLET_KEYSTORE_PASSWORD=$password"
  echo "# RPC from the host (used by cargo run / local backend)"
  echo "WALLET_RPC_URL=$rpc_url"
  echo "# RPC from inside a docker container (backend service, anvil is the compose service name)"
  echo "WALLET_RPC_URL_DOCKER=http://anvil:8545"
  echo "WALLET_ADDRESS=$wallet_addr"
  echo "WALLET_NATIVE_FUND_ETH=$native_fund_eth"
  echo "ANVIL_BLOCKS=$cur_blocks"
  echo "USDT_CONTRACT=${contracts[USDT]}"
  echo "USDC_CONTRACT=${contracts[USDC]}"
  echo "TOKEN_CONTRACT=${contracts[TOKEN]}"
  echo "RECIPIENT=$recipient"
} > "$keystore_dir/setup.env"

# --- 7. summary ---------------------------------------------------------------
step "summary"
echo "wallet:   $wallet_addr"
echo "native:   $(cast_in_node balance --rpc-url http://localhost:8545 "$wallet_addr") wei"
for spec in USDT USDC TOKEN; do
  bal="$(cast_in_node call --rpc-url http://localhost:8545 "${contracts[$spec]}" "balanceOf(address)(uint256)" "$wallet_addr")"
  raw_bal="$(printf '%s\n' "$bal" | awk '{print $1}')"
  decimals="6"; [ "$spec" = "TOKEN" ] && decimals="18"
  human="$(awk -v b="$raw_bal" -v d="$decimals" 'BEGIN{ printf "%.4f", b / (10 ^ d) }')"
  echo "  $spec:  $human  (contract ${contracts[$spec]}, $raw_bal raw)"
done
echo "keystore env file: $keystore_dir/setup.env"
echo
echo "Next: point the backend at this wallet and run scripts/wallet-smoke.sh"