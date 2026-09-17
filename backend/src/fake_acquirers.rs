use serde::Serialize;

use crate::activitypub::model::Proposal;

/// A currency pair an acquirer serves, with the commission it charges on it
/// (the template "Usd/Hkd commission 3,4% Usd/Aud commission 2,5%").
#[derive(Debug, Clone, Serialize)]
pub struct FakePair {
    pub from: &'static str,
    pub to: &'static str,
    /// Commission string, e.g. "3.4%".
    pub commission: &'static str,
}

/// A single fake endpoint of an acquirer's API layer. These are *invented*
/// API contracts our backend serves for each solver — nothing is real, all
/// requests/responses are example JSON returned by the fake API/adapters.
#[derive(Debug, Clone, Serialize)]
pub struct FakeEndpoint {
    pub method: &'static str,
    pub path: &'static str,
    pub description: &'static str,
    /// Example request body (JSON).
    pub request: &'static str,
    /// Example response body (JSON).
    pub response: &'static str,
}

/// The invented API layer of a fake acquirer. Represents "берём весь
/// необходимый API" — the contract our backend would use to actually
/// execute a payment with this solver (PLAN: acquirers' passports).
#[derive(Debug, Clone, Serialize)]
pub struct FakeApiLayer {
    /// Fake base URL of the solver's API.
    pub base_url: &'static str,
    /// How the solver authenticates us.
    pub auth: &'static str,
    /// Webhook signature secret scheme (fake).
    pub webhook_secret: &'static str,
    pub endpoints: &'static [FakeEndpoint],
}

/// A single fictional acquirer (solver) served by fmatch.
#[derive(Debug, Clone, Serialize)]
pub struct FakeAcquirer {
    pub slug: &'static str,
    pub name: &'static str,
    pub geo: &'static str,
    pub limits: &'static str,
    /// 0..1 — fed to fmatch rating as the `qualityScore` attachment.
    pub quality: f64,
    pub latency_ms: u64,
    pub capacity: u64,
    /// Per-pair commissions, the fmatch offer content.
    pub pairs: &'static [FakePair],
    pub api: FakeApiLayer,
}

impl FakeAcquirer {
    /// All currencies this acquirer touches (union of pair legs), in the
    /// `A|B|C` shape the routing profile parser expects.
    pub fn currencies_token(&self) -> String {
        use std::collections::BTreeSet;
        let mut set = BTreeSet::new();
        for p in self.pairs {
            set.insert(p.from);
            set.insert(p.to);
        }
        set.into_iter().collect::<Vec<_>>().join("|")
    }

    /// Worst-case pair commission, expressed as a single percent fee (PLAN: for
    /// ranges we take the worst case). Used by the local profile / fallback.
    pub fn worst_commission(&self) -> f64 {
        self.pairs
            .iter()
            .filter_map(|p| {
                p.commission
                    .trim_end_matches('%')
                    .trim()
                    .replace(',', ".")
                    .parse::<f64>()
                    .ok()
            })
            .fold(0.0_f64, f64::max)
    }

