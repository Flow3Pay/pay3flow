use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::p2p::service::{Advertiser, P2pOffer, P2pSearchQuery, P2pSide, P2pSource};

const MAX_BINANCE_ROWS: usize = 20;

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
        let request = BinanceRequest {
            fiat: &query.fiat,
            page: 1,
            rows: query.fetch_limit().min(MAX_BINANCE_ROWS),
            trade_type: side,
            asset: &query.asset,
            countries: Vec::new(),
            pro_merchant_ads: false,
            shield_merchant_ads: false,
            publisher_type: None,
            pay_types: Vec::new(),
            additional_kyc_verify_filter: 0,
        };
        let response = self
            .client
            .post(&self.url)
            .header("origin", "https://www.binance.com")
            .header("referer", "https://www.binance.com/")
            .json(&request)
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
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.into_offer(query.side))
            .collect())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BinanceRequest<'a> {
    fiat: &'a str,
    page: u32,
    rows: usize,
    trade_type: &'a str,
    asset: &'a str,
    countries: Vec<String>,
    pro_merchant_ads: bool,
    shield_merchant_ads: bool,
    publisher_type: Option<String>,
    pay_types: Vec<String>,
    additional_kyc_verify_filter: u8,
}

#[derive(Debug, Deserialize)]
struct BinanceResponse {
    code: String,
    message: Option<String>,
    data: Option<Vec<BinanceItem>>,
}

#[derive(Debug, Deserialize)]
struct BinanceItem {
    adv: BinanceAd,
    advertiser: BinanceAdvertiser,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceAd {
    adv_no: String,
    price: String,
    fiat_unit: String,
    asset: String,
    tradable_quantity: String,
    min_single_trans_amount: String,
    max_single_trans_amount: String,
    pay_time_limit: Option<u32>,
    #[serde(default)]
    trade_methods: Vec<BinanceTradeMethod>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceTradeMethod {
    trade_method_name: Option<String>,
    identifier: Option<String>,
}

impl BinanceTradeMethod {
    fn name(self) -> Option<String> {
        self.trade_method_name
            .or(self.identifier)
            .filter(|name| !name.is_empty())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceAdvertiser {
    user_no: Option<String>,
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
        let advertiser_profile_url = self.advertiser.user_no.as_deref().map(|user_no| {
            format!("https://c2c.binance.com/en/advertiserDetail?advertiserNo={user_no}")
        });
        P2pOffer {
            source: "binance".into(),
            source_url: format!("https://c2c.binance.com/en/adv?code={}", self.adv.adv_no),
            ad_id: self.adv.adv_no,
            side,
            fiat: self.adv.fiat_unit,
            asset: self.adv.asset,
            price: self.adv.price,
            available_asset: self.adv.tradable_quantity,
            min_fiat: self.adv.min_single_trans_amount,
            max_fiat: self.adv.max_single_trans_amount,
            payment_methods: self
                .adv
                .trade_methods
                .into_iter()
                .filter_map(BinanceTradeMethod::name)
                .collect(),
            pay_time_limit_minutes: self.adv.pay_time_limit,
            advertiser: Advertiser {
                id: self.advertiser.user_no,
                nickname: self.advertiser.nick_name,
                user_type: self.advertiser.user_type,
                is_merchant,
                is_verified: is_merchant,
                completed_orders_30d: self.advertiser.month_order_count,
                completion_rate_30d: self.advertiser.month_finish_rate,
                positive_rate: self.advertiser.positive_rate,
            },
            advertiser_profile_url,
            source_url_is_exact: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_normalizes_agent_ad() {
        let response: BinanceResponse = serde_json::from_str(
            r#"{
              "code":"000000",
              "data":[{"adv":{
                "advNo":"ad-1","price":"361.75","fiatUnit":"AMD",
                "asset":"USDT","tradableQuantity":"423.41",
                "minSingleTransAmount":"19997.54","maxSingleTransAmount":"36175.00",
                "payTimeLimit":15,
                "tradeMethods":[{"tradeMethodName":"IDBank","identifier":"IDBank"}]},
                "advertiser":{"userNo":"user-1","nickName":"Trader","userType":"merchant",
                  "monthOrderCount":1204,"monthFinishRate":0.999,
                  "positiveRate":1.0,"merchantGroupMember":false}
              }]
            }"#,
        )
        .unwrap();
        let offer = response
            .data
            .unwrap()
            .remove(0)
            .into_offer(P2pSide::BuyCrypto);
        assert_eq!(offer.min_fiat, "19997.54");
        assert_eq!(offer.payment_methods, ["IDBank"]);
        assert!(offer.source_url_is_exact);
        assert!(offer.source_url.ends_with("code=ad-1"));
        assert!(offer.advertiser.is_merchant);
    }
}
