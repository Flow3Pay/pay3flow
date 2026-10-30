//! Informational FX benchmarks. Never used to price or execute a payment.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use axum::extract::{Query, State};
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::core::{error::AppError, state::AppState};

const FIAT: &[&str] = &["AMD", "RUB", "USD", "BYN", "UAH", "KZT"];
const RETRY: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Market {
    Cash,
    #[default]
    Noncash,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Source {
    Cbr,
    RateAm,
    Nbrb,
    Nbu,
    Nbk,
}

impl Source {
    fn name(self) -> &'static str {
        match self {
            Self::Cbr => "CBR",
            Self::RateAm => "Rate.am",
            Self::Nbrb => "NBRB",
            Self::Nbu => "NBU",
            Self::Nbk => "NBK",
        }
    }
    fn url(self) -> &'static str {
        match self {
            Self::Cbr => "https://www.cbr.ru/scripts/XML_daily.asp",
            Self::RateAm => "https://www.rate.am/en/armenian-dram-exchange-rates/banks",
            Self::Nbrb => "https://api.nbrb.by/exrates/rates?periodicity=0",
            Self::Nbu => "https://bank.gov.ua/NBUStatService/v1/statdirectory/exchange?json",
            Self::Nbk => "https://nationalbank.kz/rss/rates_all.xml",
        }
    }
    fn ttl(self) -> Duration {
        Duration::from_secs(if self == Self::RateAm { 300 } else { 3600 })
    }
    fn max_age(self) -> Duration {
        Duration::from_secs(if self == Self::RateAm {
            3600
        } else {
            72 * 3600
        })
    }
    fn base(self) -> &'static str {
        match self {
            Self::Cbr => "RUB",
            Self::RateAm => "AMD",
            Self::Nbrb => "BYN",
            Self::Nbu => "UAH",
            Self::Nbk => "KZT",
        }
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct Attribution {
    pub name: String,
    pub url: String,
    pub published_at: String,
    pub fetched_at: String,
}

#[derive(Clone, Serialize, Debug)]
pub struct ReferenceRate {
    pub from: String,
    pub to: String,
    pub rate: f64,
    pub sources: Vec<Attribution>,
    pub method: &'static str,
    pub sample_count: usize,
    pub stale: bool,
}

#[derive(Serialize, Debug)]
pub struct UnavailableSource {
    pub name: String,
    pub url: String,
}

#[derive(Serialize, Debug)]
pub struct ReferenceRatesResponse {
    pub from: String,
    pub to: String,
    pub market: Market,
    pub rates: Vec<ReferenceRate>,
    pub unavailable_sources: Vec<UnavailableSource>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceRatesQuery {
    from: String,
    to: String,
    #[serde(default)]
    market: Market,
}

#[derive(Clone)]
struct Observation {
    value: f64, // local currency units per one foreign unit
    count: usize,
    published_at: String,
}

#[derive(Clone)]
struct Snapshot {
    rates: HashMap<String, Observation>,
    fetched_at: String,
    fetched: Instant,
}

#[derive(Default)]
struct Cache {
    snapshot: Option<Snapshot>,
    failed_at: Option<Instant>,
}

type SourceCache = Arc<Mutex<Cache>>;

#[derive(Clone)]
pub struct ReferenceRateService {
    client: reqwest::Client,
    caches: Arc<HashMap<(Source, Market), SourceCache>>,
    endpoints: Arc<HashMap<Source, String>>,
}

impl Default for ReferenceRateService {
    fn default() -> Self {
        let sources = [
            Source::Cbr,
            Source::RateAm,
            Source::Nbrb,
            Source::Nbu,
            Source::Nbk,
        ];
        let mut caches = HashMap::new();
        for source in sources {
            for market in [Market::Cash, Market::Noncash] {
                caches.insert((source, market), Arc::new(Mutex::new(Cache::default())));
            }
        }
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .user_agent("Mozilla/5.0 (compatible; Pay3flow reference rates)")
                .build()
                .expect("reference rate HTTP client"),
            caches: Arc::new(caches),
            endpoints: Arc::new(sources.into_iter().map(|s| (s, s.url().into())).collect()),
        }
    }
}

