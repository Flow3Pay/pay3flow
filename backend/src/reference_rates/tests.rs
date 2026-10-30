use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn snapshot(rates: HashMap<String, Observation>) -> Snapshot {
    Snapshot {
        rates,
        fetched_at: "2026-10-30T00:00:00Z".into(),
        fetched: Instant::now(),
    }
}

#[test]
fn xml_nominals_encoding_inversion_and_direct_rub_amd() {
    let xml = r#"<?xml version="1.0" encoding="windows-1251"?><ValCurs Date="30.10.2026"><Valute><Name>Доллар США</Name><CharCode>USD</CharCode><Nominal>1</Nominal><Value>80,0</Value></Valute><Valute><CharCode>AMD</CharCode><Nominal>100</Nominal><Value>20,0</Value></Valute><Valute><CharCode>BAD</CharCode><Nominal>0</Nominal><Value>1</Value></Valute></ValCurs>"#;
    let (bytes, _, errors) = encoding_rs::WINDOWS_1251.encode(xml);
    assert!(!errors);
    let text = encoding_rs::WINDOWS_1251.decode(&bytes).0;
    let snapshot = snapshot(parse_xml(Source::Cbr, &text).unwrap());
    assert!(!snapshot.rates.contains_key("BAD"));
    assert_eq!(
        direct_rate(Source::Cbr, &snapshot, "USD", "RUB", false)
            .unwrap()
            .rate,
        80.0
    );
    assert_eq!(
        direct_rate(Source::Cbr, &snapshot, "RUB", "USD", false)
            .unwrap()
            .rate,
        0.0125
    );
    assert_eq!(
        direct_rate(Source::Cbr, &snapshot, "RUB", "AMD", false)
            .unwrap()
            .rate,
        5.0
    );
    assert_eq!(
        direct_rate(Source::Cbr, &snapshot, "AMD", "RUB", false)
            .unwrap()
            .rate,
        0.2
    );
    assert_eq!(snapshot.rates["AMD"].published_at, "2026-10-30");
}