    /// The pair commission for a specific `from/to` route, if served.
    pub fn commission_for(&self, from: &str, to: &str) -> Option<&'static str> {
        self.pairs.iter().find(|p| {
            p.from.eq_ignore_ascii_case(from) && p.to.eq_ignore_ascii_case(to)
        }).map(|p| p.commission)
    }

    /// Build a `purpose="offer"` proposal advertising this acquirer into fmatch
    /// (each fake solver is its own actor). Content lists the per-pair fees.
    pub fn to_proposal(&self, actor_id: &str, resource: &str, inbox: &str) -> Proposal {
        let pairs: Vec<String> = self
            .pairs
            .iter()
            .map(|p| format!("{}/{}={}", p.from, p.to, p.commission))
            .collect();
        let content = format!(
            "acquirer={}; status=active; geo={}; pairs=[{}]; limits={}",
            self.name,
            self.geo,
            pairs.join(", "),
            self.limits
        );
        Proposal {
            id: format!("{actor_id}/acquirers/{}/offer", self.slug),
            purpose: "offer".into(),
            attributed_to: actor_id.into(),
            name: format!("{} ({}, {})", self.name, self.geo, self.worst_commission()),
            content,
            resource_conforms_to: resource.into(),
            action: "deliverService".into(),
            resource_unit: "one".into(),
            attachments: vec![
                crate::acquirer::rating_attachment("inbox", inbox),
                crate::acquirer::rating_attachment("provider", self.slug),
                crate::acquirer::rating_attachment("model", "fx"),
                crate::acquirer::rating_attachment("qualityScore", self.quality),
                crate::acquirer::rating_attachment("latencyMs", self.latency_ms),
                crate::acquirer::rating_attachment("capacity", self.capacity),
                crate::acquirer::rating_attachment("successCount", 100_u64),
                crate::acquirer::rating_attachment("failureCount", 0_u64),
            ],
        }
    }
}

