use axum::Json;
use serde_json::{json, Map, Value};

/// Return the OpenAPI document used by the Scalar API reference.
pub async fn document() -> Json<Value> {
    Json(api_document())
}

fn api_document() -> Value {
    let mut paths = Map::new();
    for operation in OPERATIONS {
        add_operation(&mut paths, *operation);
    }

    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Pay3Flow API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Pay3Flow routes payments and cross-border exchanges between fiat and digital-asset rails.\n\n## Quick start\n\n1. Register or log in with an email and one-time code.\n2. Copy the returned `token` into the **Authorize** dialog as a bearer token.\n3. Create a payment or exchange order.\n\nAmounts in exchange endpoints use the smallest currency unit (for example, cents). Amounts in the payment endpoint are decimal currency units. `Idempotency-Key` is recommended for every request that creates or confirms a transaction.\n\nWebSocket endpoints are listed for discovery; use a WebSocket client to interact with them."
        },
        "servers": [{ "url": "/", "description": "Current Pay3Flow origin" }],
        "tags": [
            { "name": "System", "description": "Health and metrics endpoints." },
            { "name": "Authentication", "description": "Create a user session and inspect the current user. The returned JWT is used by protected endpoints." },
            { "name": "Referrals", "description": "Direct referral attribution and commission balances." },
            { "name": "Payments", "description": "Create and monitor one-step payments." },
            { "name": "Exchange", "description": "Create an exchange order, discover solvers, select a quote and track settlement." },
            { "name": "Solver", "description": "Solver-facing order and quote endpoints." },
            { "name": "Catalogs", "description": "Public banks, networks, providers and exchange-pair catalogs." },
            { "name": "Routing", "description": "P2P search, route execution and route reputation." },
            { "name": "Administration", "description": "Protected operational controls. These endpoints require the configured admin bearer token." },
            { "name": "WebSockets", "description": "Real-time rates, payment events and route updates." },
            { "name": "ActivityPub", "description": "Federation and marketplace discovery endpoints." },
            { "name": "Debug", "description": "Development and diagnostics endpoints; do not expose them publicly without access controls." }
        ],
        "paths": paths,
        "components": components()
    })
}

