use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::p2p::service::{Advertiser, P2pOffer, P2pSearchQuery, P2pSide, P2pSource};

pub(crate) struct BybitP2pSource {
    client: reqwest::Client,
    url: String,
}

impl BybitP2pSource {
    pub(crate) fn new(client: reqwest::Client, url: String) -> Self {
        Self { client, url }
    }
}

#[async_trait]
impl P2pSource for BybitP2pSource {
    fn name(&self) -> &'static str {
        "bybit"
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        let side = match query.side {
            P2pSide::BuyCrypto => "1",
            P2pSide::SellCrypto => "0",
        };
        let body = BybitRequest {
            user_id: "",
            token_id: &query.asset,
            currency_id: &query.fiat,
            payment: Vec::new(),
            side,
            size: query.fetch_limit().to_string(),
            page: "1",
            amount: query
                .amount
                .map(|amount| amount.to_string())
                .unwrap_or_default(),
            auth_maker: false,
            can_trade: false,
        };
        let response = self
            .client
            .post(&self.url)
            .header("origin", "https://www.bybit.com")
            .header("referer", "https://www.bybit.com/")
            .json(&body)
            .send()
            .await
            .context("Bybit P2P request failed")?
            .error_for_status()
            .context("Bybit P2P returned an HTTP error")?
            .json::<BybitResponse>()
            .await
            .context("invalid Bybit P2P response")?;
        if response.ret_code != 0 {
            bail!(
                "Bybit P2P error {}: {}",
                response.ret_code,
                response.ret_msg
            );
        }
        Ok(response
            .result
            .map(|result| result.items)
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.into_offer(query.side))
            .collect())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BybitRequest<'a> {
    user_id: &'a str,
    token_id: &'a str,
    currency_id: &'a str,
    payment: Vec<String>,
    side: &'a str,
    size: String,
    page: &'a str,
    amount: String,
    auth_maker: bool,
    can_trade: bool,
}

#[derive(Debug, Deserialize)]
struct BybitResponse {
    ret_code: i64,
    ret_msg: String,
    result: Option<BybitResult>,
}

#[derive(Debug, Deserialize)]
struct BybitResult {
    #[serde(default)]
    items: Vec<BybitItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BybitItem {
    id: String,
    user_mask_id: Option<String>,
    nick_name: String,
    token_id: String,
    currency_id: String,
    price: String,
    quantity: String,
    min_amount: String,
    max_amount: String,
    #[serde(default)]
    payments: Vec<String>,
    recent_order_num: Option<u64>,
    recent_execute_rate: Option<f64>,
    auth_status: Option<i32>,
    user_type: Option<String>,
    payment_period: Option<u32>,
    #[serde(default)]
    auth_tag: Vec<String>,
}

impl BybitItem {
    fn into_offer(self, side: P2pSide) -> P2pOffer {
        let is_merchant = !self.auth_tag.is_empty()
            || self
                .user_type
                .as_deref()
                .is_some_and(|kind| !kind.eq_ignore_ascii_case("personal"));
        let fiat = self.currency_id;
        let asset = self.token_id;
        let advertiser_profile_url = self.user_mask_id.as_deref().map(|user_id| {
            format!("https://www.bybit.com/en/p2p/profile/{user_id}/{asset}/{fiat}/item")
        });
        P2pOffer {
            source: "bybit".into(),
            ad_id: self.id,
            side,
            price: self.price,
            available_asset: self.quantity,
            min_fiat: self.min_amount,
            max_fiat: self.max_amount,
            payment_methods: self.payments,
            pay_time_limit_minutes: self.payment_period,
            advertiser: Advertiser {
                id: self.user_mask_id,
                nickname: self.nick_name,
                user_type: self.user_type,
                is_merchant,
                is_verified: self.auth_status == Some(1),
                completed_orders_30d: self.recent_order_num,
                completion_rate_30d: self.recent_execute_rate.map(|rate| rate / 100.0),
                positive_rate: None,
            },
            advertiser_profile_url,
            source_url: format!("https://www.bybit.com/fiat/trade/otc/?token={asset}&fiat={fiat}"),
            source_url_is_exact: false,
            fiat,
            asset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_normalizes_public_ad() {
        let response: BybitResponse = serde_json::from_str(
            r#"{
              "ret_code":0,"ret_msg":"SUCCESS","result":{"items":[{
                "id":"ad-1","userMaskId":"masked-user","nickName":"Trader",
                "tokenId":"USDT","currencyId":"RUB","price":"96.76",
                "quantity":"100000","minAmount":"10000.00","maxAmount":"3500000.00",
                "payments":["40","14"],"recentOrderNum":294,"recentExecuteRate":99.5,
                "authStatus":1,"userType":"PERSONAL","paymentPeriod":15,"authTag":["VA"]
              }]}
            }"#,
        )
        .unwrap();
        let offer = response
            .result
            .unwrap()
            .items
            .remove(0)
            .into_offer(P2pSide::SellCrypto);
        assert_eq!(offer.price, "96.76");
        assert_eq!(offer.advertiser.completion_rate_30d, Some(0.995));
        assert!(!offer.source_url_is_exact);
        assert!(offer.advertiser.is_merchant);
        assert_eq!(
            offer.advertiser_profile_url.as_deref(),
            Some("https://www.bybit.com/en/p2p/profile/masked-user/USDT/RUB/item")
        );
    }
}
