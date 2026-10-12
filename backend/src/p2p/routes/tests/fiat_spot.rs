use super::*;
use crate::p2p::spot::CryptoMarketSource;

struct UsdtEntry {
    available: &'static str,
}

#[async_trait]
impl P2pSource for UsdtEntry {
    fn name(&self) -> &str {
        "bybit"
    }

    async fn search(&self, query: &crate::p2p::P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        if query.asset == "SUI" {
            // Direct-asset discovery finishes after the Spot route and must not erase it.
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        if query.asset != "USDT" || query.fiat != "UAH" || query.side != P2pSide::BuyCrypto {
            return Ok(Vec::new());
        }
        let mut entry = offer("bybit", query.side, "40", "100", "100000");
        entry.fiat = "UAH".into();
        entry.available_asset = self.available.into();
        entry.payment_methods = vec!["Monobank".into()];
        Ok(vec![entry])
    }
}

struct SuiMarket;

#[async_trait]
impl PublicRouteProvider for SuiMarket {
    fn name(&self) -> &str {
        "catalog-only"
    }

    async fn supported_assets(&self) -> Vec<Asset> {
        vec![Asset::new("SUI", Some("sui")).unwrap()]
    }

    async fn quote(&self, _: Asset, _: Asset, _: Amount) -> Result<PublicRouteQuote> {
        anyhow::bail!("catalog-only test provider has no swap quotes")
    }
}

#[async_trait]
impl CryptoMarketSource for SuiMarket {
    fn name(&self) -> &str {
        "bybit"
    }

    async fn tickers(&self) -> Result<Vec<CryptoTicker>> {
        Ok(vec![CryptoTicker {
            symbol: "SUIUSDT".into(),
            bid: 1.9,
            ask: 2.0,
        }])
    }
}

fn request() -> P2pRouteSearchQuery {
    let mut request = crypto_cycle_request(10000.0);
    request.source_fiat = "UAH".into();
    request.target_fiat = "SUI".into();
    request.source_network = None;
    request.target_network = Some("sui".into());
    request.intermediary_assets = Some("USDT".into());
    request.source_payment_method = Some("Monobank".into());
    request.allow_cross_venue = Some(false);
    request.exchange_mode = ExchangeMode::All;
    request
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uah_to_sui_combines_same_venue_p2p_and_spot_and_survives_late_direct_results() {
    let service = P2pSearchService::with_sources(
        vec![Arc::new(UsdtEntry { available: "1000" })],
        Duration::from_secs(1),
    )
    .with_market_sources(vec![Arc::new(SuiMarket)])
    .with_route_providers(vec![Arc::new(SuiMarket)]);
    let result = service.search_routes(request()).await.unwrap();
    assert_eq!(result.routes_found, 1);
    let route = &result.routes[0];
    assert_eq!(route.source_fiat, "UAH");
    assert_eq!(route.source_amount.parse::<f64>().unwrap(), 10000.0);
    assert_eq!(route.target_fiat, "SUI");
    assert_eq!(route.target_amount.parse::<f64>().unwrap(), 124.875);
    assert_eq!(route.acquired_asset_amount.parse::<f64>().unwrap(), 250.0);
    assert_eq!(route.asset, "USDT");
    assert_eq!(route.entry_offer.as_ref().unwrap().source, "bybit");
    assert_eq!(route.market_path.as_ref().unwrap().source_pair, "SUIUSDT");
    assert_eq!(route.route_path, ["UAH", "USDT", "SUI@sui"]);
    assert_eq!(route.route_kind, "fiat_to_crypto");
    assert!(route.same_venue);
    assert!(!route.requires_asset_transfer);
    assert!(!route.transfer_fee_included);
    assert!(route.payment_methods_verified);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fiat_spot_routes_honor_modes_sources_intermediaries_bank_and_available_balance() {
    for case in [
        "p2p_only",
        "exchangers_only",
        "excluded_venue",
        "excluded_asset",
        "bank",
        "balance",
    ] {
        let service = P2pSearchService::with_sources(
            vec![Arc::new(UsdtEntry {
                available: if case == "balance" { "1" } else { "1000" },
            })],
            Duration::from_secs(1),
        )
        .with_market_sources(vec![Arc::new(SuiMarket)])
        .with_route_providers(vec![Arc::new(SuiMarket)]);
        let mut request = request();
        match case {
            "p2p_only" => request.exchange_mode = ExchangeMode::P2p,
            "exchangers_only" => request.exchange_mode = ExchangeMode::Exchanger,
            "excluded_venue" => request.sources = Some("binance".into()),
            "excluded_asset" => request.intermediary_assets = Some("BTC".into()),
            "bank" => request.source_payment_method = Some("PrivatBank".into()),
            _ => {}
        }
        let result = service.search_routes(request).await.unwrap();
        if case == "bank" {
            // Keep the existing payment-method fallback, but never claim it was verified.
            assert_eq!(result.routes_found, 1);
            assert!(!result.routes[0].payment_methods_verified);
            assert!(result.routes[0]
                .warnings
                .iter()
                .any(|warning| warning.contains("payment method was not confirmed")));
        } else {
            assert_eq!(result.routes_found, 0, "case: {case}");
        }
    }
}
