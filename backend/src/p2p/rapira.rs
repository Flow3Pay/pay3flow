use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::p2p::service::{Advertiser, P2pOffer, P2pSearchQuery, P2pSide, P2pSource};

const MAX_RAPIRA_ROWS: usize = 100;
const RAPIRA_BASE_URL: &str = "https://rapira.net";

pub(crate) struct RapiraP2pSource {
    client: reqwest::Client,
    url: String,
}

impl RapiraP2pSource {
    pub(crate) fn new(client: reqwest::Client, url: String) -> Self {
        Self { client, url }
    }
}

#[async_trait]
impl P2pSource for RapiraP2pSource {
    fn name(&self) -> &'static str {
        "rapira"
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        if query.fiat != "RUB" || query.asset != "USDT" {
            bail!("Rapira P2P currently supports only the USDT/RUB market");
        }
        let request = RapiraRequest {
            listing_type: "RECOMMENDED",
            merchant_side: match query.side {
                // Rapira's merchant side is the opposite of the user's side.
                P2pSide::BuyCrypto => "SELL",
                P2pSide::SellCrypto => "BUY",
            },
            internal_coin_unit: &query.asset,
            external_coin_unit: &query.fiat,
            payment_ids: "",
            show_only_eligible: false,
            sort_by_number_of_mutual_orders: false,
            amount: query.amount.map(|amount| amount.to_string()).unwrap_or_default(),
            page_no: 1,
            page_size: query.fetch_limit().min(MAX_RAPIRA_ROWS),
        };
        let response = self
            .client
            .get(&self.url)
            .header("origin", "https://rapira.net")
            .header("referer", "https://rapira.net/ru/p2p/BUY?p=1")
            .query(&request)
            .send()
            .await
            .context("Rapira P2P request failed")?
            .error_for_status()
            .context("Rapira P2P returned an HTTP error")?
            .json::<RapiraResponse>()
            .await
            .context("invalid Rapira P2P response")?;

        if let Some(code) = response.code {
            if code != 0 {
                bail!(
                    "Rapira P2P error {}: {}",
                    code,
                    response
                        .message
                        .unwrap_or_else(|| "unknown error".into())
                );
            }
        }

        Ok(response
            .content
            .unwrap_or_default()
            .into_iter()
            .map(|ad| ad.into_offer(query.side))
            .collect())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RapiraRequest<'a> {
    listing_type: &'a str,
    merchant_side: &'a str,
    internal_coin_unit: &'a str,
    external_coin_unit: &'a str,
    payment_ids: &'a str,
    show_only_eligible: bool,
    sort_by_number_of_mutual_orders: bool,
    amount: String,
    page_no: u32,
    page_size: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RapiraResponse {
    #[serde(default)]
    content: Option<Vec<RapiraAd>>,
    code: Option<i64>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RapiraAd {
    advertise_id: u64,
    merchant: RapiraMerchant,
    price: f64,
    min_limit: f64,
    max_limit: f64,
    quantity: Option<f64>,
    time_limit: Option<u32>,
    internal_coin_unit: String,
    external_coin_unit: String,
    #[serde(default)]
    payment_types: Vec<RapiraPaymentType>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RapiraMerchant {
    profile_uid: Option<String>,
    username: String,
    p2p_level: Option<String>,
    total_terminated_after_accept_count: Option<u64>,
    total_completed_percent: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RapiraPaymentType {
    payment_name: String,
}

impl RapiraAd {
    fn into_offer(self, side: P2pSide) -> P2pOffer {
        let fiat = self.external_coin_unit;
        let asset = self.internal_coin_unit;
        let advertiser_profile_url = self
            .merchant
            .profile_uid
            .as_deref()
            .map(|profile_uid| {
                format!(
                    "{RAPIRA_BASE_URL}/ru/p2p/profileUser?p=1&profileUid={profile_uid}&msp=1"
                )
            });
        let is_verified = self.merchant.p2p_level.as_deref().is_some_and(|level| {
            matches!(
                level.to_ascii_lowercase().as_str(),
                "verified" | "trusted" | "premium"
            )
        });

        P2pOffer {
            source: "rapira".into(),
            ad_id: self.advertise_id.to_string(),
            side,
            fiat,
            asset,
            price: number_to_string(self.price),
            available_asset: self
                .quantity
                .map(number_to_string)
                .unwrap_or_else(|| "0".into()),
            min_fiat: number_to_string(self.min_limit),
            max_fiat: number_to_string(self.max_limit),
            payment_methods: self
                .payment_types
                .into_iter()
                .map(|payment| payment.payment_name)
                .collect(),
            pay_time_limit_minutes: self.time_limit,
            advertiser: Advertiser {
                id: self.merchant.profile_uid,
                nickname: self.merchant.username,
                user_type: self.merchant.p2p_level,
                is_merchant: true,
                is_verified,
                completed_orders_30d: self.merchant.total_terminated_after_accept_count,
                completion_rate_30d: self
                    .merchant
                    .total_completed_percent
                    .and_then(|percent| (0.0..=100.0).contains(&percent).then_some(percent / 100.0)),
                positive_rate: None,
            },
            advertiser_profile_url,
            source_url: format!("{RAPIRA_BASE_URL}/p2p?adId={}", self.advertise_id),
            source_url_is_exact: true,
        }
    }
}

fn number_to_string(value: f64) -> String {
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_public_ad_and_maps_user_side() {
        let response: RapiraResponse = serde_json::from_str(
            r#"{
              "content":[{
                "merchant":{"profileUid":"profile-1","username":"Trader",
                  "p2pLevel":"REGULAR","totalTerminatedAfterAcceptCount":749,
                  "totalCompletedPercent":95.6},
                "advertiseId":52834,"merchantSide":"SELL","internalCoinUnit":"USDT",
                "externalCoinUnit":"RUB","price":89.2,"merchantFee":0.0093,
                "minLimit":15000,"maxLimit":52449.6,"quantity":588,"timeLimit":15,
                "paymentTypes":[{"id":5,"category":"CARDS","paymentName":"T-Card"}]
              }]
            }"#,
        )
        .unwrap();
        let offer = response
            .content
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .into_offer(P2pSide::BuyCrypto);

        assert_eq!(offer.source, "rapira");
        assert_eq!(offer.price, "89.2");
        assert_eq!(offer.payment_methods, ["T-Card"]);
        assert_eq!(offer.advertiser.completed_orders_30d, Some(749));
        assert!((offer.advertiser.completion_rate_30d.unwrap() - 0.956).abs() < 1e-12);
        assert_eq!(
            offer.advertiser_profile_url.as_deref(),
            Some("https://rapira.net/ru/p2p/profileUser?p=1&profileUid=profile-1&msp=1")
        );
        assert_eq!(offer.source_url, "https://rapira.net/p2p?adId=52834");
        assert!(offer.source_url_is_exact);
    }
}
