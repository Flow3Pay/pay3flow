use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::p2p::service::{Advertiser, P2pOffer, P2pSearchQuery, P2pSide, P2pSource};

pub(crate) struct BitgetP2pSource {
    client: reqwest::Client,
    url: String,
}

impl BitgetP2pSource {
    pub(crate) fn new(client: reqwest::Client, url: String) -> Self {
        Self { client, url }
    }
}

#[async_trait]
impl P2pSource for BitgetP2pSource {
    fn name(&self) -> &'static str {
        "bitget"
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        let body = BitgetRequest {
            side: match query.side {
                P2pSide::BuyCrypto => 1,
                P2pSide::SellCrypto => 2,
            },
            page_no: 1,
            page_size: query.fetch_limit(),
            coin_code: &query.asset,
            fiat_code: &query.fiat,
        };
        let response = self
            .client
            .post(&self.url)
            .header("origin", "https://www.bitget.com")
            .header("referer", "https://www.bitget.com/p2p-trade")
            .json(&body)
            .send()
            .await
            .context("Bitget P2P request failed")?
            .error_for_status()
            .context("Bitget P2P returned an HTTP error")?
            .json::<BitgetResponse>()
            .await
            .context("invalid Bitget P2P response")?;
        if response.code != "00000" {
            bail!(
                "Bitget P2P error {}: {}",
                response.code,
                response.msg.unwrap_or_else(|| "unknown error".into())
            );
        }
        Ok(response
            .data
            .map(|data| data.data_list)
            .unwrap_or_default()
            .into_iter()
            .map(|ad| ad.into_offer(query.side))
            .collect())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BitgetRequest<'a> {
    side: u8,
    page_no: usize,
    page_size: usize,
    coin_code: &'a str,
    fiat_code: &'a str,
}

#[derive(Debug, Deserialize)]
struct BitgetResponse {
    code: String,
    data: Option<BitgetData>,
    msg: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BitgetData {
    #[serde(default)]
    data_list: Vec<BitgetAd>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BitgetAd {
    ad_no: String,
    coin_code: String,
    fiat_code: String,
    price: String,
    edit_amount: String,
    min_amount: String,
    max_amount: String,
    #[serde(default)]
    paymethod_info: Vec<BitgetPaymentMethod>,
    pay_duration: Option<u32>,
    encrypt_user_id: Option<String>,
    nick_name: String,
    certified_merchant: Option<u8>,
    thirty_tunover_num: Option<String>,
    thirty_completion_rate: Option<String>,
    good_evaluation_rate: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BitgetPaymentMethod {
    paymethod_name: String,
}

impl BitgetAd {
    fn into_offer(self, side: P2pSide) -> P2pOffer {
        let is_merchant = self.certified_merchant.is_some_and(|value| value > 0);
        let fiat = self.fiat_code;
        let asset = self.coin_code;
        let advertiser_profile_url = self
            .encrypt_user_id
            .as_deref()
            .map(|user_id| format!("https://www.bitget.com/p2p-trade/user/{user_id}"));
        P2pOffer {
            source: "bitget".into(),
            source_url: format!(
                "https://www.bitget.com/p2p-trade/{}?fiatName={}",
                match side {
                    P2pSide::BuyCrypto => "buy",
                    P2pSide::SellCrypto => "sell",
                },
                fiat
            ),
            source_url_is_exact: false,
            ad_id: self.ad_no,
            side,
            fiat,
            asset,
            price: self.price,
            available_asset: self.edit_amount,
            min_fiat: self.min_amount,
            max_fiat: self.max_amount,
            payment_methods: self
                .paymethod_info
                .into_iter()
                .map(|method| method.paymethod_name)
                .collect(),
            pay_time_limit_minutes: self.pay_duration,
            advertiser: Advertiser {
                id: self.encrypt_user_id,
                nickname: self.nick_name,
                user_type: self.certified_merchant.map(|value| value.to_string()),
                is_merchant,
                is_verified: is_merchant,
                completed_orders_30d: self
                    .thirty_tunover_num
                    .as_deref()
                    .and_then(|value| value.parse().ok()),
                completion_rate_30d: self.thirty_completion_rate.as_deref().and_then(parse_ratio),
                positive_rate: self
                    .good_evaluation_rate
                    .as_deref()
                    .and_then(parse_percentage),
            },
            advertiser_profile_url,
        }
    }
}

fn parse_ratio(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().and_then(|value| {
        let value = if value > 1.0 { value / 100.0 } else { value };
        (0.0..=1.0).contains(&value).then_some(value)
    })
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
    fn parses_public_ad_and_payment_methods() {
        let response: BitgetResponse = serde_json::from_str(
            r#"{
              "code":"00000","msg":"success","data":{"dataList":[{
                "adNo":"bitget-ad-1","coinCode":"USDT","fiatCode":"AMD",
                "price":"373.74","editAmount":"375.95","minAmount":"10000",
                "maxAmount":"140000","payDuration":20,"encryptUserId":"masked",
                "nickName":"Exchange_ARM","certifiedMerchant":1,
                "thirtyTunoverNum":"69","thirtyCompletionRate":"0.97",
                "goodEvaluationRate":"100","paymethodInfo":[
                  {"paymethodName":"Ameriabank"},{"paymethodName":"IDBank"}
                ]
              }]}
            }"#,
        )
        .unwrap();
        let offer = response
            .data
            .unwrap()
            .data_list
            .remove(0)
            .into_offer(P2pSide::BuyCrypto);
        assert_eq!(offer.source, "bitget");
        assert_eq!(offer.available_asset, "375.95");
        assert_eq!(offer.payment_methods, ["Ameriabank", "IDBank"]);
        assert_eq!(offer.advertiser.completed_orders_30d, Some(69));
        assert_eq!(offer.advertiser.positive_rate, Some(1.0));
        assert!(!offer.source_url_is_exact);
        assert_eq!(
            offer.advertiser_profile_url.as_deref(),
            Some("https://www.bitget.com/p2p-trade/user/masked")
        );
    }
}