/// 10 fictional acquirers (solvers). Nothing is a real brand — names, rates
/// and API layers are invented so the whole fmatch ↔ backend loop runs on
/// made-up data we fully control.
pub const FAKE_ACQUIRERS: &[FakeAcquirer] = &[
    FakeAcquirer {
        slug: "flinger",
        name: "Flinger Pay",
        geo: "US|EU|Global",
        limits: "min 5, max 250000 USD",
        quality: 0.86,
        latency_ms: 155,
        capacity: 82,
        pairs: &[
            FakePair { from: "USD", to: "HKD", commission: "3.4%" },
            FakePair { from: "USD", to: "AUD", commission: "2.5%" },
            FakePair { from: "USD", to: "JPY", commission: "3.0%" },
            FakePair { from: "EUR", to: "USD", commission: "1.8%" },
        ],
        api: FakeApiLayer {
            base_url: "https://api.flingerpay.test/v1",
            auth: "Authorization: Bearer fng_live_sk_8f3a...",
            webhook_secret: "whsec_flinger_91d2",
            endpoints: &[
                FakeEndpoint {
                    method: "POST",
                    path: "/quotes",
                    description: "Get a quote for an FX pair",
                    request: r#"{"from":"USD","to":"HKD","amount":15000}"#,
                    response: r#"{"quoteId":"q_01H","pair":"USD/HKD","rate":7.8120,"fee":510,"total":15720,"commission":"3.4%"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/payments",
                    description: "Create a payment (Idempotency-Key header)",
                    request: r#"{"quoteId":"q_01H","from_account":"card-4242","to_account":"hk-12345","currency":"USD","amount":15000}"#,
                    response: r#"{"id":"pay_fl_9A","status":"booked","external_ref":"FNG-2026-0001"}"#,
                },
                FakeEndpoint {
                    method: "GET",
                    path: "/payments/{id}",
                    description: "Poll payment status",
                    request: r#"{}"#,
                    response: r#"{"id":"pay_fl_9A","status":"settled","external_ref":"FNG-2026-0001"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/payments/{id}/refund",
                    description: "Refund a payment",
                    request: r#"{"amount":15000,"reason":"cancel"}"#,
                    response: r#"{"id":"pay_fl_9A","status":"refunded","refund_ref":"FNG-R-2026-001"}"#,
                },
            ],
        },
    },
    FakeAcquirer {
        slug: "warpgate",
        name: "WarpGate FX",
        geo: "US|EU|UK|Global",
        limits: "min 2, max 500000 EUR",
        quality: 0.90,
        latency_ms: 120,
        capacity: 95,
        pairs: &[
            FakePair { from: "EUR", to: "USD", commission: "1.4%" },
            FakePair { from: "EUR", to: "GBP", commission: "2.1%" },
            FakePair { from: "GBP", to: "USD", commission: "1.9%" },
            FakePair { from: "USD", to: "CHF", commission: "2.3%" },
        ],
        api: FakeApiLayer {
            base_url: "https://api.warpgatefx.test/v2",
            auth: "X-Api-Key: wg_live_4e7c...",
            webhook_secret: "whsec_warpgate_a1b2",
            endpoints: &[
                FakeEndpoint {
                    method: "POST",
                    path: "/fx/quote",
                    description: "Quote an FX conversion",
                    request: r#"{"base":"EUR","quote":"USD","amount":100}"#,
                    response: r#"{"quoteId":"wg_qC2","rate":1.0840,"feeMinor":1.4,"commission":"1.4%"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/fx/payments",
                    description: "Execute a conversion",
                    request: r#"{"quoteId":"wg_qC2","payout":{"iban":"DE...","currency":"EUR"}}"#,
                    response: r#"{"paymentId":"wg_p77","status":"processing","trace":"WG-T-2026-12"}"#,
                },
                FakeEndpoint {
                    method: "GET",
                    path: "/fx/payments/{paymentId}",
                    description: "Status of a conversion",
                    request: r#"{}"#,
                    response: r#"{"paymentId":"wg_p77","status":"settled","trace":"WG-T-2026-12"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/fx/payments/{paymentId}/cancel",
                    description: "Cancel an un-settled conversion",
                    request: r#"{"reason":"duplicate"}"#,
                    response: r#"{"paymentId":"wg_p77","status":"cancelled"}"#,
                },
            ],
        },
    },
    FakeAcquirer {
        slug: "corvus",
        name: "Corvus Exchange",
        geo: "Global",
        limits: "min 1, max 1000000 USD",
        quality: 0.82,
        latency_ms: 210,
        capacity: 70,
        pairs: &[
            FakePair { from: "USD", to: "CNY", commission: "2.2%" },
            FakePair { from: "EUR", to: "CNY", commission: "2.8%" },
            FakePair { from: "USD", to: "INR", commission: "2.6%" },
            FakePair { from: "EUR", to: "USD", commission: "1.6%" },
        ],
        api: FakeApiLayer {
            base_url: "https://api.corvus-exchange.test",
            auth: "X-Corvus-Key: cv_live_c9d1...",
            webhook_secret: "whsec_corvus_e3f4",
            endpoints: &[
                FakeEndpoint {
                    method: "POST",
                    path: "/api/v1/rate",
                    description: "Request an indicative rate",
                    request: r#"{"from":"USD","to":"CNY","amount":50000}"#,
                    response: r#"{"rateId":"cv_r09","rate":6.9050,"markup":"2.2%"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/api/v1/transfer",
                    description: "Create a transfer",
                    request: r#"{"rateId":"cv_r09","source_account":"req_1","destination":"alipay-ccn"}"#,
                    response: r#"{"transferId":"cv_t314","status":"created","ref":"CV-26-000982"}"#,
                },
                FakeEndpoint {
                    method: "GET",
                    path: "/api/v1/transfer/{transferId}",
                    description: "Transfer status",
                    request: r#"{}"#,
                    response: r#"{"transferId":"cv_t314","status":"completed","ref":"CV-26-000982"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/api/v1/transfer/{transferId}/reversal",
                    description: "Reverse a transfer",
                    request: r#"{"amount":50000}"#,
                    response: r#"{"transferId":"cv_t314","status":"reversed"}"#,
                },
            ],
        },
    },
    FakeAcquirer {
        slug: "vormir",
        name: "Vormir FX",
        geo: "UK|EU|Global",
        limits: "min 10, max 400000 GBP",
        quality: 0.88,
        latency_ms: 140,
        capacity: 78,
        pairs: &[
            FakePair { from: "GBP", to: "USD", commission: "1.7%" },
            FakePair { from: "EUR", to: "GBP", commission: "2.0%" },
            FakePair { from: "GBP", to: "INR", commission: "2.9%" },
            FakePair { from: "USD", to: "GBP", commission: "1.9%" },
        ],
        api: FakeApiLayer {
            base_url: "https://api.vormirfx.test/gbp",
            auth: "X-Vormir-Token: vm_live_55ab...",
            webhook_secret: "whsec_vormir_b7c8",
            endpoints: &[
                FakeEndpoint {
                    method: "POST",
                    path: "/quote",
                    description: "GBP quote",
                    request: r#"{"sell":"GBP","buy":"USD","amount":2500}"#,
                    response: r#"{"quoteId":"vm_qX","rate":1.2654,"feePercent":"1.7%"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/orders",
                    description: "Place an FX order",
                    request: r#"{"quoteId":"vm_qX","bank":{"sort_code":"40-31-24","account":"12345678"}}"#,
                    response: r#"{"orderId":"vm_o21","status":"placed","ordRef":"VM-2026-551"}"#,
                },
                FakeEndpoint {
                    method: "GET",
                    path: "/orders/{orderId}",
                    description: "Order status",
                    request: r#"{}"#,
                    response: r#"{"orderId":"vm_o21","status":"paid_out","ordRef":"VM-2026-551"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/orders/{orderId}/recall",
                    description: "Recall an order",
                    request: r#"{}"#,
                    response: r#"{"orderId":"vm_o21","status":"recalled"}"#,
                },
            ],
        },
    },
    FakeAcquirer {
        slug: "helixpay",
        name: "HelixPay",
        geo: "US|EU|Global",
        limits: "min 5, max 300000 USD",
        quality: 0.79,
        latency_ms: 250,
        capacity: 60,
        pairs: &[
            FakePair { from: "USD", to: "BRL", commission: "3.3%" },
            FakePair { from: "EUR", to: "BRL", commission: "3.6%" },
            FakePair { from: "USD", to: "MXN", commission: "2.9%" },
            FakePair { from: "EUR", to: "USD", commission: "1.5%" },
        ],
        api: FakeApiLayer {
            base_url: "https://api.helixpay.test/v1.1",
            auth: "X-Helix-Key: hx_live_dc44...",
            webhook_secret: "whsec_helix_9f1e",
            endpoints: &[
                FakeEndpoint {
                    method: "POST",
                    path: "/latam/quote",
                    description: "LatAm-payout quote",
                    request: r#"{"from":"USD","to":"BRL","amount":1000}"#,
                    response: r#"{"quoteId":"hx_q4","rate":5.18,"fxFee":"3.3%"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/latam/remittance",
                    description: "Create a remittance",
                    request: r#"{"quoteId":"hx_q4","recipient":{"cpf":"00000000000","name":"Maria"}}"#,
                    response: r#"{"remitId":"hx_r78","status":"accepted","ref":"HX-2026-00422"}"#,
                },
                FakeEndpoint {
                    method: "GET",
                    path: "/latam/remittance/{remitId}",
                    description: "Remittance status",
                    request: r#"{}"#,
                    response: r#"{"remitId":"hx_r78","status":"paid","ref":"HX-2026-00422"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/latam/remittance/{remitId}/pullback",
                    description: "Pull back a remittance",
                    request: r#"{"reason":"wrong_cpf"}"#,
                    response: r#"{"remitId":"hx_r78","status":"pullback_requested"}"#,
                },
            ],
        },
    },
    FakeAcquirer {
        slug: "quicksilver",
        name: "Quicksilver Transfers",
        geo: "AU|SG|Global",
        limits: "min 3, max 200000 USD",
        quality: 0.92,
        latency_ms: 105,
        capacity: 90,
        pairs: &[
            FakePair { from: "USD", to: "SGD", commission: "1.8%" },
            FakePair { from: "EUR", to: "SGD", commission: "2.4%" },
            FakePair { from: "USD", to: "AUD", commission: "2.1%" },
            FakePair { from: "AUD", to: "USD", commission: "2.0%" },
        ],
        api: FakeApiLayer {
            base_url: "https://gate.quicksilvertx.test",
            auth: "X-QS-Key: qs_live_7aaa...",
            webhook_secret: "whsec_quicksilver_2d3c",
            endpoints: &[
                FakeEndpoint {
                    method: "POST",
                    path: "/v1/pricing",
                    description: "Pricing for APAC payout",
                    request: r#"{"from":"USD","to":"SGD","amount":800}"#,
                    response: r#"{"pricingId":"qs_p1","rate":1.3412,"sgdFee":1.44,"fee":"1.8%"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/v1/payouts",
                    description: "Create a fast payout",
                    request: r#"{"pricingId":"qs_p1","beneficiary":"DBS-001-002-003","currency":"SGD"}"#,
                    response: r#"{"payoutId":"qs_x55","status":"fast_path","ref":"QS-2026-9001"}"#,
                },
                FakeEndpoint {
                    method: "GET",
                    path: "/v1/payouts/{payoutId}",
                    description: "Payout status",
                    request: r#"{}"#,
                    response: r#"{"payoutId":"qs_x55","status":"credited","ref":"QS-2026-9001"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/v1/payouts/{payoutId}/revoke",
                    description: "Revoke a not-yet-credited payout",
                    request: r#"{"reason":"beneficiary_error"}"#,
                    response: r#"{"payoutId":"qs_x55","status":"revoked"}"#,
                },
            ],
        },
    },
    FakeAcquirer {
        slug: "tessera",
        name: "Tessera Pay",
        geo: "EU|Global",
        limits: "min 5, max 400000 EUR",
        quality: 0.85,
        latency_ms: 175,
        capacity: 72,
        pairs: &[
            FakePair { from: "USD", to: "NOK", commission: "2.5%" },
            FakePair { from: "EUR", to: "NOK", commission: "2.7%" },
            FakePair { from: "USD", to: "SEK", commission: "2.4%" },
            FakePair { from: "EUR", to: "SEK", commission: "2.6%" },
        ],
        api: FakeApiLayer {
            base_url: "https://api.tesserapay.test/nordic",
            auth: "X-Tessera-Key: ts_live_ee82...",
            webhook_secret: "whsec_tessera_6a7b",
            endpoints: &[
                FakeEndpoint {
                    method: "POST",
                    path: "/quote",
                    description: "Nordic-payout quote",
                    request: r#"{"from":"EUR","to":"NOK","amount":250000}"#,
                    response: r#"{"quoteId":"ts_qc","rate":11.32,"feePercent":"2.7%"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/payments",
                    description: "Create a Nordic payout",
                    request: r#"{"quoteId":"ts_qc","beneficiary":{"norwegianBankAccount":"NO9386"}}"#,
                    response: r#"{"id":"ts_pm9","status":"created","ref":"TS-26-771"}"#,
                },
                FakeEndpoint {
                    method: "GET",
                    path: "/payments/{id}",
                    description: "Payment status",
                    request: r#"{}"#,
                    response: r#"{"id":"ts_pm9","status":"cleared","ref":"TS-26-771"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/payments/{id}/refund",
                    description: "Refund a payment",
                    request: r#"{"amount":250000}"#,
                    response: r#"{"id":"ts_pm9","status":"refunded"}"#,
                },
            ],
        },
    },
    FakeAcquirer {
        slug: "bramba",
        name: "Bramba Global",
        geo: "Global",
        limits: "min 2, max 600000 USD",
        quality: 0.77,
        latency_ms: 280,
        capacity: 88,
        pairs: &[
            FakePair { from: "USD", to: "TRY", commission: "3.9%" },
            FakePair { from: "EUR", to: "TRY", commission: "4.1%" },
            FakePair { from: "USD", to: "AED", commission: "2.3%" },
            FakePair { from: "EUR", to: "AED", commission: "2.6%" },
        ],
        api: FakeApiLayer {
            base_url: "https://api.bramba.test/gateway",
            auth: "X-Bramba-Key: br_live_11ac...",
            webhook_secret: "whsec_bramba_3c4d",
            endpoints: &[
                FakeEndpoint {
                    method: "POST",
                    path: "/v2/quote",
                    description: "Quote for GCC/Turkish payout",
                    request: r#"{"from":"USD","to":"TRY","amount":120000}"#,
                    response: r#"{"quoteId":"br_q2","rate":34.2,"feePercent":"3.9%"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/v2/orders",
                    description: "Create a payout order",
                    request: r#"{"quoteId":"br_q2","beneficiary":{"iban":"AE07..."}}"#,
                    response: r#"{"orderId":"br_o7","status":"accepted","ref":"BR-2026-118"}"#,
                },
                FakeEndpoint {
                    method: "GET",
                    path: "/v2/orders/{orderId}",
                    description: "Order status",
                    request: r#"{}"#,
                    response: r#"{"orderId":"br_o7","status":"completed","ref":"BR-2026-118"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/v2/orders/{orderId}/void",
                    description: "Void an order",
                    request: r#"{"reason":"fraud_review"}"#,
                    response: r#"{"orderId":"br_o7","status":"voided"}"#,
                },
            ],
        },
    },
    FakeAcquirer {
        slug: "okto",
        name: "Okto FX",
        geo: "EU|Global",
        limits: "min 1, max 150000 EUR",
        quality: 0.93,
        latency_ms: 95,
        capacity: 98,
        pairs: &[
            FakePair { from: "USD", to: "PLN", commission: "1.9%" },
            FakePair { from: "EUR", to: "PLN", commission: "1.5%" },
            FakePair { from: "USD", to: "CZK", commission: "2.2%" },
            FakePair { from: "EUR", to: "CZK", commission: "1.8%" },
        ],
        api: FakeApiLayer {
            base_url: "https://api.oktofx.test/v1",
            auth: "X-Okto-Key: ok_live_77de...",
            webhook_secret: "whsec_okto_e1f2",
            endpoints: &[
                FakeEndpoint {
                    method: "POST",
                    path: "/rates",
                    description: "CEE-market rate",
                    request: r#"{"from":"EUR","to":"PLN","amount":500}"#,
                    response: r#"{"rateId":"ok_r4","rate":4.31,"feePercent":"1.5%"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/transfers",
                    description: "Create a CEE transfer",
                    request: r#"{"rateId":"ok_r4","recipient_iban":"PL6110901014..."}"#,
                    response: r#"{"transferId":"ok_t12","status":"booked","ref":"OK-2026-009"}"#,
                },
                FakeEndpoint {
                    method: "GET",
                    path: "/transfers/{transferId}",
                    description: "Transfer status",
                    request: r#"{}"#,
                    response: r#"{"transferId":"ok_t12","status":"settled","ref":"OK-2026-009"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/transfers/{transferId}/refund",
                    description: "Refund a transfer",
                    request: r#"{"amount":500}"#,
                    response: r#"{"transferId":"ok_t12","status":"refunded"}"#,
                },
            ],
        },
    },
    FakeAcquirer {
        slug: "vanda",
        name: "Vanda Move",
        geo: "US|EU|UK|CA|Global",
        limits: "min 2, max 800000 USD",
        quality: 0.91,
        latency_ms: 110,
        capacity: 85,
        pairs: &[
            FakePair { from: "USD", to: "GBP", commission: "1.6%" },
            FakePair { from: "EUR", to: "USD", commission: "1.3%" },
            FakePair { from: "GBP", to: "EUR", commission: "1.7%" },
            FakePair { from: "USD", to: "CAD", commission: "1.9%" },
        ],
        api: FakeApiLayer {
            base_url: "https://move.vanda.test/core",
            auth: "X-Vanda-Key: vd_live_42ab...",
            webhook_secret: "whsec_vanda_4b5c",
            endpoints: &[
                FakeEndpoint {
                    method: "POST",
                    path: "/quote",
                    description: "Core-market quote",
                    request: r#"{"from":"USD","to":"CAD","amount":1000}"#,
                    response: r#"{"quoteId":"vd_q3","rate":1.3551,"feePercent":"1.9%"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/payments",
                    description: "Create a payment",
                    request: r#"{"quoteId":"vd_q3","beneficiary":{"acct":"CA-..."}}"#,
                    response: r#"{"paymentId":"vd_p8","status":"queued","ref":"VD-2026-033"}"#,
                },
                FakeEndpoint {
                    method: "GET",
                    path: "/payments/{paymentId}",
                    description: "Payment status",
                    request: r#"{}"#,
                    response: r#"{"paymentId":"vd_p8","status":"delivered","ref":"VD-2026-033"}"#,
                },
                FakeEndpoint {
                    method: "POST",
                    path: "/payments/{paymentId}/refund",
                    description: "Refund a payment",
                    request: r#"{"amount":1000}"#,
                    response: r#"{"paymentId":"vd_p8","status":"refunded"}"#,
                },
            ],
        },
    },
];

