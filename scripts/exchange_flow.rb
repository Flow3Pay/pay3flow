#!/usr/bin/env ruby

require "json"
require "net/http"
require "securerandom"
require "uri"

BACKEND = ENV.fetch("BACKEND_URL", "http://localhost:8080")
ADMIN_TOKEN = ENV.fetch("ADMIN_TOKEN", "dev-admin-token-change-me")
EMAIL = "smoke-#{Time.now.to_i}-#{Process.pid}@pay3flow.dev"
CODE = "1234"

def http_request(method, path, body: nil, token: nil, idempotency_key: nil)
  uri = URI.join(BACKEND, path)
  request_class = Net::HTTP.const_get(method.capitalize)
  request = request_class.new(uri)
  request["Accept"] = "application/json"
  request["Content-Type"] = "application/json" if body
  request["Authorization"] = "Bearer #{token}" if token
  request["Idempotency-Key"] = idempotency_key if idempotency_key
  request.body = JSON.generate(body) if body

  response = Net::HTTP.start(uri.hostname, uri.port, use_ssl: uri.scheme == "https") do |client|
    client.request(request)
  end
  [response.code.to_i, response.body]
end

def request(method, path, body: nil, token: nil, idempotency_key: nil)
  status, response = http_request(method, path, body:, token:, idempotency_key:)
  abort "#{method} #{path} failed with #{status}: #{response}" unless status.between?(200, 299)
  response.empty? ? nil : JSON.parse(response)
end

def admin(method, path, body = nil)
  request(method, path, body:, token: ADMIN_TOKEN)
end

def expect_equal(actual, expected, label)
  return if actual == expected

  abort "assertion failed: expected #{expected.inspect}, got #{actual.inspect} (#{label})"
end

def new_idempotency_key
  "smoke-#{Process.clock_gettime(Process::CLOCK_REALTIME, :nanosecond)}-#{SecureRandom.hex(4)}"
end

puts "[1/10] health and auth"
expect_equal(Net::HTTP.get(URI.join(BACKEND, "/health")), "ok", "backend health")
token = request("POST", "/api/auth/register", body: { email: EMAIL, code: CODE }).fetch("token")

puts "[2/10] enabled and disabled corridor"
corridors = request("GET", "/api/exchange/corridors")
corridor = corridors.fetch("items").find do |item|
  item.values_at("source_country", "source_currency", "target_country", "target_currency") == %w[AM AMD RU RUB]
end
abort "MVP AMD/RUB corridor not found" unless corridor
terms_version = corridors.fetch("terms_version")
disabled_body = {
  source_country: "US", source_currency: "USD", source_amount_minor: 100_000,
  source_method_type: "bank_card", source_method_ref: nil,
  target_country: "DE", target_currency: "EUR", target_amount_min_minor: nil,
  target_method_type: "bank_card", target_method_ref: nil
}
disabled_code, = http_request("POST", "/api/exchange/orders", body: disabled_body, token:, idempotency_key: new_idempotency_key)
expect_equal(disabled_code, 400, "disabled corridor")

order_body = {
  source_country: "AM", source_currency: "AMD", source_amount_minor: 10_000_000,
  source_method_type: "bank_card", source_method_ref: nil,
  target_country: "RU", target_currency: "RUB", target_amount_min_minor: nil,
  target_method_type: "bank_card", target_method_ref: "smoke-recipient"
}
create_order = lambda do |idempotency_key|
  request("POST", "/api/exchange/orders", body: order_body, token:, idempotency_key:)
end

puts "[3/10] idempotent create"
idempotency_key = new_idempotency_key
order = create_order.call(idempotency_key)
order_id = order.fetch("id")
expect_equal(create_order.call(idempotency_key).fetch("id"), order_id, "Idempotency-Key")

puts "[4/10] fmatch discovery with cache/local fallback allowed"
discovery = request("POST", "/api/exchange/orders/#{order_id}/discover", body: {}, token:)
abort "discovery returned no candidates" unless discovery.fetch("candidates").length >= 1
abort "unexpected discovery source" unless %w[fmatch redis_cache local_registry].include?(discovery.fetch("source"))

