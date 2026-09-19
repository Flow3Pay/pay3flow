use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::Deserialize;

use crate::p2p::service::{Advertiser, P2pOffer, P2pSearchQuery, P2pSide, P2pSource};

pub(crate) struct BinanceP2pSource {
    client: reqwest::Client,
    url: String,
}

impl BinanceP2pSource {
    pub(crate) fn new(client: reqwest::Client, url: String) -> Self {
        Self { client, url }
    }
}

#[async_trait]
impl P2pSource for BinanceP2pSource {
    fn name(&self) -> &'static str {
        "binance"
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        let side = match query.side {
            P2pSide::BuyCrypto => "BUY",
            P2pSide::SellCrypto => "SELL",
        };
        let response = self
            .client
            .get(&self.url)
            .query(&[
                ("fiat", query.fiat.as_str()),
                ("asset", query.asset.as_str()),
                ("tradeType", side),
                ("limit", &query.fetch_limit().to_string()),
                ("order", "PRICE"),
            ])
            .send()
            .await
            .context("Binance P2P request failed")?
            .error_for_status()
            .context("Binance P2P returned an HTTP error")?
            .json::<BinanceResponse>()
            .await
            .context("invalid Binance P2P response")?;
        if response.code != "000000" {
            bail!(
                "Binance P2P error {}: {}",
                response.code,
                response.message.unwrap_or_else(|| "unknown error".into())
            );
        }
        Ok(response
            .data
            .map(|data| data.items)
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.into_offer(query.side))
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct BinanceResponse {
    code: String,
    message: Option<String>,
    data: Option<BinanceData>,
}

#[derive(Debug, Deserialize)]
struct BinanceData {
    #[serde(default)]
    items: Vec<BinanceItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceItem {
    ad_no: String,
    price: f64,
    fiat: String,
    fiat_scale: u32,
    asset: String,
    asset_scale: u32,
    price_scale: u32,
    min_trans_amount: f64,
    max_trans_amount: f64,
    tradable_amount: f64,
    pay_time_limit: Option<u32>,
    #[serde(default)]
    trade_methods: Vec<String>,
    advertiser: BinanceAdvertiser,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceAdvertiser {
    nick_name: String,
    user_type: Option<String>,
    month_order_count: Option<u64>,
    month_finish_rate: Option<f64>,
    positive_rate: Option<f64>,
    #[serde(default)]
    merchant_group_member: bool,
}

impl BinanceItem {
    fn into_offer(self, side: P2pSide) -> P2pOffer {
        let is_merchant = self.advertiser.merchant_group_member
            || self
                .advertiser
                .user_type
                .as_deref()
                .is_some_and(|kind| kind.eq_ignore_ascii_case("merchant"));
        P2pOffer {
            source: "binance".into(),
            source_url: format!("https://c2c.binance.com/en/adv?code={}", self.ad_no),
            ad_id: self.ad_no,
            side,
            fiat: self.fiat,
            asset: self.asset,
            price: fixed(self.price, self.price_scale),
            available_asset: fixed(self.tradable_amount, self.asset_scale),
            // Binance's agent endpoint expresses these two limits in asset units.
            min_fiat: fixed(self.min_trans_amount * self.price, self.fiat_scale),
            max_fiat: fixed(self.max_trans_amount * self.price, self.fiat_scale),
            payment_methods: self.trade_methods,
            pay_time_limit_minutes: self.pay_time_limit,
            advertiser: Advertiser {
                id: None,
                nickname: self.advertiser.nick_name,
                user_type: self.advertiser.user_type,
                is_merchant,
                is_verified: is_merchant,
                completed_orders_30d: self.advertiser.month_order_count,
                completion_rate_30d: self.advertiser.month_finish_rate,
                positive_rate: self.advertiser.positive_rate,
            },
        }
    }
}

fn fixed(value: f64, scale: u32) -> String {
    format!("{value:.precision$}", precision = scale.min(8) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_normalizes_agent_ad() {
        let response: BinanceResponse = serde_json::from_str(
            r#"{
              "code":"000000",
              "data":{"items":[{
                "adNo":"ad-1","price":361.75,"fiat":"AMD","fiatScale":2,
                "asset":"USDT","assetScale":2,"priceScale":2,
                "minTransAmount":55.28,"maxTransAmount":100.0,
                "tradableAmount":423.41,"payTimeLimit":15,
                "tradeMethods":["IDBank"],
                "advertiser":{"nickName":"Trader","userType":"merchant",
                  "monthOrderCount":1204,"monthFinishRate":0.999,
                  "positiveRate":1.0,"merchantGroupMember":false}
              }]}
            }"#,
        )
        .unwrap();
        let offer = response
            .data
            .unwrap()
            .items
            .remove(0)
            .into_offer(P2pSide::BuyCrypto);
        assert_eq!(offer.min_fiat, "19997.54");
        assert_eq!(offer.payment_methods, ["IDBank"]);
        assert!(offer.advertiser.is_merchant);
    }
}
