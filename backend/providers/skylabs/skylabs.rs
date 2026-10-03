use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::p2p::{DeclarativeP2pSource, P2pOffer, P2pSearchQuery, P2pSide, P2pSource};
use crate::providers::ProviderAdapterRecord;

const API_BASE: &str = "https://api.skylabs.world/api";
const DEFAULT_FIAT_PROBE: f64 = 100_000.0;

pub(crate) struct SkyLabsSource {
    client: Client,
    inner: DeclarativeP2pSource,
}

impl SkyLabsSource {
    pub(crate) fn from_record(client: Client, record: &ProviderAdapterRecord) -> Option<Self> {
        if record.slug != "skylabs" {
            return None;
        }
        let inner = DeclarativeP2pSource::from_record(client.clone(), record)?;
        Some(Self { client, inner })
    }

    async fn api<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let response = self
            .client
            .get(format!("{API_BASE}/{path}"))
            .header("Origin", "https://skylabs.world")
            .header("Referer", "https://skylabs.world/")
            .send()
            .await
            .with_context(|| format!("fetch SkyLabs {path}"))?
            .error_for_status()
            .with_context(|| format!("fetch SkyLabs {path}"))?
            .json::<ApiResponse<T>>()
            .await
            .with_context(|| format!("decode SkyLabs {path}"))?;
        if !response.status {
            bail!(
                "SkyLabs {path} failed: {}",
                response.message.as_deref().unwrap_or("unknown API error")
            );
        }
        response
            .result
            .ok_or_else(|| anyhow!("SkyLabs {path} returned no result"))
    }

    async fn payment_methods(&self) -> Result<Vec<PaymentMethod>> {
        self.api("payment-methods").await
    }

    async fn withdrawal_fees(&self) -> Result<CommissionCatalog> {
        self.api("cryptomat/commissions").await
    }

    async fn conversion_rate(&self, query: &P2pSearchQuery) -> Result<f64> {
        self.api(&format!("rate/{}/{}/any", query.asset, query.fiat))
            .await
    }

    fn offers(
        &self,
        query: &P2pSearchQuery,
        raw_offer: P2pOffer,
        payment_methods: &[PaymentMethod],
        commissions: &CommissionCatalog,
        conversion_rate: f64,
    ) -> Result<Vec<P2pOffer>> {
        let raw_price = raw_offer
            .price
            .parse::<f64>()
            .context("SkyLabs returned a non-numeric price")?;
        let withdrawal_fees = commissions
            .currencies
            .iter()
            .find(|currency| currency.code.eq_ignore_ascii_case(&query.asset))
            .map(|currency| &currency.withdrawal_fees);
        let mut offers = Vec::new();

        for method in payment_methods {
            let factor = commission_factor(method, &query.asset, query.side)?;
            let method_price = if query.side == P2pSide::SellCrypto && method.requires_conversion {
                conversion_rate
            } else {
                raw_price
            };
            let method_name = payment_method_name(&method.key);

            if query.side == P2pSide::BuyCrypto
                && withdrawal_fees.is_some_and(|fees| !fees.is_empty())
            {
                let input_fiat = query.amount.unwrap_or(DEFAULT_FIAT_PROBE);
                for (fee_code, fee) in withdrawal_fees.expect("checked as non-empty") {
                    let Some(network) = withdrawal_network(fee_code) else {
                        continue;
                    };
                    let Ok(price) = effective_price(
                        method_price,
                        factor,
                        query.side,
                        Some(input_fiat),
                        fee.fee,
                    ) else {
                        continue;
                    };
                    offers.push(adjusted_offer(
                        &raw_offer,
                        &method.key,
                        &method_name,
                        Some(network),
                        price,
                    ));
                }
            } else {
                let price = effective_price(method_price, factor, query.side, None, 0.0)?;
                offers.push(adjusted_offer(
                    &raw_offer,
                    &method.key,
                    &method_name,
                    raw_offer.network.clone(),
                    price,
                ));
            }
        }
        Ok(offers)
    }
}

#[async_trait]
impl P2pSource for SkyLabsSource {
    fn name(&self) -> &str {
        "skylabs"
    }

    fn market(&self) -> crate::p2p::P2pOfferMarket {
        self.inner.market()
    }

    fn timeout(&self, default: Duration) -> Duration {
        self.inner.timeout(default)
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        if !self.inner.supports_query(query) {
            return Ok(Vec::new());
        }

        let raw_offers = match self.inner.search(query).await {
            Ok(offers) => offers,
            Err(error) if is_unsupported_quote_error(&error) => {
                tracing::debug!(
                    fiat = %query.fiat,
                    asset = %query.asset,
                    side = ?query.side,
                    "SkyLabs does not support this quote"
                );
                return Ok(Vec::new());
            }
            Err(error) => return Err(error),
        };
        let Some(raw_offer) = raw_offers.into_iter().next() else {
            return Ok(Vec::new());
        };

        let (payment_methods, commissions, conversion_rate) = tokio::try_join!(
            self.payment_methods(),
            self.withdrawal_fees(),
            self.conversion_rate(query),
        )?;
        self.offers(
            query,
            raw_offer,
            &payment_methods,
            &commissions,
            conversion_rate,
        )
    }
}

