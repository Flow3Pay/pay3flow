use crate::activitypub::model::Proposal;

/// First acquirer entries loaded into fmatch as `purpose="offer"` proposals.
pub struct AcquirerSeed {
    pub slug: &'static str,
    pub name: &'static str,
    pub geo: &'static str,
    pub currencies: &'static str,
    pub fee: &'static str,
    pub limits: &'static str,
}

pub const ACQUIRERS: &[AcquirerSeed] = &[
    AcquirerSeed {
        slug: "stripe",
        name: "Stripe",
        geo: "US|EU|CA|AU|JP|SG",
        currencies: "USD|EUR|GBP|CAD|AUD|JPY|SGD",
        fee: "3.1% + 0.50 flat (card)",
        limits: "min 1, max 100000 USD",
    },
    AcquirerSeed {
        slug: "adyen",
        name: "Adyen",
        geo: "NL|EU|US|GB",
        currencies: "EUR|USD|GBP",
        fee: "2.9% + 0.30 flat (card)",
        limits: "min 1, max 500000 EUR",
    },
    AcquirerSeed {
        slug: "checkout",
        name: "Checkout.com",
        geo: "UK|EU|US",
        currencies: "EUR|USD|GBP",
        fee: "2.8% + 0.30 flat (card)",
        limits: "min 1, max 250000 USD",
    },
    AcquirerSeed {
        slug: "paypal",
        name: "PayPal",
        geo: "US|EU|GB|CA",
        currencies: "USD|EUR|GBP|CAD",
        fee: "3.49% + 0.49 flat",
        limits: "min 1, max 60000 USD",
    },
    AcquirerSeed {
        slug: "braintree",
        name: "Braintree",
        geo: "US|EU|GB|AU",
        currencies: "USD|EUR|GBP|AUD",
        fee: "2.9% + 0.30 flat (card)",
        limits: "min 1, max 200000 USD",
    },
    AcquirerSeed {
        slug: "mollie",
        name: "Mollie",
        geo: "NL|EU",
        currencies: "EUR",
        fee: "1.5% + 0.25 flat (SEPA/ideal)",
        limits: "min 1, max 50000 EUR",
    },
    AcquirerSeed {
        slug: "klarna",
        name: "Klarna",
        geo: "SE|EU|GB|US",
        currencies: "EUR|USD|GBP",
        fee: "2.99% + 0.30 flat",
        limits: "min 1, max 300000 EUR",
    },
    AcquirerSeed {
        slug: "payoneer",
        name: "Payoneer",
        geo: "US|EU|Global",
        currencies: "USD|EUR|GBP",
        fee: "2.5% + 0.30 flat",
        limits: "min 1, max 1000000 USD",
    },
    AcquirerSeed {
        slug: "wise",
        name: "Wise",
        geo: "UK|EU|US|Global",
        currencies: "USD|EUR|GBP|40+",
        fee: "0.99%..2.5% (fx+transfer)",
        limits: "min 1, max 1500000 USD",
    },
];

impl AcquirerSeed {
    pub fn to_proposal(&self, actor_id: &str, resource: &str) -> Proposal {
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
        for seed in ACQUIRERS {
            let p = seed.to_proposal(actor, resource);
            assert_eq!(p.purpose, "offer");
            assert!(p.name.contains(seed.name));
            assert!(p.content.contains("status=active"));
            assert_eq!(p.action, "deliverService");
            assert_eq!(p.resource_conforms_to, resource);
        }
    }

    #[test]
    fn ids_are_unique() {
        let actor = "https://pay3flow.local/actor/pay3flow";
        let resource = "https://pay3flow.local/marketplace/resources/acquiring";
        let ids = ACQUIRERS
            .iter()
            .map(|s| s.to_proposal(actor, resource).id)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), ACQUIRERS.len());
    }
}