impl ReferenceRateService {
    // The per-source lock also coalesces in-flight requests. Different feeds refresh independently.
    async fn snapshot(&self, source: Source, market: Market) -> Option<(Snapshot, bool)> {
        let key = (
            source,
            if source == Source::RateAm {
                market
            } else {
                Market::Noncash
            },
        );
        let mut cache = self.caches[&key].lock().await;
        let fresh = cache
            .snapshot
            .as_ref()
            .is_some_and(|s| s.fetched.elapsed() < source.ttl());
        let retry_pending = cache.failed_at.is_some_and(|t| t.elapsed() < RETRY);
        if !(fresh && cache.failed_at.is_none()) && !retry_pending {
            match self.fetch(source, market).await {
                Ok(snapshot) => {
                    cache.snapshot = Some(snapshot);
                    cache.failed_at = None;
                }
                Err(error) => {
                    tracing::warn!(source = source.name(), %error, "reference rate refresh failed");
                    cache.failed_at = Some(Instant::now());
                }
            }
        }
        cache
            .snapshot
            .as_ref()
            .filter(|s| s.fetched.elapsed() < source.max_age())
            .map(|s| {
                (
                    s.clone(),
                    cache.failed_at.is_some() || s.fetched.elapsed() >= source.ttl(),
                )
            })
    }

    async fn fetch(&self, source: Source, market: Market) -> Result<Snapshot> {
        let mut response = self
            .client
            .get(&self.endpoints[&source])
            .send()
            .await?
            .error_for_status()?;
        // Bound unexpected upstream responses, including chunked bodies.
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                bail!("rate response exceeds 2 MiB");
            }
            bytes.extend_from_slice(&chunk);
        }
        let text = if source == Source::Cbr {
            encoding_rs::WINDOWS_1251.decode(&bytes).0.into_owned()
        } else {
            String::from_utf8(bytes)?
        };
        let rates = match source {
            Source::Cbr | Source::Nbk => parse_xml(source, &text)?,
            Source::Nbrb | Source::Nbu => parse_json(source, &text)?,
            Source::RateAm => parse_rateam(&text, market)?,
        };
        if !rates.contains_key("USD") {
            bail!("feed has no valid USD benchmark");
        }
        Ok(Snapshot {
            rates,
            fetched_at: Utc::now().to_rfc3339(),
            fetched: Instant::now(),
        })
    }

    pub async fn list(
        &self,
        from: &str,
        to: &str,
        market: Market,
    ) -> Result<ReferenceRatesResponse> {
        let from = from.trim().to_ascii_uppercase();
        let to = to.trim().to_ascii_uppercase();
        if !FIAT.contains(&from.as_str()) || !FIAT.contains(&to.as_str()) {
            bail!("supported fiat: AMD, RUB, USD, BYN, UAH, KZT");
        }
        let mut result = ReferenceRatesResponse {
            from: from.clone(),
            to: to.clone(),
            market,
            rates: vec![],
            unavailable_sources: vec![],
        };
        if from == to {
            result.rates.push(ReferenceRate {
                from,
                to,
                rate: 1.0,
                sources: vec![],
                method: "identity",
                sample_count: 1,
                stale: false,
            });
            return Ok(result);
        }
        let rub_amd = (from == "RUB" && to == "AMD") || (from == "AMD" && to == "RUB");
        if rub_amd {
            let (cbr, rateam) = tokio::join!(
                self.snapshot(Source::Cbr, market),
                self.snapshot(Source::RateAm, market)
            );
            for (source, snapshot) in [(Source::Cbr, cbr), (Source::RateAm, rateam)] {
                match snapshot.and_then(|(s, stale)| direct_rate(source, &s, &from, &to, stale)) {
                    Some(rate) => result.rates.push(rate),
                    None => result.unavailable_sources.push(unavailable(source)),
                }
            }
            return Ok(result);
        }
        let from_source = currency_source(&from);
        let to_source = currency_source(&to);
        let (left, right) = tokio::join!(
            async {
                match from_source {
                    Some(s) => self.snapshot(s, market).await,
                    None => None,
                }
            },
            async {
                match to_source {
                    Some(s) => self.snapshot(s, market).await,
                    None => None,
                }
            },
        );
        let mut sources = vec![];
        let mut stale = false;
        let mut count = 1;
        let mut usd_values = vec![];
        for (currency, source, snapshot) in [(&from, from_source, left), (&to, to_source, right)] {
            if currency == "USD" {
                usd_values.push(1.0);
                continue;
            }
            let source = source.expect("non-USD fiat has a source");
            if let Some(rate) =
                snapshot.and_then(|(s, stale)| direct_rate(source, &s, currency, "USD", stale))
            {
                usd_values.push(rate.rate);
                sources.extend(rate.sources);
                stale |= rate.stale;
                if rate.method == "median_bank_midpoint" {
                    count = rate.sample_count;
                }
            } else {
                result.unavailable_sources.push(unavailable(source));
            }
        }
        if usd_values.len() == 2 {
            let rate = usd_values[0] / usd_values[1];
            if valid(rate) {
                let cross = from != "USD" && to != "USD";
                result.rates.push(ReferenceRate {
                    from,
                    to,
                    rate,
                    sources,
                    method: if cross {
                        "cross_via_usd"
                    } else if from_source == Some(Source::RateAm)
                        || to_source == Some(Source::RateAm)
                    {
                        "median_bank_midpoint"
                    } else {
                        "official"
                    },
                    sample_count: count,
                    stale,
                });
            }
        }
        Ok(result)
    }
}