fn components() -> Value {
    let mut components = json!({
        "securitySchemes": {
            "bearerAuth": {
                "type": "http",
                "scheme": "bearer",
                "bearerFormat": "JWT",
                "description": "JWT returned by `/api/auth/register` or `/api/auth/login`."
            },
            "adminBearerAuth": {
                "type": "http",
                "scheme": "bearer",
                "description": "Admin token configured by the backend operator."
            }
        },
        "schemas": {
            "AuthCodeRequest": {
                "type": "object",
                "required": ["email", "code"],
                "properties": {
                    "email": { "type": "string", "format": "email", "example": "user@example.com" },
                    "code": { "type": "string", "description": "One-time code delivered to the email address.", "example": "123456" },
                    "referral_code": { "type": "string", "description": "Optional inviter code, accepted only when a new account is registered.", "example": "A1B2C3D4E5F60718" }
                }
            },
            "AuthToken": {
                "type": "object",
                "required": ["token"],
                "properties": { "token": { "type": "string", "description": "JWT bearer token." } }
            },
            "AnonymousRegisterRequest": {
                "type": "object",
                "required": ["anonymous_id"],
                "additionalProperties": false,
                "properties": {
                    "anonymous_id": { "type": "string", "format": "uuid", "description": "Random browser-generated pseudonymous ID kept in localStorage." }
                }
            },
            "AnonymousUser": {
                "type": "object",
                "required": ["anonymous_id"],
                "properties": {
                    "anonymous_id": { "type": "string", "format": "uuid" }
                }
            },
            "MarketValuesRequest": {
                "type": "object",
                "required": ["items"],
                "properties": { "items": { "type": "array", "minItems": 1, "maxItems": 4, "items": { "type": "object", "required": ["asset", "amount"], "properties": {
                    "asset": { "type": "string", "example": "BTC" },
                    "amount": { "type": "string", "example": "0.5" }
                } } } }
            },
            "MarketValuesResponse": {
                "type": "object",
                "required": ["source", "stale", "values"],
                "properties": {
                    "source": { "type": "string", "example": "DefiLlama" },
                    "stale": { "type": "boolean" },
                    "values": { "type": "array", "items": { "type": "object", "required": ["asset", "amount", "price_usd", "value_usd", "price_timestamp"], "properties": {
                        "asset": { "type": "string" }, "amount": { "type": "string" },
                        "price_usd": { "type": "number", "nullable": true }, "value_usd": { "type": "number", "nullable": true },
                        "price_timestamp": { "type": "integer", "format": "int64", "nullable": true }
                    } } }
                }
            },
            "MarketPricesResponse": {
                "type": "object",
                "required": ["source", "stale", "updated_at", "prices"],
                "properties": {
                    "source": { "type": "string", "example": "DefiLlama" },
                    "stale": { "type": "boolean" },
                    "updated_at": { "type": "integer", "format": "int64", "nullable": true, "description": "Unix time of the last successful server refresh." },
                    "prices": { "type": "object", "additionalProperties": { "type": "number", "format": "double" }, "description": "USD price per one unit of each supported asset." }
                }
            },
            "User": {
                "type": "object",
                "required": ["id", "email", "referral_code"],
                "properties": {
                    "id": { "type": "string", "format": "uuid" },
                    "email": { "type": "string", "format": "email" },
                    "referral_code": { "type": "string" }
                }
            },
            "NewPayment": {
                "type": "object",
                "required": ["amount", "currency", "from", "to"],
                "properties": {
                    "amount": { "type": "number", "format": "double", "minimum": 0, "exclusiveMinimum": true, "example": 100.5 },
                    "currency": { "type": "string", "description": "Source currency, for example EUR.", "example": "EUR" },
                    "from": { "type": "string", "description": "Source account or payment address.", "example": "sender@example.com" },
                    "to": { "type": "string", "description": "Destination account or payment address.", "example": "recipient@example.com" },
                    "to_geo": { "type": "string", "description": "Optional destination country or region.", "example": "DE" },
                    "method": { "type": "string", "description": "Optional payment method.", "example": "bank_transfer" },
                    "to_currency": { "type": "string", "description": "Optional destination currency for conversion.", "example": "USD" }
                }
            },
            "PaymentView": {
                "type": "object",
                "required": ["transaction"],
                "properties": {
                    "transaction": { "$ref": "#/components/schemas/Transaction" },
                    "route": { "$ref": "#/components/schemas/Route" },
                    "quote": { "$ref": "#/components/schemas/Quote" }
                }
            },
            "Transaction": {
                "type": "object",
                "description": "Persisted payment. Amount fields are in minor currency units.",
                "properties": {
                    "id": { "type": "string", "format": "uuid" },
                    "status": { "type": "string", "enum": ["pending", "matched", "executing", "done", "failed"] },
                    "from_amount": { "type": "integer", "format": "int64" },
                    "from_currency": { "type": "string" },
                    "to_amount": { "type": "integer", "format": "int64", "nullable": true },
                    "to_currency": { "type": "string", "nullable": true },
                    "from_account": { "type": "string" },
                    "to_account": { "type": "string" },
                    "method": { "type": "string", "nullable": true },
                    "fees": { "type": "integer", "format": "int64" },
                    "provider": { "type": "string", "nullable": true },
                    "external_id": { "type": "string", "nullable": true },
                    "created_at": { "type": "string", "format": "date-time" },
                    "updated_at": { "type": "string", "format": "date-time" }
                }
            },
            "Route": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "format": "uuid" },
                    "acquirer_slug": { "type": "string" },
                    "fee_percent": { "type": "number", "format": "double" },
                    "exchange_rate": { "type": "number", "format": "double", "nullable": true },
                    "status": { "type": "string" },
                    "source": { "type": "string" }
                }
            },
            "Quote": {
                "type": "object",
                "properties": {
                    "request": { "$ref": "#/components/schemas/PaymentRequest" },
                    "source": { "type": "string" },
                    "candidates": { "type": "array", "items": { "$ref": "#/components/schemas/JsonObject" } },
                    "best": { "$ref": "#/components/schemas/JsonObject", "nullable": true },
                    "quoted_at": { "type": "string", "format": "date-time" }
                }
            },
            "PaymentRequest": {
                "allOf": [{ "$ref": "#/components/schemas/NewPayment" }]
            },
            "CreateExchangeOrder": {
                "type": "object",
                "required": ["source_country", "source_currency", "source_amount_minor", "source_method_type", "target_country", "target_currency", "target_method_type"],
                "properties": {
                    "source_country": { "type": "string", "example": "RU" },
                    "source_currency": { "type": "string", "example": "RUB" },
                    "source_amount_minor": { "type": "integer", "format": "int64", "description": "Amount in the smallest source-currency unit.", "example": 100000 },
                    "source_method_type": { "type": "string", "example": "bank_card" },
                    "source_method_ref": { "type": "string", "nullable": true },
                    "target_country": { "type": "string", "example": "DE" },
                    "target_currency": { "type": "string", "example": "EUR" },
                    "target_amount_min_minor": { "type": "integer", "format": "int64", "nullable": true },
                    "target_method_type": { "type": "string", "example": "bank_transfer" },
                    "target_method_ref": { "type": "string", "nullable": true },
                    "deadline_at": { "type": "string", "format": "date-time", "nullable": true }
                }
            },
            "ExchangeOrder": {
                "type": "object",
                "description": "Exchange order and its current lifecycle state.",
                "properties": {
                    "id": { "type": "string", "format": "uuid" },
                    "status": { "type": "string", "enum": ["created", "discovering", "quoting", "quoted", "locked", "token_settling", "money_settling", "proof_pending", "done", "failed", "expired", "cancelled", "disputed"] },
                    "funding_status": { "type": "string" },
                    "source_currency": { "type": "string" },
                    "source_amount_minor": { "type": "integer", "format": "int64" },
                    "pay3flow_fee_minor": { "type": "integer", "format": "int64", "description": "Pay3Flow service fee in the source currency." },
                    "pay3flow_fee_currency": { "type": "string" },
                    "target_currency": { "type": "string" },
                    "target_amount_min_minor": { "type": "integer", "format": "int64", "nullable": true },
                    "selected_quote_id": { "type": "string", "format": "uuid", "nullable": true },
                    "failure_code": { "type": "string", "nullable": true },
                    "failure_message": { "type": "string", "nullable": true },
                    "created_at": { "type": "string", "format": "date-time" },
                    "updated_at": { "type": "string", "format": "date-time" }
                }
            },
            "ConfirmOrderRequest": {
                "type": "object",
                "properties": { "quote_id": { "type": "string", "format": "uuid", "nullable": true } }
            },
            "FundingConfirmRequest": {
                "type": "object",
                "required": ["accepts_terms", "terms_version"],
                "properties": {
                    "accepts_terms": { "type": "boolean", "example": true },
                    "terms_version": { "type": "string", "example": "2026-01" }
                }
            },
            "SubmitProofRequest": {
                "type": "object",
                "required": ["proof_type", "proof_payload"],
                "properties": {
                    "proof_type": { "type": "string", "example": "bank_transfer_receipt" },
                    "proof_payload": { "type": "object", "additionalProperties": true }
                }
            },
            "ReferralProfile": {
                "type": "object",
                "required": ["referral_code", "commission_bps", "direct_referrals", "network_size", "balances", "recent_commissions"],
                "properties": {
                    "referral_code": { "type": "string" },
                    "commission_bps": { "type": "integer", "example": 1000, "description": "Direct-referral share of Pay3Flow's service fee in basis points." },
                    "direct_referrals": { "type": "integer", "format": "int64" },
                    "network_size": { "type": "integer", "format": "int64", "description": "All descendants in the referral tree; commission is paid only for direct referrals." },
                    "balances": { "type": "array", "items": { "$ref": "#/components/schemas/JsonObject" } },
                    "recent_commissions": { "type": "array", "items": { "$ref": "#/components/schemas/JsonObject" } }
                }
            },
            "JsonObject": { "type": "object", "additionalProperties": true },
            "JsonResponse": { "type": "object", "additionalProperties": true },
            "Error": {
                "type": "object",
                "properties": {
                    "error": { "type": "string", "example": "unauthorized" },
                    "message": { "type": "string", "example": "invalid or expired token" }
                },
                "additionalProperties": true
            }
        }
    });
    let schemas = components["schemas"]
        .as_object_mut()
        .expect("OpenAPI schemas must be an object");
    schemas.insert(
        "CreateRouteExecution".into(),
        create_route_execution_schema(),
    );
    schemas.insert(
        "SubmitRouteExecution".into(),
        submit_route_execution_schema(),
    );
    schemas.insert("RouteExecution".into(), route_execution_schema());
    components
}

