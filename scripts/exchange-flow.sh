#!/usr/bin/env bash
set -euo pipefail

backend="${BACKEND_URL:-http://localhost:8080}"
admin_token="${ADMIN_TOKEN:-dev-admin-token-change-me}"
email="smoke-$(date +%s)-$$@pay3flow.dev"
code="1234"

need() { command -v "$1" >/dev/null || { echo "missing required command: $1" >&2; exit 2; }; }
need curl
need jq

request() {
  local method="$1" path="$2" body="${3:-}" token="${4:-}" idem="${5:-}"
  local args=(-fsS -X "$method" "$backend$path" -H 'Accept: application/json')
  [[ -n "$body" ]] && args+=(-H 'Content-Type: application/json' --data "$body")
  [[ -n "$token" ]] && args+=(-H "Authorization: Bearer $token")
  [[ -n "$idem" ]] && args+=(-H "Idempotency-Key: $idem")
  curl "${args[@]}"
}

admin() {
  request "$1" "$2" "${3:-}" "$admin_token"
}

expect_eq() {
  [[ "$1" == "$2" ]] || { echo "assertion failed: expected '$2', got '$1' (${3:-})" >&2; exit 1; }
}

new_idem() { printf 'smoke-%s-%s-%s' "$(date +%s%N)" "$$" "$RANDOM"; }

echo "[1/10] health and auth"
expect_eq "$(curl -fsS "$backend/health")" "ok" "backend health"
login_body="$(jq -nc --arg email "$email" --arg code "$code" '{email:$email,code:$code}')"
token="$(request POST /api/auth/register "$login_body" | jq -er .token)"

echo "[2/10] enabled and disabled corridor"
corridors="$(request GET /api/exchange/corridors)"
corridor_id="$(jq -er '.items[] | select(.source_country=="AM" and .source_currency=="AMD" and .target_country=="RU" and .target_currency=="RUB") | .id' <<<"$corridors")"
terms_version="$(jq -er .terms_version <<<"$corridors")"
disabled_body='{"source_country":"US","source_currency":"USD","source_amount_minor":100000,"source_method_type":"bank_card","source_method_ref":null,"target_country":"DE","target_currency":"EUR","target_amount_min_minor":null,"target_method_type":"bank_card","target_method_ref":null}'
disabled_code="$(curl -sS -o /tmp/pay3flow-disabled-$$.json -w '%{http_code}' -X POST "$backend/api/exchange/orders" -H "Authorization: Bearer $token" -H "Idempotency-Key: $(new_idem)" -H 'Content-Type: application/json' --data "$disabled_body")"
expect_eq "$disabled_code" "400" "disabled corridor"

order_body='{"source_country":"AM","source_currency":"AMD","source_amount_minor":10000000,"source_method_type":"bank_card","source_method_ref":null,"target_country":"RU","target_currency":"RUB","target_amount_min_minor":null,"target_method_type":"bank_card","target_method_ref":"smoke-recipient"}'

create_order() {
  local idem="$1"
  request POST /api/exchange/orders "$order_body" "$token" "$idem"
}

echo "[3/10] idempotent create"
idem="$(new_idem)"
order="$(create_order "$idem")"
order_id="$(jq -er .id <<<"$order")"
same_id="$(create_order "$idem" | jq -er .id)"
expect_eq "$same_id" "$order_id" "Idempotency-Key"

echo "[4/10] fmatch discovery with cache/local fallback allowed"
discovery="$(request POST "/api/exchange/orders/$order_id/discover" '{}' "$token")"
jq -e '.candidates | length >= 1' >/dev/null <<<"$discovery"
jq -e '.source == "fmatch" or .source == "redis_cache" or .source == "local_registry"' >/dev/null <<<"$discovery"

echo "[5/10] auction and winner"
auction="$(request POST "/api/exchange/orders/$order_id/auction" '{}' "$token")"
quote_id="$(jq -er .selected_quote.id <<<"$auction")"
expect_eq "$(jq -r .order.status <<<"$auction")" "quoted" "auction status"

echo "[6/10] funding instruction; no consent means no settlement start"
confirm="$(request POST "/api/exchange/orders/$order_id/confirm" "$(jq -nc --arg id "$quote_id" '{quote_id:$id}')" "$token")"
expect_eq "$(jq -r .order.status <<<"$confirm")" "locked" "confirm status"
expect_eq "$(jq -r .funding_instruction.status <<<"$confirm")" "shown_to_user" "instruction visible"
settlement="$(request GET "/api/exchange/orders/$order_id/settlement" '' "$token")"
expect_eq "$(jq -r .status <<<"$settlement")" "funding_pending" "no consent gate"