fn is_unsupported_quote_error(error: &anyhow::Error) -> bool {
    let message = format!("{error:#}");
    message.contains("response condition failed (false != true)")
        && message.contains("to type check failed")
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    status: bool,
    result: Option<T>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaymentMethod {
    key: String,
    #[serde(deserialize_with = "number_from_json")]
    cash_in_commission: f64,
    #[serde(deserialize_with = "number_from_json")]
    cash_out_commission: f64,
    #[serde(default)]
    requires_conversion: bool,
    #[serde(default)]
    convert_fees: HashMap<String, f64>,
    #[serde(default)]
    commission_discounts: CommissionDiscounts,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommissionDiscounts {
    #[serde(default)]
    cash_in_discount: HashMap<String, f64>,
    #[serde(default)]
    cash_out_discount: HashMap<String, f64>,
}

#[derive(Debug, Deserialize)]
struct CommissionCatalog {
    #[serde(default)]
    currencies: Vec<CurrencyFees>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurrencyFees {
    code: String,
    #[serde(default)]
    withdrawal_fees: HashMap<String, WithdrawalFee>,
}

#[derive(Debug, Deserialize)]
struct WithdrawalFee {
    fee: f64,
}

fn number_from_json<'de, D>(deserializer: D) -> std::result::Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        .ok_or_else(|| serde::de::Error::custom("expected a number or numeric string"))
}

fn commission_factor(method: &PaymentMethod, asset: &str, side: P2pSide) -> Result<f64> {
    let (base, discounts) = match side {
        P2pSide::BuyCrypto => (
            method.cash_in_commission,
            &method.commission_discounts.cash_in_discount,
        ),
        P2pSide::SellCrypto => (
            method.cash_out_commission,
            &method.commission_discounts.cash_out_discount,
        ),
    };
    let discount = discounts.get(asset).copied().unwrap_or(0.0);
    let discounted = base * (1.0 - discount / 100.0);
    let commission = if side == P2pSide::SellCrypto && method.requires_conversion {
        let conversion = method.convert_fees.get(asset).copied().unwrap_or(0.0);
        (1.0 - (1.0 - conversion / 100.0) * (1.0 - discounted / 100.0)) * 100.0
    } else {
        discounted
    };
    let factor = if method.key == "bank" {
        1.0 / (1.0 - commission / 100.0)
    } else {
        1.0 + commission / 100.0
    };
    if !factor.is_finite() || factor <= 0.0 {
        bail!("SkyLabs returned an invalid commission for {}", method.key);
    }
    Ok(factor)
}

fn effective_price(
    raw_price: f64,
    commission_factor: f64,
    side: P2pSide,
    input_fiat: Option<f64>,
    withdrawal_fee: f64,
) -> Result<f64> {
    if !raw_price.is_finite() || raw_price <= 0.0 || withdrawal_fee < 0.0 {
        bail!("SkyLabs returned an invalid rate or withdrawal fee");
    }
    let price = match side {
        P2pSide::BuyCrypto => raw_price * commission_factor,
        P2pSide::SellCrypto => raw_price / commission_factor,
    };
    let price = if withdrawal_fee > 0.0 {
        let input = input_fiat
            .filter(|amount| amount.is_finite() && *amount > 0.0)
            .context("a fiat amount is required for the SkyLabs withdrawal fee")?;
        let net_output = input / price - withdrawal_fee;
        if !net_output.is_finite() || net_output <= 0.0 {
            bail!("SkyLabs withdrawal fee consumes the quoted output");
        }
        input / net_output
    } else {
        price
    };
    if !price.is_finite() || price <= 0.0 {
        bail!("SkyLabs effective price is invalid");
    }
    Ok(price)
}

fn adjusted_offer(
    raw: &P2pOffer,
    method_key: &str,
    method_name: &str,
    network: Option<String>,
    price: f64,
) -> P2pOffer {
    let mut offer = raw.clone();
    offer.ad_id = format!(
        "{}-{method_key}-{}",
        raw.ad_id,
        network.as_deref().unwrap_or("default")
    );
    offer.network = network;
    offer.price = price.to_string();
    offer.payment_methods = vec![method_name.to_string()];
    offer
}

fn payment_method_name(key: &str) -> String {
    match key {
        "atm" => "SkyLabs ATM".into(),
        "easypay" => "EasyPay".into(),
        "telcell" => "Telcell".into(),
        "bank" => "Bank Transfer".into(),
        other => other.to_string(),
    }
}

fn withdrawal_network(code: &str) -> Option<String> {
    match code {
        "BTC" => Some("bitcoin"),
        "ETH" | "USDTE20" | "USDCE20" | "EURCE20" => Some("ethereum"),
        "TRX" | "USDTT20" | "USDCT20" => Some("tron"),
        "BNB" | "USDTB20" => Some("bnb-smart-chain"),
        "USDTTON" | "TON" => Some("ton"),
        "MATIC" => Some("polygon-pos"),
        "LTC" => Some("litecoin"),
        "SOL" | "EURCSOL" => Some("solana"),
        _ => None,
    }
    .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_adapter::ProviderAdapters;

    fn method(key: &str, cash_in: f64, cash_out: f64) -> PaymentMethod {
        PaymentMethod {
            key: key.into(),
            cash_in_commission: cash_in,
            cash_out_commission: cash_out,
            requires_conversion: false,
            convert_fees: HashMap::new(),
            commission_discounts: CommissionDiscounts::default(),
        }
    }

    fn source() -> SkyLabsSource {
        let document: toml::Value = toml::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/providers/skylabs/Providerfile"
        )))
        .unwrap();
        let config: ProviderAdapters = document.get("adapter").unwrap().clone().try_into().unwrap();
        let record = ProviderAdapterRecord {
            slug: "skylabs".into(),
            source_url: "https://skylabs.world/".into(),
            display_name: "SkyLabs".into(),
            exchange_methods: vec![crate::providers::ProviderExchangeMethod::Exchanger],
            config: Some(config),
            workflow: None,
        };
        SkyLabsSource::from_record(Client::new(), &record).unwrap()
    }

    #[test]
    fn effective_buy_price_includes_commission_and_fixed_withdrawal_fee() {
        let price =
            effective_price(365.5, 1.014, P2pSide::BuyCrypto, Some(100_000.0), 4.0).unwrap();

        assert!((price - 376.193_955_000_8).abs() < 0.000_000_1);
    }

    #[test]
    fn effective_sell_price_includes_cash_out_commission() {
        let price = effective_price(360.5, 1.027, P2pSide::SellCrypto, None, 0.0).unwrap();

        assert!((price - 351.022_395_326_2).abs() < 0.000_000_1);
    }

    #[test]
    fn commission_factors_match_skylabs_calculator_rules() {
        let atm = method("atm", 1.4, 2.7);
        let bank = method("bank", -1.0, 2.0);

        assert_eq!(
            commission_factor(&atm, "USDT", P2pSide::BuyCrypto).unwrap(),
            1.014
        );
        assert_eq!(
            commission_factor(&atm, "USDT", P2pSide::SellCrypto).unwrap(),
            1.027
        );
        assert!(
            (commission_factor(&bank, "USDT", P2pSide::BuyCrypto).unwrap()
                - 0.990_099_009_900_990_1)
                .abs()
                < f64::EPSILON
        );
        assert!(
            (commission_factor(&bank, "USDT", P2pSide::SellCrypto).unwrap()
                - 1.020_408_163_265_306_1)
                .abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn conversion_payment_method_combines_conversion_and_cash_out_commissions() {
        let mut easypay = method("easypay", 0.0, 2.0);
        easypay.requires_conversion = true;
        easypay.convert_fees.insert("USDT".into(), 1.0);

        assert!(
            (commission_factor(&easypay, "USDT", P2pSide::SellCrypto).unwrap() - 1.0298).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn maps_every_published_skylabs_withdrawal_rail() {
        let cases = [
            ("USDTE20", "ethereum"),
            ("USDTT20", "tron"),
            ("USDTB20", "bnb-smart-chain"),
            ("USDTTON", "ton"),
            ("SOL", "solana"),
        ];
        for (code, expected) in cases {
            assert_eq!(
                withdrawal_network(code).as_deref(),
                Some(expected),
                "{code}"
            );
        }
    }

    #[tokio::test]
    #[ignore = "calls the live SkyLabs rate and commission APIs"]
    async fn live_quote_is_split_by_payment_method_and_withdrawal_network() {
        let offers = source()
            .search(&P2pSearchQuery {
                fiat: "AMD".into(),
                asset: "USDT".into(),
                side: P2pSide::BuyCrypto,
                amount: Some(100_000.0),
                payment_method: None,
                merchant_only: None,
                min_orders: None,
                min_completion_rate: None,
                limit: Some(20),
                sources: Some("skylabs".into()),
            })
            .await
            .unwrap();

        assert!(offers.len() >= 16);
        assert!(offers
            .iter()
            .any(|offer| offer.network.as_deref() == Some("tron")));
        assert!(offers
            .iter()
            .any(|offer| offer.payment_methods == ["SkyLabs ATM"]));
        assert!(offers
            .iter()
            .all(|offer| offer.price.parse::<f64>().unwrap().is_finite()));
    }
}