fn create_route_execution_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["anonymous_id", "route_token", "source_address", "recipient"],
        "properties": {
            "anonymous_id": { "type": "string", "format": "uuid" },
            "route_token": { "type": "string", "description": "Short-lived signed token returned on an executable route." },
            "source_address": { "type": "string" },
            "recipient": { "type": "string" },
            "refund_to": { "type": "string", "nullable": true },
            "amount": { "type": "string", "description": "Must equal the amount in the signed route descriptor." },
            "slippage_bps": { "type": "integer", "minimum": 0, "maximum": 10000, "default": 100 }
        }
    })
}

fn submit_route_execution_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["anonymous_id", "reference", "kind"],
        "properties": {
            "anonymous_id": { "type": "string", "format": "uuid" },
            "reference": { "type": "string", "description": "Wallet transaction hash or CoW order UID." },
            "kind": { "type": "string", "enum": ["transaction_hash", "order_uid"] }
        }
    })
}

fn route_execution_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "route_id", "provider", "status", "from_asset", "to_asset", "input_amount", "source_address", "recipient", "action", "quote_expires_at", "updated_at"],
        "properties": {
            "id": { "type": "string", "format": "uuid" },
            "route_id": { "type": "string" },
            "provider": { "type": "string", "enum": ["near-intents", "cow-swap", "symbiosis"] },
            "status": { "type": "string", "enum": ["awaiting_signature", "submitted", "completed", "failed", "cancelled", "expired", "refunded", "stuck"] },
            "from_asset": { "type": "string" },
            "to_asset": { "type": "string" },
            "input_amount": { "type": "string" },
            "source_address": { "type": "string" },
            "recipient": { "type": "string" },
            "action": { "$ref": "#/components/schemas/JsonObject" },
            "provider_reference": { "type": "string", "nullable": true },
            "submitted_reference": { "type": "string", "nullable": true },
            "provider_status": { "$ref": "#/components/schemas/JsonObject" },
            "quote_expires_at": { "type": "string", "format": "date-time" },
            "updated_at": { "type": "string", "format": "date-time" }
        }
    })
}

