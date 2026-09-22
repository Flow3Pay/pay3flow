use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::Deserialize;

use crate::p2p::service::{Advertiser, P2pOffer, P2pSearchQuery, P2pSide, P2pSource};

pub(crate) struct OkxP2pSource {
    client: reqwest::Client,
    url: String,
    timeout: std::time::Duration,
}

impl OkxP2pSource {
    pub(crate) fn new(
        client: reqwest::Client,
        url: String,
        timeout: std::time::Duration,
    ) -> Self {
        Self {
            client,
            url,
            timeout,
        }
    }
}

#[async_trait]
impl P2pSource for OkxP2pSource {
    fn name(&self) -> &'static str {
        "okx"
    }

    fn timeout(&self, _default: std::time::Duration) -> std::time::Duration {
        self.timeout
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        // OKX describes the advertisement owner: `sell` is an ad selling
        // crypto to our user, while `buy` is an ad buying crypto from them.
        let market_side = match query.side {
            P2pSide::BuyCrypto => "sell",
            P2pSide::SellCrypto => "buy",
        };
        let response = self
            .client
            .get(&self.url)
            .header("origin", "https://www.okx.com")
            .header("referer", "https://www.okx.com/p2p-markets/")
            .query(&[
                ("quoteCurrency", query.fiat.as_str()),
                ("baseCurrency", query.asset.as_str()),
                ("side", market_side),
                ("paymentMethod", "all"),
                ("userType", "all"),
                ("showTrade", "false"),
                ("showFollow", "false"),
                ("showAlreadyTraded", "false"),
                ("isAbleFilter", "false"),
                ("urlId", "0"),
            ])
            .send()
            .await
            .context("OKX P2P request failed")?
            .error_for_status()
            .context("OKX P2P returned an HTTP error")?
            .json::<OkxResponse>()
            .await
            .context("invalid OKX P2P response")?;
        if response.code != 0 {
            bail!(
                "OKX P2P error {}: {}",
                response.code,
                response
                    .detail_msg
                    .or(response.msg)
                    .unwrap_or_else(|| "unknown error".into())
            );
        }
        let data = response.data.unwrap_or_default();
        let ads = match query.side {
            P2pSide::BuyCrypto => data.sell,
            P2pSide::SellCrypto => data.buy,
        };
        Ok(ads
            .into_iter()
            .take(query.fetch_limit())
            .map(|ad| ad.into_offer(query.side))
            .collect())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OkxResponse {
    code: i64,
    data: Option<OkxData>,
    msg: Option<String>,
    detail_msg: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct OkxData {
    #[serde(default)]
    buy: Vec<OkxAd>,
    #[serde(default)]
    sell: Vec<OkxAd>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OkxAd {
    id: String,
    public_user_id: Option<String>,
    merchant_id: Option<String>,
    nick_name: String,
    base_currency: String,
    quote_currency: String,
    price: String,
    available_amount: String,
    quote_min_amount_per_order: String,
    quote_max_amount_per_order: String,
    #[serde(default)]
    payment_methods: Vec<String>,
    payment_timeout_minutes: Option<u32>,
    user_type: Option<String>,
    completed_order_quantity: Option<u64>,
    completed_rate: Option<String>,
    pos_review_percentage: Option<String>,
    #[serde(default)]
    is_institution: u8,
}

impl OkxAd {
    fn into_offer(self, side: P2pSide) -> P2pOffer {
        let is_merchant = self.is_institution > 0
            || self.merchant_id.as_deref().is_some_and(|id| !id.is_empty())
            || self
                .user_type
                .as_deref()
                .is_some_and(|kind| kind.eq_ignore_ascii_case("merchant"));
        let fiat = self.quote_currency;
        let asset = self.base_currency;
        let advertiser_profile_url = self
            .public_user_id
            .as_deref()
            .map(|user_id| format!("https://www.okx.com/p2p/ads-merchant?publicUserId={user_id}"));
        P2pOffer {
            source: "okx".into(),
            source_url: format!(
                "https://www.okx.com/p2p-markets/{}/{}-{}",
                fiat.to_ascii_lowercase(),
                match side {
                    P2pSide::BuyCrypto => "buy",
                    P2pSide::SellCrypto => "sell",
                },
                asset.to_ascii_lowercase()
            ),
            source_url_is_exact: false,
            ad_id: self.id,
            side,
            fiat,
            asset,
            price: self.price,
            available_asset: self.available_amount,
            min_fiat: self.quote_min_amount_per_order,
            max_fiat: self.quote_max_amount_per_order,
            payment_methods: self.payment_methods,
            pay_time_limit_minutes: self.payment_timeout_minutes,
            advertiser: Advertiser {
                id: self.public_user_id,
                nickname: self.nick_name,
                user_type: self.user_type,
                is_merchant,
                is_verified: is_merchant,
                completed_orders_30d: self.completed_order_quantity,
                completion_rate_30d: self.completed_rate.as_deref().and_then(parse_ratio),
                positive_rate: self
                    .pos_review_percentage
                    .as_deref()
                    .and_then(parse_percentage),
            },
            advertiser_profile_url,
        }
    }
}

fn parse_ratio(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| (0.0..=1.0).contains(value))
}

fn parse_percentage(value: &str) -> Option<f64> {
    value
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()
        .map(|value| if value > 1.0 { value / 100.0 } else { value })
        .filter(|value| (0.0..=1.0).contains(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_public_sell_ad_into_buy_crypto_offer() {
        let response: OkxResponse = serde_json::from_str(
            r#"{
              "code":0,
              "data":{"buy":[],"sell":[{
                "id":"okx-ad-1","publicUserId":"masked","merchantId":"merchant-1",
                "nickName":"CapitalX","baseCurrency":"USDT","quoteCurrency":"AMD",
                "price":"364.95","availableAmount":"2519.73",
                "quoteMinAmountPerOrder":"20000.00","quoteMaxAmountPerOrder":"919575.46",
                "paymentMethods":["IDBank","Ameriabank"],"paymentTimeoutMinutes":15,
                "userType":"all","completedOrderQuantity":1672,"completedRate":"0.9489",
                "posReviewPercentage":"99.10%","isInstitution":0
              }]},"msg":""
            }"#,
        )
        .unwrap();
        let offer = response
            .data
            .unwrap()
            .sell
            .remove(0)
            .into_offer(P2pSide::BuyCrypto);
        assert_eq!(offer.source, "okx");
        assert_eq!(offer.payment_methods, ["IDBank", "Ameriabank"]);
        assert_eq!(offer.advertiser.completion_rate_30d, Some(0.9489));
        assert_eq!(offer.advertiser.positive_rate, Some(0.991));
        assert!(!offer.source_url_is_exact);
        assert!(offer.advertiser.is_merchant);
        assert_eq!(
            offer.advertiser_profile_url.as_deref(),
            Some("https://www.okx.com/p2p/ads-merchant?publicUserId=masked")
        );
    }
}