puts "[5/10] auction and winner"
auction = request("POST", "/api/exchange/orders/#{order_id}/auction", body: {}, token:)
quote_id = auction.dig("selected_quote", "id")
abort "auction did not select a quote" unless quote_id
expect_equal(auction.dig("order", "status"), "quoted", "auction status")

puts "[6/10] funding instruction; no consent means no settlement start"
confirm = request("POST", "/api/exchange/orders/#{order_id}/confirm", body: { quote_id: }, token:)
expect_equal(confirm.dig("order", "status"), "locked", "confirm status")
expect_equal(confirm.dig("funding_instruction", "status"), "shown_to_user", "instruction visible")
settlement = request("GET", "/api/exchange/orders/#{order_id}/settlement", token:)
expect_equal(settlement.fetch("status"), "funding_pending", "no consent gate")

puts "[7/10] consent, token leg, money leg, proof and done"
funding_body = { accepts_terms: true, terms_version: }
funded = request("POST", "/api/exchange/orders/#{order_id}/funding/confirm", body: funding_body, token:)
expect_equal(funded.dig("order", "status"), "proof_pending", "settlement after consent")
proof = {
  proof_type: "machine_receipt",
  proof_payload: {
    valid: true, reference: "happy-smoke", settlement_id: settlement.fetch("id"),
    solver_id: settlement.fetch("solver_id"), currency: auction.dig("selected_quote", "target_currency"),
    amount_minor: auction.dig("selected_quote", "target_amount_minor")
  }
}
done = request("POST", "/api/exchange/orders/#{order_id}/proof", body: proof, token:)
expect_equal(done.dig("order", "status"), "done", "happy path")

puts "[8/10] bad proof and manual dispute resolution"
bad_id = create_order.call(new_idempotency_key).fetch("id")
request("POST", "/api/exchange/orders/#{bad_id}/discover", body: {}, token:)
bad_auction = request("POST", "/api/exchange/orders/#{bad_id}/auction", body: {}, token:)
bad_quote = bad_auction.dig("selected_quote", "id")
bad_confirm = request("POST", "/api/exchange/orders/#{bad_id}/confirm", body: { quote_id: bad_quote }, token:)
request("POST", "/api/exchange/orders/#{bad_id}/funding/confirm", body: funding_body, token:)
bad_proof = {
  proof_type: "machine_receipt",
  proof_payload: {
    valid: false, reference: "bad-smoke", settlement_id: bad_confirm.dig("settlement", "id"),
    solver_id: bad_confirm.dig("settlement", "solver_id"), currency: bad_auction.dig("selected_quote", "target_currency"),
    amount_minor: bad_auction.dig("selected_quote", "target_amount_minor")
  }
}
disputed = request("POST", "/api/exchange/orders/#{bad_id}/proof", body: bad_proof, token:)
expect_equal(disputed.fetch("order").fetch("status"), "disputed", "bad proof")
resolved = admin("POST", "/api/admin/exchange/orders/#{bad_id}/dispute", { outcome: "failed", note: "smoke rejection" })
expect_equal(resolved.fetch("status"), "failed", "manual dispute resolution")

puts "[9/10] global and corridor kill switches"
admin("POST", "/api/admin/exchange/controls", { exchange_enabled: false })
kill_code, = http_request("POST", "/api/exchange/orders", body: order_body, token:, idempotency_key: new_idempotency_key)
admin("POST", "/api/admin/exchange/controls", { exchange_enabled: true })
expect_equal(kill_code, 400, "global kill switch")
admin("POST", "/api/admin/exchange/corridors/#{corridor.fetch("id")}", { enabled: false })
corridor_code, = http_request("POST", "/api/exchange/orders", body: order_body, token:, idempotency_key: new_idempotency_key)
admin("POST", "/api/admin/exchange/corridors/#{corridor.fetch("id")}", { enabled: true })
expect_equal(corridor_code, 400, "corridor kill switch")

puts "[10/10] audit and history"
audit = request("GET", "/api/debug/exchange/orders/#{order_id}/audit", token:)
abort "audit history is empty" unless audit.length.positive?
history = request("GET", "/api/exchange/orders", token:)
abort "order history does not contain smoke order" unless history.any? { |item| item.fetch("id") == order_id }
puts "PLAN2 exchange smoke passed"