type Operation = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    bool,
    bool,
);

const OPERATIONS: &[Operation] = &[
    (
        "/health",
        "get",
        "Health check",
        "Returns `ok` when the API is reachable.",
        "System",
        false,
        false,
    ),
    (
        "/metrics",
        "get",
        "Metrics",
        "Return backend Prometheus metrics.",
        "System",
        false,
        false,
    ),
    (
        "/api/auth/register",
        "post",
        "Register",
        "Create an account with an email and one-time code.",
        "Authentication",
        true,
        false,
    ),
    (
        "/api/anonymous/register",
        "post",
        "Register anonymous browser",
        "Store a random pseudonymous browser ID for usage metrics. No contact, device, or network identity is collected.",
        "System",
        true,
        false,
    ),
    (
        "/api/auth/login",
        "post",
        "Log in",
        "Exchange an email and one-time code for a bearer token.",
        "Authentication",
        true,
        false,
    ),
    (
        "/api/auth/me",
        "get",
        "Current user",
        "Return the authenticated user.",
        "Authentication",
        false,
        true,
    ),
    (
        "/api/referrals/me",
        "get",
        "Referral network",
        "Return the caller's invite code, network counts, per-currency balances, and recent direct-referral commissions.",
        "Referrals",
        false,
        true,
    ),
    (
        "/api/auth/oauth/{provider}",
        "post",
        "OAuth login",
        "Authenticate through a configured OAuth provider.",
        "Authentication",
        true,
        false,
    ),
    (
        "/api/payments",
        "post",
        "Create payment",
        "Create and route a payment.",
        "Payments",
        true,
        true,
    ),
    (
        "/api/payments",
        "get",
        "List payments",
        "List the authenticated user's payments.",
        "Payments",
        false,
        true,
    ),
    (
        "/api/payments/{id}",
        "get",
        "Get payment",
        "Return one payment owned by the authenticated user.",
        "Payments",
        false,
        true,
    ),
    (
        "/api/providers/{provider}/webhooks",
        "post",
        "Provider webhook",
        "Record an asynchronous provider event.",
        "Payments",
        true,
        false,
    ),
    (
        "/api/exchange/orders",
        "post",
        "Create exchange order",
        "Create an idempotent exchange order.",
        "Exchange",
        true,
        true,
    ),
    (
        "/api/exchange/orders",
        "get",
        "List exchange orders",
        "List exchange orders for the authenticated user.",
        "Exchange",
        false,
        true,
    ),
    (
        "/api/exchange/corridors",
        "get",
        "List corridors",
        "List enabled exchange corridors and the active terms version.",
        "Exchange",
        false,
        false,
    ),
    (
        "/api/exchange/orders/{id}",
        "get",
        "Get exchange order",
        "Return one exchange order owned by the authenticated user.",
        "Exchange",
        false,
        true,
    ),
    (
        "/api/exchange/orders/{id}/quotes",
        "get",
        "List order quotes",
        "List quotes collected for an exchange order.",
        "Exchange",
        false,
        true,
    ),
    (
        "/api/exchange/orders/{id}/live",
        "get",
        "Live routes",
        "Open a WebSocket stream of live routes for an exchange order.",
        "Exchange",
        false,
        true,
    ),
    (
        "/api/exchange/orders/{id}/discover",
        "post",
        "Discover solvers",
        "Ask the marketplace for solver candidates.",
        "Exchange",
        false,
        true,
    ),
    (
        "/api/exchange/orders/{id}/auction",
        "post",
        "Run auction",
        "Run quote selection for an exchange order.",
        "Exchange",
        false,
        true,
    ),
    (
        "/api/exchange/orders/{id}/confirm",
        "post",
        "Confirm exchange order",
        "Confirm the selected quote and prepare settlement.",
        "Exchange",
        true,
        true,
    ),
    (
        "/api/exchange/orders/{id}/funding/confirm",
        "post",
        "Confirm funding",
        "Confirm the funding terms for an exchange order.",
        "Exchange",
        true,
        true,
    ),
    (
        "/api/exchange/orders/{id}/manual-review",
        "get",
        "Get manual review",
        "Return the manual-review state for an exchange order.",
        "Exchange",
        false,
        true,
    ),
    (
        "/api/exchange/orders/{id}/settlement",
        "get",
        "Get settlement",
        "Return settlement details for an exchange order.",
        "Exchange",
        false,
        true,
    ),
    (
        "/api/exchange/orders/{id}/ledger",
        "get",
        "Get ledger",
        "Return ledger operations for an exchange order.",
        "Exchange",
        false,
        true,
    ),
    (
        "/api/exchange/orders/{id}/proofs",
        "get",
        "List proofs",
        "List submitted proofs for an exchange order.",
        "Exchange",
        false,
        true,
    ),
    (
        "/api/exchange/orders/{id}/proof",
        "post",
        "Submit proof",
        "Submit a settlement proof for an exchange order.",
        "Exchange",
        true,
        true,
    ),
    (
        "/api/exchange/orders/{id}/cancel",
        "post",
        "Cancel exchange order",
        "Cancel an exchange order owned by the authenticated user.",
        "Exchange",
        false,
        true,
    ),
    (
        "/api/debug/exchange/orders/{id}/audit",
        "get",
        "Get audit events",
        "Return audit events for an exchange order.",
        "Debug",
        false,
        true,
    ),
    (
        "/api/solver/orders/open",
        "get",
        "Open solver orders",
        "List open orders available to solvers.",
        "Solver",
        false,
        false,
    ),
    (
        "/api/solver/orders/{id}/quotes",
        "post",
        "Submit solver quote",
        "Submit a quote for an open exchange order.",
        "Solver",
        true,
        false,
    ),
    (
        "/api/exchange-pairs",
        "get",
        "List exchange pairs",
        "List enabled bank and asset exchange pairs.",
        "Catalogs",
        false,
        false,
    ),
    (
        "/api/banks",
        "get",
        "List banks",
        "List enabled payment methods and banks.",
        "Catalogs",
        false,
        false,
    ),
    (
        "/api/networks",
        "get",
        "List networks",
        "List supported crypto networks.",
        "Catalogs",
        false,
        false,
    ),
    (
        "/api/market-prices",
        "get",
        "List cached crypto prices in USD",
        "Return the server's latest crypto price coefficients. The server refreshes them at startup and once per hour.",
        "Catalogs",
        false,
        false,
    ),
    (
        "/api/market-values",
        "post",
        "Calculate crypto market values in USD",
        "Return indicative USD market values for up to four supported crypto amounts using server-cached prices. Unknown or unavailable assets have null values.",
        "Catalogs",
        true,
        false,
    ),
    (
        "/api/providers",
        "get",
        "List providers",
        "List configured provider definitions.",
        "Catalogs",
        false,
        false,
    ),
    (
        "/api/p2p/search",
        "get",
        "Search P2P offers",
        "Search public P2P offers for a payment route.",
        "Routing",
        false,
        false,
    ),
    (
        "/api/p2p/routes",
        "get",
        "List routes",
        "Return currently available payment routes.",
        "Routing",
        false,
        false,
    ),
    (
        "/api/p2p/route-activity",
        "get",
        "Route search activity",
        "Return completed user-initiated search counts for a currency direction over the selected period. Automatic route refreshes are excluded. Counts begin with this metric's corrected release.",
        "Routing",
        false,
        false,
    ),
    (
        "/api/p2p/route-executions",
        "post",
        "Prepare wallet execution",
        "Validate a signed route descriptor and create a fresh provider action for the supplied source and recipient addresses. No private key or signature is sent to Pay3Flow.",
        "Routing",
        true,
        false,
    ),
    (
        "/api/p2p/route-executions/{id}",
        "get",
        "Get wallet execution",
        "Return persisted state and poll the provider when the operation has been submitted.",
        "Routing",
        false,
        false,
    ),
    (
        "/api/p2p/route-executions/{id}/submissions",
        "post",
        "Record wallet submission",
        "Record the public transaction hash or CoW order UID after the user confirms it in their wallet.",
        "Routing",
        true,
        false,
    ),
    (
        "/api/service-executions/open",
        "post",
        "Open service execution",
        "Open a service execution for a selected route.",
        "Routing",
        true,
        false,
    ),
    (
        "/api/route-instructions/open",
        "post",
        "Open route instructions",
        "Record an anonymous instruction open using signed service links from one route.",
        "Routing",
        true,
        false,
    ),
    (
        "/api/services/{id}/vote",
        "put",
        "Vote on service",
        "Record a vote for a service execution.",
        "Routing",
        true,
        false,
    ),
    (
        "/api/routes/{route_id}/vote",
        "put",
        "Vote on route",
        "Record a vote for a route.",
        "Routing",
        true,
        false,
    ),
    (
        "/api/debug/quote",
        "post",
        "Debug quote",
        "Compute a quote through the HTTP debug surface.",
        "Routing",
        true,
        false,
    ),
    (
        "/routing/fallback",
        "post",
        "Fallback routing",
        "Resolve a fallback routing request.",
        "Routing",
        true,
        false,
    ),
    (
        "/api/admin/exchange-pairs",
        "post",
        "Create exchange pair",
        "Create or update an exchange pair. Requires the admin bearer token.",
        "Administration",
        true,
        true,
    ),
    (
        "/api/admin/exchange-pairs/{id}",
        "post",
        "Update exchange pair",
        "Update an exchange pair. Requires the admin bearer token.",
        "Administration",
        true,
        true,
    ),
    (
        "/api/admin/exchange/controls",
        "get",
        "Get exchange controls",
        "Read exchange controls. Requires the admin bearer token.",
        "Administration",
        false,
        true,
    ),
    (
        "/api/admin/exchange/controls",
        "post",
        "Update exchange controls",
        "Update exchange controls. Requires the admin bearer token.",
        "Administration",
        true,
        true,
    ),
    (
        "/api/admin/exchange/corridors/{id}",
        "post",
        "Toggle corridor",
        "Enable or disable an exchange corridor. Requires the admin bearer token.",
        "Administration",
        true,
        true,
    ),
    (
        "/api/admin/exchange/solvers/{id}",
        "post",
        "Set solver status",
        "Change a solver status. Requires the admin bearer token.",
        "Administration",
        true,
        true,
    ),
    (
        "/api/admin/exchange/orders/{id}/manual-review",
        "post",
        "Resolve manual review",
        "Resolve a manual review. Requires the admin bearer token.",
        "Administration",
        true,
        true,
    ),
    (
        "/api/admin/exchange/orders/{id}/dispute",
        "post",
        "Resolve dispute",
        "Resolve an exchange dispute. Requires the admin bearer token.",
        "Administration",
        true,
        true,
    ),
    (
        "/api/admin/banks",
        "post",
        "Create bank",
        "Create or update a bank. Requires the admin bearer token.",
        "Administration",
        true,
        true,
    ),
    (
        "/api/admin/banks/{name}/status",
        "post",
        "Set bank status",
        "Enable or disable a bank. Requires the admin bearer token.",
        "Administration",
        true,
        true,
    ),
    (
        "/ws",
        "get",
        "Echo WebSocket",
        "WebSocket echo endpoint.",
        "WebSockets",
        false,
        false,
    ),
    (
        "/ws/rates",
        "get",
        "Live rates WebSocket",
        "WebSocket for live quote requests and responses.",
        "WebSockets",
        false,
        false,
    ),
    (
        "/ws/payments",
        "get",
        "Payment events WebSocket",
        "WebSocket stream of payment status events.",
        "WebSockets",
        false,
        false,
    ),
    (
        "/ws/p2p/routes",
        "get",
        "P2P routes WebSocket",
        "WebSocket stream for P2P route updates.",
        "WebSockets",
        false,
        false,
    ),
    (
        "/.well-known/webfinger",
        "get",
        "WebFinger",
        "ActivityPub WebFinger discovery endpoint.",
        "ActivityPub",
        false,
        false,
    ),
    (
        "/actor",
        "get",
        "Actor collection",
        "Return the ActivityPub actor collection.",
        "ActivityPub",
        false,
        false,
    ),
    (
        "/actor/{handle}",
        "get",
        "Actor document",
        "Return an ActivityPub actor document by handle.",
        "ActivityPub",
        false,
        false,
    ),
    (
        "/marketplace/resources/exchange",
        "get",
        "Exchange resource",
        "Return the exchange marketplace resource.",
        "ActivityPub",
        false,
        false,
    ),
    (
        "/marketplace/proposals/exchange",
        "get",
        "Exchange proposal",
        "Return the exchange marketplace proposal.",
        "ActivityPub",
        false,
        false,
    ),
    (
        "/marketplace/interfaces/exchange",
        "get",
        "Exchange interface",
        "Return the exchange marketplace interface.",
        "ActivityPub",
        false,
        false,
    ),
    (
        "/marketplace/shapes/exchange-input",
        "get",
        "Exchange input shape",
        "Return the exchange input shape.",
        "ActivityPub",
        false,
        false,
    ),
    (
        "/marketplace/shapes/exchange-output",
        "get",
        "Exchange output shape",
        "Return the exchange output shape.",
        "ActivityPub",
        false,
        false,
    ),
    (
        "/marketplace/preview",
        "post",
        "Preview exchange",
        "Preview an exchange request.",
        "ActivityPub",
        true,
        false,
    ),
    (
        "/candidates",
        "get",
        "List candidates",
        "Return marketplace candidates.",
        "ActivityPub",
        false,
        false,
    ),
    (
        "/api/debug/task",
        "post",
        "Debug task",
        "Run a debug marketplace task.",
        "Debug",
        true,
        false,
    ),
    (
        "/inbox",
        "post",
        "Shared inbox",
        "Receive an ActivityPub activity in the shared inbox.",
        "ActivityPub",
        true,
        false,
    ),
    (
        "/inbox/{handle}",
        "post",
        "Actor inbox",
        "Receive an ActivityPub activity in a named inbox.",
        "ActivityPub",
        true,
        false,
    ),
];