pub async fn reference_rates(
    State(state): State<AppState>,
    Query(query): Query<ReferenceRatesQuery>,
) -> Result<Json<ReferenceRatesResponse>, AppError> {
    state
        .reference_rates
        .list(&query.from, &query.to, query.market)
        .await
        .map(Json)
        .map_err(|e| AppError::BadRequest(e.to_string()))
}

fn currency_source(currency: &str) -> Option<Source> {
    match currency {
        "AMD" => Some(Source::RateAm),
        "RUB" => Some(Source::Cbr),
        "BYN" => Some(Source::Nbrb),
        "UAH" => Some(Source::Nbu),
        "KZT" => Some(Source::Nbk),
        _ => None,
    }
}
fn unavailable(source: Source) -> UnavailableSource {
    UnavailableSource {
        name: source.name().into(),
        url: source.url().into(),
    }
}
fn valid(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
fn number(value: &str) -> Option<f64> {
    value
        .trim()
        .replace(',', ".")
        .parse()
        .ok()
        .filter(|v| valid(*v))
}
fn date(value: &str) -> Option<String> {
    NaiveDate::parse_from_str(value, "%d.%m.%Y")
        .ok()
        .map(|v| v.to_string())
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|v| v.to_string())
        })
}
fn direct_rate(
    source: Source,
    snapshot: &Snapshot,
    from: &str,
    to: &str,
    stale: bool,
) -> Option<ReferenceRate> {
    let identity = Observation {
        value: 1.0,
        count: usize::MAX,
        published_at: String::new(),
    };
    let left = if from == source.base() {
        &identity
    } else {
        snapshot.rates.get(from)?
    };
    let right = if to == source.base() {
        &identity
    } else {
        snapshot.rates.get(to)?
    };
    let rate = left.value / right.value;
    if !valid(rate) {
        return None;
    }
    let published_at = [left, right]
        .iter()
        .filter(|o| !o.published_at.is_empty())
        .map(|o| &o.published_at)
        .min()?
        .clone();
    Some(ReferenceRate {
        from: from.into(),
        to: to.into(),
        rate,
        sources: vec![Attribution {
            name: source.name().into(),
            url: source.url().into(),
            published_at,
            fetched_at: snapshot.fetched_at.clone(),
        }],
        method: if source == Source::RateAm {
            "median_bank_midpoint"
        } else {
            "official"
        },
        sample_count: left.count.min(right.count),
        stale,
    })
}