#[test]
fn national_bank_parsers_normalize_scale_and_reject_invalid_values() {
    let nbrb = parse_json(Source::Nbrb, r#"[{"Cur_Abbreviation":"USD","Cur_OfficialRate":3.2,"Cur_Scale":1,"Date":"2026-10-30T00:00:00"},{"Cur_Abbreviation":"RUB","Cur_OfficialRate":4,"Cur_Scale":100,"Date":"2026-10-30T00:00:00"},{"Cur_Abbreviation":"BAD","Cur_OfficialRate":1,"Cur_Scale":0,"Date":"2026-10-30T00:00:00"}]"#).unwrap();
    assert_eq!(nbrb["RUB"].value, 0.04);
    assert!(!nbrb.contains_key("BAD"));
    let nbu = parse_json(
        Source::Nbu,
        r#"[{"cc":"USD","rate":40.0,"exchangedate":"30.10.2026"}]"#,
    )
    .unwrap();
    assert_eq!(nbu["USD"].value, 40.0);
    let nbk = parse_xml(Source::Nbk, r#"<rss><channel><item><title>USD</title><pubDate>30.10.2026</pubDate><description>500.0</description><quant>1</quant></item><item><title>JPY</title><pubDate>30.10.2026</pubDate><description>300.0</description><quant>100</quant></item></channel></rss>"#).unwrap();
    assert_eq!(nbk["JPY"].value, 3.0);
    assert!(parse_xml(Source::Cbr, "<broken").is_err());
}

fn rateam_page() -> String {
    // Representative Flight records: record references, duplicates, summaries and both markets.
    let now = Utc::now().timestamp_millis();
    let stream = format!(
        r##"1:{{"CASH":{{"buy":"350","sell":"370"}},"CLEARING":{{"buy":"370","sell":"390"}}}}
2:{{"USD":"$1","RUR":{{"CASH":{{"buy":"4","sell":"6"}},"CLEARING":{{"buy":"3","sell":"5"}}}}}}
3:{{"USD":{{"CASH":{{"buy":"370","sell":"390"}},"CLEARING":{{"buy":"390","sell":"410"}}}}}}
4:["$","$L5",null,{{"aggregateRates":[{{"organizationType":"BANK","code":"one","lastUpdated":{now},"rates":"$2"}},{{"organizationType":"BANK","code":"one","lastUpdated":{now},"rates":"$2"}},{{"organizationType":"BANK","code":"two","lastUpdated":{now},"rates":"$3"}},{{"organizationType":"BANK","code":"bad","lastUpdated":{now},"rates":{{"USD":{{"CASH":{{"buy":"900","sell":"1"}}}}}}}},{{"organizationType":"BANK","code":"old","lastUpdated":1,"rates":"$2"}},{{"organizationType":"SUMMARY","code":"average","lastUpdated":{now},"rates":"$2"}}]}}]
"##
    );
    // Split a JSON record across script chunks, as Next can do.
    let middle = stream.len() / 2;
    format!(
        "<script>self.__next_f.push({})</script><script>self.__next_f.push({})</script>",
        serde_json::json!([1, &stream[..middle]]),
        serde_json::json!([1, &stream[middle..]])
    )
}

#[test]
fn rateam_resolves_records_deduplicates_banks_and_separates_markets() {
    let cash = parse_rateam(&rateam_page(), Market::Cash).unwrap();
    let noncash = parse_rateam(&rateam_page(), Market::Noncash).unwrap();
    assert_eq!(cash["USD"].value, 370.0);
    assert_eq!(cash["USD"].count, 2);
    assert_eq!(cash["RUB"].value, 5.0);
    assert_eq!(cash["RUB"].count, 1);
    assert_eq!(noncash["USD"].value, 390.0);
    assert_eq!(noncash["RUB"].value, 4.0);
    assert!(parse_rateam("<html>No quotes</html>", Market::Cash).is_err());
}

#[tokio::test]
async fn cross_rates_dual_sources_identity_and_unsupported_currency() {
    let service = ReferenceRateService::default();
    for (source, values) in [
        (Source::Cbr, vec![("USD", 80.0), ("AMD", 0.2)]),
        (Source::Nbrb, vec![("USD", 3.2)]),
        (Source::Nbu, vec![("USD", 40.0)]),
        (Source::Nbk, vec![("USD", 500.0)]),
    ] {
        service.caches[&(source, Market::Noncash)]
            .lock()
            .await
            .snapshot = Some(snapshot(
            values
                .into_iter()
                .map(|(currency, value)| {
                    (
                        currency.into(),
                        Observation {
                            value,
                            count: 1,
                            published_at: "2026-10-30".into(),
                        },
                    )
                })
                .collect(),
        ));
    }
    for market in [Market::Cash, Market::Noncash] {
        service.caches[&(Source::RateAm, market)]
            .lock()
            .await
            .snapshot = Some(snapshot(parse_rateam(&rateam_page(), market).unwrap()));
    }
    let dual = service.list("RUB", "AMD", Market::Cash).await.unwrap();
    assert_eq!(dual.rates.len(), 2);
    assert_eq!(dual.rates[0].rate, 5.0);
    assert_eq!(dual.rates[1].rate, 5.0);
    let cross = service.list("BYN", "KZT", Market::Cash).await.unwrap();
    assert_eq!(cross.rates[0].rate, 156.25);
    assert_eq!(cross.rates[0].method, "cross_via_usd");
    assert_eq!(cross.rates[0].sources.len(), 2);
    assert_eq!(
        service
            .list("AMD", "BYN", Market::Cash)
            .await
            .unwrap()
            .rates[0]
            .sample_count,
        2
    );
    for currency in FIAT {
        for target in FIAT {
            assert!(
                !service
                    .list(currency, target, Market::Noncash)
                    .await
                    .unwrap()
                    .rates
                    .is_empty(),
                "{currency}/{target}"
            );
        }
    }
    assert_eq!(
        service
            .list("usd", "USD", Market::Cash)
            .await
            .unwrap()
            .rates[0]
            .rate,
        1.0
    );
    assert!(service.list("BTC", "USD", Market::Cash).await.is_err());
}

async fn upstream(
    status: axum::http::StatusCode,
    body: &'static str,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let requests = Arc::new(AtomicUsize::new(0));
    let counter = requests.clone();
    let app = axum::Router::new().route(
        "/",
        axum::routing::get(move || {
            counter.fetch_add(1, Ordering::SeqCst);
            async move {
                tokio::time::sleep(Duration::from_millis(20)).await;
                (status, body)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (endpoint, requests, task)
}

#[tokio::test]
async fn concurrent_requests_share_refresh_and_populate_cache() {
    let (endpoint, requests, task) = upstream(axum::http::StatusCode::OK, r#"<ValCurs Date="30.10.2026"><Valute><CharCode>USD</CharCode><Value>80</Value><Nominal>1</Nominal></Valute></ValCurs>"#).await;
    let mut service = ReferenceRateService::default();
    Arc::make_mut(&mut service.endpoints).insert(Source::Cbr, endpoint);
    let calls = (0..8).map(|_| service.list("USD", "RUB", Market::Cash));
    for result in futures::future::join_all(calls).await {
        assert_eq!(result.unwrap().rates[0].rate, 80.0);
    }
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    task.abort();
}

#[tokio::test]
async fn failed_refresh_preserves_partial_results_with_backoff_and_stale_expiry() {
    let (endpoint, requests, task) =
        upstream(axum::http::StatusCode::SERVICE_UNAVAILABLE, "down").await;
    let mut service = ReferenceRateService::default();
    Arc::make_mut(&mut service.endpoints).insert(Source::Cbr, endpoint.clone());
    Arc::make_mut(&mut service.endpoints).insert(Source::RateAm, endpoint);
    let mut old = snapshot(parse_xml(Source::Cbr, r#"<ValCurs Date="30.10.2026"><Valute><CharCode>USD</CharCode><Value>80</Value><Nominal>1</Nominal></Valute><Valute><CharCode>AMD</CharCode><Value>20</Value><Nominal>100</Nominal></Valute></ValCurs>"#).unwrap());
    old.fetched = Instant::now() - Duration::from_secs(3601);
    service.caches[&(Source::Cbr, Market::Noncash)]
        .lock()
        .await
        .snapshot = Some(old);
    let partial = service.list("RUB", "AMD", Market::Cash).await.unwrap();
    assert_eq!(partial.rates.len(), 1);
    assert!(partial.rates[0].stale);
    assert_eq!(partial.unavailable_sources[0].name, "Rate.am");
    service.list("RUB", "AMD", Market::Cash).await.unwrap();
    assert_eq!(requests.load(Ordering::SeqCst), 2); // one per feed; repeated request backs off
    service.caches[&(Source::Cbr, Market::Noncash)]
        .lock()
        .await
        .snapshot
        .as_mut()
        .unwrap()
        .fetched = Instant::now() - Duration::from_secs(72 * 3600 + 1);
    let expired = service.list("RUB", "AMD", Market::Cash).await.unwrap();
    assert!(expired.rates.is_empty());
    assert_eq!(expired.unavailable_sources.len(), 2);
    task.abort();
}

#[tokio::test]
#[ignore = "read-only smoke check against live external feeds"]
async fn live_reference_feeds() {
    let service = ReferenceRateService::default();
    for market in [Market::Cash, Market::Noncash] {
        for currency in ["RUB", "AMD", "BYN", "UAH", "KZT"] {
            let result = service.list("USD", currency, market).await.unwrap();
            println!("USD/{currency} {market:?}: {result:?}");
            assert_eq!(result.rates.len(), 1, "live {currency} {market:?}");
        }
        let dual = service.list("RUB", "AMD", market).await.unwrap();
        assert_eq!(dual.rates.len(), 2);
    }
}

#[test]
fn query_requires_a_pair_and_valid_market() {
    let query = |uri: &str| Query::<ReferenceRatesQuery>::try_from_uri(&uri.parse().unwrap());
    assert_eq!(
        query("/api/reference-rates?from=USD&to=AMD")
            .unwrap()
            .market,
        Market::Noncash
    );
    assert_eq!(
        query("/api/reference-rates?from=USD&to=AMD&market=cash")
            .unwrap()
            .market,
        Market::Cash
    );
    assert!(query("/api/reference-rates?from=USD").is_err());
    assert!(query("/api/reference-rates?from=USD&to=AMD&market=card").is_err());
}