echo "[7/10] consent, token leg, money leg, proof and done"
funding_body="$(jq -nc --arg version "$terms_version" '{accepts_terms:true,terms_version:$version}')"
funded="$(request POST "/api/exchange/orders/$order_id/funding/confirm" "$funding_body" "$token")"
expect_eq "$(jq -r .order.status <<<"$funded")" "proof_pending" "settlement after consent"
proof="$(jq -nc \
  --arg settlement_id "$(jq -r .id <<<"$settlement")" \
  --arg solver_id "$(jq -r .solver_id <<<"$settlement")" \
  --arg currency "$(jq -r .selected_quote.target_currency <<<"$auction")" \
  --argjson amount_minor "$(jq -r .selected_quote.target_amount_minor <<<"$auction")" \
  '{proof_type:"machine_receipt",proof_payload:{valid:true,reference:"happy-smoke",settlement_id:$settlement_id,solver_id:$solver_id,currency:$currency,amount_minor:$amount_minor}}')"
done="$(request POST "/api/exchange/orders/$order_id/proof" "$proof" "$token")"
expect_eq "$(jq -r .order.status <<<"$done")" "done" "happy path"

echo "[8/10] bad proof and manual dispute resolution"
bad_id="$(create_order "$(new_idem)" | jq -er .id)"
request POST "/api/exchange/orders/$bad_id/discover" '{}' "$token" >/dev/null
bad_auction="$(request POST "/api/exchange/orders/$bad_id/auction" '{}' "$token")"
bad_quote="$(jq -er .selected_quote.id <<<"$bad_auction")"
bad_confirm="$(request POST "/api/exchange/orders/$bad_id/confirm" "$(jq -nc --arg id "$bad_quote" '{quote_id:$id}')" "$token")"
request POST "/api/exchange/orders/$bad_id/funding/confirm" "$funding_body" "$token" >/dev/null
bad_proof="$(jq -nc \
  --arg settlement_id "$(jq -r .settlement.id <<<"$bad_confirm")" \
  --arg solver_id "$(jq -r .settlement.solver_id <<<"$bad_confirm")" \
  --arg currency "$(jq -r .selected_quote.target_currency <<<"$bad_auction")" \
  --argjson amount_minor "$(jq -r .selected_quote.target_amount_minor <<<"$bad_auction")" \
  '{proof_type:"machine_receipt",proof_payload:{valid:false,reference:"bad-smoke",settlement_id:$settlement_id,solver_id:$solver_id,currency:$currency,amount_minor:$amount_minor}}')"
disputed="$(request POST "/api/exchange/orders/$bad_id/proof" "$bad_proof" "$token")"
expect_eq "$(jq -r .order.status <<<"$disputed")" "disputed" "bad proof"
resolved="$(admin POST "/api/admin/exchange/orders/$bad_id/dispute" '{"outcome":"failed","note":"smoke rejection"}')"
expect_eq "$(jq -r .status <<<"$resolved")" "failed" "manual dispute resolution"

echo "[9/10] global and corridor kill switches"
admin POST /api/admin/exchange/controls '{"exchange_enabled":false}' >/dev/null
kill_code="$(curl -sS -o /tmp/pay3flow-kill-$$.json -w '%{http_code}' -X POST "$backend/api/exchange/orders" -H "Authorization: Bearer $token" -H "Idempotency-Key: $(new_idem)" -H 'Content-Type: application/json' --data "$order_body")"
admin POST /api/admin/exchange/controls '{"exchange_enabled":true}' >/dev/null
expect_eq "$kill_code" "400" "global kill switch"
admin POST "/api/admin/exchange/corridors/$corridor_id" '{"enabled":false}' >/dev/null
corridor_code="$(curl -sS -o /tmp/pay3flow-corridor-$$.json -w '%{http_code}' -X POST "$backend/api/exchange/orders" -H "Authorization: Bearer $token" -H "Idempotency-Key: $(new_idem)" -H 'Content-Type: application/json' --data "$order_body")"
admin POST "/api/admin/exchange/corridors/$corridor_id" '{"enabled":true}' >/dev/null
expect_eq "$corridor_code" "400" "corridor kill switch"

echo "[10/10] audit and history"
jq -e 'length > 0' >/dev/null <<<"$(request GET "/api/debug/exchange/orders/$order_id/audit" '' "$token")"
jq -e --arg id "$order_id" 'map(.id) | index($id) != null' >/dev/null <<<"$(request GET /api/exchange/orders '' "$token")"
rm -f "/tmp/pay3flow-disabled-$$.json" "/tmp/pay3flow-kill-$$.json" "/tmp/pay3flow-corridor-$$.json"
echo "PLAN2 exchange smoke passed"