fn parse_xml(source: Source, text: &str) -> Result<HashMap<String, Observation>> {
    let document = roxmltree::Document::parse(text)?;
    let mut rates = HashMap::new();
    let field = |node: roxmltree::Node<'_, '_>, name: &str| {
        node.children()
            .find(|n| n.has_tag_name(name))
            .and_then(|n| n.text())
            .unwrap_or("")
            .to_owned()
    };
    for node in document.descendants().filter(|n| {
        n.has_tag_name(if source == Source::Cbr {
            "Valute"
        } else {
            "item"
        })
    }) {
        let (currency, value, nominal, published) = if source == Source::Cbr {
            (
                field(node, "CharCode"),
                field(node, "Value"),
                field(node, "Nominal"),
                document
                    .root_element()
                    .attribute("Date")
                    .unwrap_or("")
                    .to_owned(),
            )
        } else {
            (
                field(node, "title"),
                field(node, "description"),
                field(node, "quant"),
                field(node, "pubDate"),
            )
        };
        if let (Some(value), Some(nominal), Some(published_at)) =
            (number(&value), number(&nominal), date(&published))
        {
            let value = value / nominal;
            if valid(value) {
                rates.insert(
                    currency,
                    Observation {
                        value,
                        count: 1,
                        published_at,
                    },
                );
            }
        }
    }
    Ok(rates)
}

fn parse_json(source: Source, text: &str) -> Result<HashMap<String, Observation>> {
    let rows: Vec<Value> = serde_json::from_str(text)?;
    let mut rates = HashMap::new();
    for row in rows {
        let (currency, value, scale, published) = if source == Source::Nbrb {
            (
                row["Cur_Abbreviation"].as_str(),
                row["Cur_OfficialRate"].as_f64(),
                row["Cur_Scale"].as_f64(),
                row["Date"].as_str(),
            )
        } else {
            (
                row["cc"].as_str(),
                row["rate"].as_f64(),
                Some(1.0),
                row["exchangedate"].as_str(),
            )
        };
        if let (Some(currency), Some(value), Some(scale), Some(published_at)) = (
            currency,
            value.filter(|v| valid(*v)),
            scale.filter(|v| valid(*v)),
            published.and_then(date),
        ) {
            let value = value / scale;
            if valid(value) {
                rates.insert(
                    currency.into(),
                    Observation {
                        value,
                        count: 1,
                        published_at,
                    },
                );
            }
        }
    }
    Ok(rates)
}