fn add_operation(paths: &mut Map<String, Value>, operation: Operation) {
    let (path, method, summary, description, tag, request_body, requires_auth) = operation;
    let mut operation = json!({
        "summary": summary,
        "description": description,
        "tags": [tag],
        "responses": {
            "200": success_response(path, method),
            "400": { "description": "Invalid request", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Error" } } } },
            "401": { "description": "Authentication required" },
            "404": { "description": "Resource not found" }
        }
    });

    if request_body {
        operation["requestBody"] = json!({
            "required": true,
            "content": { "application/json": { "schema": request_schema(path, method) } }
        });
    }
    if requires_auth {
        operation["security"] = if path.starts_with("/api/admin/") {
            json!([{ "adminBearerAuth": [] }])
        } else {
            json!([{ "bearerAuth": [] }])
        };
    }

    let parameters: Vec<Value> = path
        .split('{')
        .skip(1)
        .filter_map(|part| part.split('}').next())
        .map(|name| json!({ "name": name, "in": "path", "required": true, "schema": { "type": "string" } }))
        .collect();
    if !parameters.is_empty() {
        operation["parameters"] = json!(parameters);
    }

    let extra_parameters = extra_parameters(path, method);
    if !extra_parameters.is_empty() {
        operation["parameters"] = match operation.get("parameters").cloned() {
            Some(Value::Array(mut parameters)) => {
                parameters.extend(extra_parameters);
                Value::Array(parameters)
            }
            _ => Value::Array(extra_parameters),
        };
    }

    paths
        .entry(path.to_owned())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .expect("OpenAPI path item must be an object")
        .insert(method.to_owned(), operation);
}

fn schema_ref(name: &str) -> Value {
    json!({ "$ref": format!("#/components/schemas/{name}") })
}

fn array_schema(name: &str) -> Value {
    json!({ "type": "array", "items": schema_ref(name) })
}

fn request_schema(path: &str, method: &str) -> Value {
    let name = match (path, method) {
        ("/api/auth/register", "post") | ("/api/auth/login", "post") => "AuthCodeRequest",
        ("/api/anonymous/register", "post") => "AnonymousRegisterRequest",
        ("/api/market-values", "post") => "MarketValuesRequest",
        ("/api/payments", "post") => "NewPayment",
        ("/api/exchange/orders", "post") => "CreateExchangeOrder",
        ("/api/exchange/orders/{id}/confirm", "post") => "ConfirmOrderRequest",
        ("/api/exchange/orders/{id}/funding/confirm", "post") => "FundingConfirmRequest",
        ("/api/exchange/orders/{id}/proof", "post") => "SubmitProofRequest",
        ("/api/p2p/route-executions", "post") => "CreateRouteExecution",
        ("/api/p2p/route-executions/{id}/submissions", "post") => "SubmitRouteExecution",
        _ => "JsonObject",
    };
    schema_ref(name)
}

fn response_schema(path: &str, method: &str) -> Value {
    match (path, method) {
        ("/api/auth/register", "post")
        | ("/api/auth/login", "post")
        | ("/api/auth/oauth/{provider}", "post") => schema_ref("AuthToken"),
        ("/api/auth/me", "get") => schema_ref("User"),
        ("/api/anonymous/register", "post") => schema_ref("AnonymousUser"),
        ("/api/market-values", "post") => schema_ref("MarketValuesResponse"),
        ("/api/market-prices", "get") => schema_ref("MarketPricesResponse"),
        ("/api/referrals/me", "get") => schema_ref("ReferralProfile"),
        ("/api/payments", "post") | ("/api/payments/{id}", "get") => schema_ref("PaymentView"),
        ("/api/payments", "get") => array_schema("PaymentView"),
        ("/api/exchange/orders", "post")
        | ("/api/exchange/orders/{id}", "get")
        | ("/api/exchange/orders/{id}/cancel", "post") => schema_ref("ExchangeOrder"),
        ("/api/exchange/orders", "get") => array_schema("ExchangeOrder"),
        ("/api/exchange/orders/{id}/confirm", "post")
        | ("/api/exchange/orders/{id}/funding/confirm", "post") => schema_ref("JsonResponse"),
        ("/api/p2p/route-executions", "post")
        | ("/api/p2p/route-executions/{id}", "get")
        | ("/api/p2p/route-executions/{id}/submissions", "post") => schema_ref("RouteExecution"),
        ("/api/p2p/route-activity", "get") => json!({
            "type": "object",
            "required": ["source_currency", "target_currency", "hours"],
            "properties": {
                "source_currency": { "type": "string" },
                "target_currency": { "type": "string" },
                "hours": { "type": "array", "items": { "type": "object", "required": ["started_at", "count"], "properties": {
                    "started_at": { "type": "string", "format": "date-time" },
                    "count": { "type": "integer", "format": "int64", "minimum": 0 }
                } } }
            }
        }),
        _ => schema_ref("JsonResponse"),
    }
}

fn success_response(path: &str, method: &str) -> Value {
    match (path, method) {
        ("/health", "get") => json!({
            "description": "The backend is reachable.",
            "content": { "text/plain": { "schema": { "type": "string", "example": "ok" } } }
        }),
        ("/metrics", "get") => json!({
            "description": "Prometheus metrics.",
            "content": { "text/plain": { "schema": { "type": "string" } } }
        }),
        ("/api/exchange/orders/{id}/live", "get")
        | ("/ws", "get")
        | ("/ws/rates", "get")
        | ("/ws/payments", "get")
        | ("/ws/p2p/routes", "get") => json!({
            "description": "WebSocket upgrade. Connect with a WebSocket client to exchange JSON messages.",
            "headers": { "Upgrade": { "schema": { "type": "string", "example": "websocket" } } }
        }),
        _ => json!({
            "description": "Successful response",
            "content": { "application/json": { "schema": response_schema(path, method) } }
        }),
    }
}

fn extra_parameters(path: &str, method: &str) -> Vec<Value> {
    let mut parameters = Vec::new();
    if path == "/api/p2p/route-activity" && method == "get" {
        for name in ["source_currency", "target_currency"] {
            parameters.push(json!({
                "name": name, "in": "query", "required": true,
                "schema": { "type": "string", "minLength": 2, "maxLength": 12 }
            }));
        }
        parameters.push(json!({
            "name": "period", "in": "query", "required": false,
            "description": "Time range. Longer ranges use daily, weekly, or monthly buckets.",
            "schema": { "type": "string", "enum": ["1h", "1d", "1w", "1m", "3m", "6m", "1y", "all"], "default": "1w" }
        }));
    }
    if path == "/api/p2p/routes" && method == "get" {
        parameters.push(json!({
            "name": "count_activity", "in": "query", "required": false,
            "description": "Set true for a user-initiated search; false for automatic route refresh.",
            "schema": { "type": "boolean", "default": false }
        }));
    }
    if path == "/api/exchange/orders" && method == "get" {
        parameters.push(json!({
            "name": "limit",
            "in": "query",
            "description": "Maximum number of orders to return (1–100).",
            "required": false,
            "schema": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 }
        }));
    }
    if path == "/api/exchange/orders/{id}/live" && method == "get" {
        parameters.push(json!({
            "name": "access_token",
            "in": "query",
            "description": "Optional JWT for WebSocket clients that cannot send an Authorization header.",
            "required": false,
            "schema": { "type": "string" }
        }));
    }
    if path == "/api/payments" && method == "post"
        || path == "/api/exchange/orders" && method == "post"
        || path == "/api/p2p/route-executions" && method == "post"
    {
        parameters.push(json!({
            "name": "Idempotency-Key",
            "in": "header",
            "description": "Stable key used to safely retry a create request.",
            "required": path == "/api/p2p/route-executions",
            "schema": { "type": "string", "example": "payment-2026-09-26-001" }
        }));
    }
    if path == "/api/p2p/route-executions/{id}" && method == "get" {
        parameters.push(json!({
            "name": "anonymous_id",
            "in": "query",
            "description": "Pseudonymous owner ID used when the execution was created.",
            "required": true,
            "schema": { "type": "string", "format": "uuid" }
        }));
    }
    parameters
}

#[cfg(test)]
mod tests {
    use super::api_document;

    #[test]
    fn documents_auth_and_real_request_schemas() {
        let document = api_document();
        let login = &document["paths"]["/api/auth/login"]["post"];
        assert_eq!(
            login["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/AuthCodeRequest"
        );
        assert_eq!(
            login["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/AuthToken"
        );
        assert!(login["description"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert!(document["components"]["schemas"]["NewPayment"].is_object());
    }

    #[test]
    fn uses_admin_security_for_admin_operations() {
        let document = api_document();
        let operation = &document["paths"]["/api/admin/exchange/controls"]["post"];
        assert!(operation["security"][0]["adminBearerAuth"].is_array());
    }

    #[test]
    fn documents_referral_registration_and_profile() {
        let document = api_document();
        assert!(
            document["components"]["schemas"]["AuthCodeRequest"]["properties"]["referral_code"]
                .is_object()
        );
        assert_eq!(
            document["paths"]["/api/referrals/me"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/ReferralProfile"
        );
    }
}
