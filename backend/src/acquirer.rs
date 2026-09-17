use crate::activitypub::model::Proposal;

/// First acquirer entries loaded into fmatch as `purpose="offer"` proposals.
pub struct AcquirerSeed {
    pub slug: &'static str,
    pub name: &'static str,
    pub geo: &'static str,
    pub currencies: &'static str,
    pub fee: &'static str,
    pub limits: &'static str,
    /// 0..1 — fed to fmatch rating as the `qualityScore` attachment.
    pub quality: f64,
    /// milliseconds — fed to fmatch rating as the `latencyMs` attachment.
    pub latency_ms: u64,
    /// concurrent capacity unit — `capacity` attachment.
    pub capacity: u64,
}

pub const ACQUIRERS: &[AcquirerSeed] = &[
    AcquirerSeed {
        slug: "stripe",
        name: "Stripe",
        geo: "US|EU|CA|AU|JP|SG",
        currencies: "USD|EUR|GBP|CAD|AUD|JPY|SGD",
        fee: "3.1% + 0.50 flat (card)",
        limits: "min 1, max 100000 USD",
        quality: 0.90,
        latency_ms: 180,
        capacity: 100,
    },
    AcquirerSeed {
        slug: "adyen",
        name: "Adyen",
        geo: "NL|EU|US|GB",
        currencies: "EUR|USD|GBP",
        fee: "2.9% + 0.30 flat (card)",
        limits: "min 1, max 500000 EUR",
        quality: 0.89,
        latency_ms: 200,
        capacity: 90,
    },
    AcquirerSeed {
        slug: "checkout",
        name: "Checkout.com",
        geo: "UK|EU|US",
        currencies: "EUR|USD|GBP",
        fee: "2.8% + 0.30 flat (card)",
        limits: "min 1, max 250000 USD",
        quality: 0.91,
        latency_ms: 220,
        capacity: 80,
    },
    AcquirerSeed {
        slug: "paypal",
        name: "PayPal",
        geo: "US|EU|GB|CA",
        currencies: "USD|EUR|GBP|CAD",
        fee: "3.49% + 0.49 flat",
        limits: "min 1, max 60000 USD",
        quality: 0.85,
        latency_ms: 160,
        capacity: 70,
    },
    AcquirerSeed {
        slug: "braintree",
        name: "Braintree",
        geo: "US|EU|GB|AU",
        currencies: "USD|EUR|GBP|AUD",
        fee: "2.9% + 0.30 flat (card)",
        limits: "min 1, max 200000 USD",
        quality: 0.90,
        latency_ms: 210,
        capacity: 75,
    },
    AcquirerSeed {
        slug: "mollie",
        name: "Mollie",
        geo: "NL|EU",
        currencies: "EUR",
        fee: "1.5% + 0.25 flat (SEPA/ideal)",
        limits: "min 1, max 50000 EUR",
        quality: 0.92,
        latency_ms: 120,
        capacity: 60,
    },
    AcquirerSeed {
        slug: "klarna",
        name: "Klarna",
        geo: "SE|EU|GB|US",
        currencies: "EUR|USD|GBP",
        fee: "2.99% + 0.30 flat",
        limits: "min 1, max 300000 EUR",
        quality: 0.88,
        latency_ms: 250,
        capacity: 65,
    },
    AcquirerSeed {
        slug: "payoneer",
        name: "Payoneer",
        geo: "US|EU|Global",
        currencies: "USD|EUR|GBP",
        fee: "2.5% + 0.30 flat",
        limits: "min 1, max 1000000 USD",
        quality: 0.87,
        latency_ms: 240,
        capacity: 85,
    },
    AcquirerSeed {
        slug: "wise",
        name: "Wise",
        geo: "UK|EU|US|Global",
        currencies: "USD|EUR|GBP|40+",
        fee: "0.99%..2.5% (fx+transfer)",
        limits: "min 1, max 1500000 USD",
        quality: 0.93,
        latency_ms: 100,
        capacity: 95,
    },
];

pub fn rating_attachment(name: &str, value: impl Into<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({ "type": "PropertyValue", "name": name, "value": value.into() })
}

impl AcquirerSeed {
    /// Build a `purpose="offer"` proposal advertising `inbox` as the delivery
    /// target plus the rating fields fmatch consumes during candidate scoring.
    pub fn to_proposal(&self, actor_id: &str, resource: &str, inbox: &str) -> Proposal {
        Proposal {
            id: format!("{actor_id}/acquirers/{}/offer", self.slug),
            purpose: "offer".into(),
            attributed_to: actor_id.into(),
            name: format!("{} ({}, {})", self.name, self.geo, self.fee),
            content: format!(
                "acquirer={}; status=active; geo={}; currencies={}; fee={}; limits={}",
                self.name, self.geo, self.currencies, self.fee, self.limits
            ),
            resource_conforms_to: resource.into(),
            action: "deliverService".into(),
            resource_unit: "one".into(),
            attachments: vec![
                rating_attachment("inbox", inbox),
                rating_attachment("provider", self.slug),
                rating_attachment("model", "card"),
                rating_attachment("qualityScore", self.quality),
                rating_attachment("latencyMs", self.latency_ms),
                rating_attachment("capacity", self.capacity),
                rating_attachment("successCount", 100_u64),
                rating_attachment("failureCount", 0_u64),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_are_valid_proposals() {
        let actor = "https://pay3flow.local/actor/pay3flow";
        let resource = "https://pay3flow.local/marketplace/resources/acquiring";
        let inbox = "https://pay3flow.local/inbox";
        for seed in ACQUIRERS {
            let p = seed.to_proposal(actor, resource, inbox);
            assert_eq!(p.purpose, "offer");
            assert!(p.name.contains(seed.name));
            assert!(p.content.contains("status=active"));
            assert_eq!(p.action, "deliverService");
            assert_eq!(p.resource_conforms_to, resource);
            assert!(p
                .attachments
                .iter()
                .any(|a| a["name"] == "inbox" && a["value"] == inbox));
            assert!(p.attachments.iter().any(|a| a["name"] == "qualityScore"));
        }
    }

    #[test]
    fn ids_are_unique() {
        let actor = "https://pay3flow.local/actor/pay3flow";
        let resource = "https://pay3flow.local/marketplace/resources/acquiring";
        let inbox = "https://pay3flow.local/inbox";
        let ids = ACQUIRERS
            .iter()
            .map(|s| s.to_proposal(actor, resource, inbox).id)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), ACQUIRERS.len());
    }
}