// Rate.am's Next Flight stream contains JSON records referenced by $<hex-id>.
// Decode JSON only: never evaluate the scripts, flatten all numbers, or count display duplicates.
fn parse_rateam(text: &str, market: Market) -> Result<HashMap<String, Observation>> {
    let html = Html::parse_document(text);
    let selector = Selector::parse("script").expect("script selector");
    let mut stream = String::new();
    for script in html.select(&selector) {
        let script = script.inner_html();
        if let Some(payload) = script
            .strip_prefix("self.__next_f.push(")
            .and_then(|s| s.strip_suffix(')'))
        {
            if let Ok(Value::Array(parts)) = serde_json::from_str::<Value>(payload) {
                if parts.first().and_then(Value::as_u64) == Some(1) {
                    if let Some(part) = parts.get(1).and_then(Value::as_str) {
                        stream.push_str(part);
                    }
                }
            }
        }
    }
    let records: HashMap<&str, Value> = stream
        .lines()
        .filter_map(|line| {
            let (id, json) = line.split_once(':')?;
            Some((id, serde_json::from_str(json).ok()?))
        })
        .collect();
    fn resolve<'a>(value: &'a Value, records: &'a HashMap<&str, Value>) -> Option<&'a Value> {
        let mut value = value;
        for _ in 0..32 {
            match value
                .as_str()
                .and_then(|s| s.strip_prefix('$'))
                .and_then(|id| records.get(id))
            {
                Some(next) => value = next,
                None => return Some(value),
            }
        }
        None
    }
    let mut banks = vec![];
    fn find_banks<'a>(
        value: &'a Value,
        records: &'a HashMap<&str, Value>,
        banks: &mut Vec<&'a Value>,
    ) {
        match value {
            Value::Object(object) => {
                if let Some(rows) = object
                    .get("aggregateRates")
                    .and_then(|v| resolve(v, records))
                    .and_then(Value::as_array)
                {
                    banks.extend(rows);
                }
                for child in object.values() {
                    find_banks(child, records, banks);
                }
            }
            Value::Array(array) => {
                for child in array {
                    find_banks(child, records, banks);
                }
            }
            _ => {}
        }
    }
    for record in records.values() {
        find_banks(record, &records, &mut banks);
    }
    let mut seen = HashSet::new();
    let mut observations: HashMap<String, Vec<(f64, String)>> = HashMap::new();
    for bank in banks {
        let bank = resolve(bank, &records).context("invalid bank reference")?;
        if bank["organizationType"] != "BANK" {
            continue;
        }
        let Some(id) = bank["code"].as_str() else {
            continue;
        };
        if !seen.insert(id) {
            continue;
        }
        let Some(timestamp) = bank["lastUpdated"]
            .as_i64()
            .and_then(DateTime::from_timestamp_millis)
        else {
            continue;
        };
        // Quotes with missing or old provider timestamps are not refreshed by fetching the page.
        if timestamp > Utc::now() + chrono::Duration::minutes(5)
            || Utc::now() - timestamp > chrono::Duration::hours(72)
        {
            continue;
        }
        let Some(rates) = resolve(&bank["rates"], &records).and_then(Value::as_object) else {
            continue;
        };
        for (currency, quotes) in rates {
            if !["USD", "RUR", "RUB"].contains(&currency.as_str()) {
                continue;
            }
            let Some(quotes) = resolve(quotes, &records) else {
                continue;
            };
            let Some(quote) = resolve(
                &quotes[if market == Market::Cash {
                    "CASH"
                } else {
                    "CLEARING"
                }],
                &records,
            ) else {
                continue;
            };
            let read = |v: &Value| {
                v.as_str()
                    .and_then(number)
                    .or_else(|| v.as_f64().filter(|v| valid(*v)))
            };
            if let (Some(buy), Some(sell)) = (read(&quote["buy"]), read(&quote["sell"])) {
                if buy <= sell {
                    observations
                        .entry(if currency == "RUR" {
                            "RUB".into()
                        } else {
                            currency.clone()
                        })
                        .or_default()
                        .push((buy / 2.0 + sell / 2.0, timestamp.to_rfc3339()));
                }
            }
        }
    }
    let rates = observations
        .into_iter()
        .filter_map(|(currency, mut samples)| {
            samples.sort_by(|a, b| a.0.total_cmp(&b.0));
            let count = samples.len();
            let mid = count / 2;
            let value = if count % 2 == 1 {
                samples[mid].0
            } else {
                samples[mid - 1].0 / 2.0 + samples[mid].0 / 2.0
            };
            let published_at = samples.iter().map(|(_, date)| date).min()?.clone();
            Some((
                currency,
                Observation {
                    value,
                    count,
                    published_at,
                },
            ))
        })
        .collect::<HashMap<_, _>>();
    if rates.is_empty() {
        return Err(anyhow!("no valid Rate.am bank quotes"));
    }
    Ok(rates)
}

#[cfg(test)]
mod tests;