/// Find a fake acquirer by slug.
pub fn fake_acquirer_by_slug(slug: &str) -> Option<&'static FakeAcquirer> {
    FAKE_ACQUIRERS
        .iter()
        .find(|a| a.slug.eq_ignore_ascii_case(slug))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_ten_fake_solvers() {
        assert_eq!(FAKE_ACQUIRERS.len(), 10);
    }

    #[test]
    fn slugs_are_unique() {
        let mut slugs = FAKE_ACQUIRERS.iter().map(|a| a.slug).collect::<Vec<_>>();
        let len = slugs.len();
        slugs.sort();
        slugs.dedup();
        assert_eq!(slugs.len(), len);
    }

    #[test]
    fn every_solver_has_pairs_and_commissions() {
        for a in FAKE_ACQUIRERS {
            assert!(!a.pairs.is_empty(), "{} has no pairs", a.slug);
            for p in a.pairs {
                assert!(
                    p.commission.ends_with('%'),
                    "{}: commission {} lacks %",
                    a.slug,
                    p.commission
                );
            }
        }
    }

    #[test]
    fn worst_commission_is_parsed() {
        assert_eq!(
            FAKE_ACQUIRERS
                .iter()
                .find(|a| a.slug == "flinger")
                .unwrap()
                .worst_commission(),
            3.4
        );
    }

    #[test]
    fn commission_for_pair() {
        let f = fake_acquirer_by_slug("flinger").unwrap();
        assert_eq!(f.commission_for("usd", "hkd"), Some("3.4%"));
        assert_eq!(f.commission_for("EUR", "AUD"), None);
    }

    #[test]
    fn proposals_are_valid_offers() {
        let actor = "http://localhost:9090/actor/flinger";
        let resource = "http://localhost:7277/marketplace/resources/acquiring";
        let inbox = "http://localhost:9090/inbox/flinger";
        for a in FAKE_ACQUIRERS {
            let p = a.to_proposal(actor, resource, inbox);
            assert_eq!(p.purpose, "offer");
            assert!(p.content.contains(&format!("pairs=[{}", a.pairs[0].from)));
            assert!(p.attachments.iter().any(|at| at["name"] == "qualityScore"));
        }
    }

    #[test]
    fn currencies_token_is_deduped() {
        let flinger = fake_acquirer_by_slug("flinger").unwrap();
        let currency_str = flinger.currencies_token();
        let tokens: Vec<&str> = currency_str.split('|').collect();
        assert!(tokens.contains(&"HKD"));
        assert!(tokens.contains(&"USD"));
        assert!(tokens.contains(&"EUR"));
    }
}