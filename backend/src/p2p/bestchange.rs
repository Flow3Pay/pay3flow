use std::collections::HashSet;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use futures::future::join_all;
use tokio::sync::OnceCell;

use crate::p2p::service::{
    Advertiser, P2pOffer, P2pOfferMarket, P2pSearchQuery, P2pSide, P2pSource,
};
use crate::provider_adapter::BestChangeAdapterConfig;
use crate::providers::ProviderAdapterRecord;

const MAX_UNIT_COMBINATIONS: usize = 12;
const DEFAULT_MAX_AMOUNT: f64 = 1_000_000_000_000_000.0;

#[derive(Debug, Clone)]
struct CatalogUnit {
    code: String,
    slug: String,
    search_text: String,
    popularity: i64,
}

#[derive(Debug)]
struct Catalog {
    units: Vec<CatalogUnit>,
}

#[derive(Debug)]
struct PublicRow {
    name: String,
    source_url: String,
    source_amount: f64,
    target_amount: f64,
    minimum_source: Option<f64>,
}

pub(crate) struct BestChangeSource {
    client: reqwest::Client,
    slug: String,
    config: BestChangeAdapterConfig,
    metadata: OnceCell<Catalog>,
}

impl BestChangeSource {
    pub(crate) fn from_record(
        client: reqwest::Client,
        record: &ProviderAdapterRecord,
    ) -> Option<Self> {
        let config = record.config.as_ref()?.bestchange.clone()?;
        Some(Self {
            client,
            slug: record.slug.clone(),
            config,
            metadata: OnceCell::new(),
        })
    }

    fn catalog_url(&self) -> String {
        format!(
            "{}/{}",
            self.config.endpoint.trim_end_matches('/'),
            self.config.language
        )
    }

    fn direction_url(&self, from: &CatalogUnit, to: &CatalogUnit, amount: Option<f64>) -> String {
        let mut url = format!(
            "{}/{}/{}-to-{}",
            self.config.endpoint.trim_end_matches('/'),
            self.config.language,
            from.slug,
            to.slug
        );
        if let Some(amount) = amount.filter(|amount| amount.is_finite() && *amount > 0.0) {
            url.push_str("?fromAmount=");
            url.push_str(&number(amount));
        }
        url
    }

    async fn html(&self, url: String) -> Result<String> {
        let response = self
            .client
            .get(url)
            .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
            .send()
            .await
            .context("BestChange public page request failed")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!(
                "BestChange public page returned {status}: {}",
                truncate(&body, 240)
            );
        }
        response
            .text()
            .await
            .context("BestChange public page returned invalid text")
    }

    async fn metadata(&self) -> Result<&Catalog> {
        self.metadata
            .get_or_try_init(|| async {
                let html = self.html(self.catalog_url()).await?;
                let units = parse_catalog(&html);
                if units.is_empty() {
                    bail!("BestChange public catalog contained no exchange units");
                }
                Ok(Catalog { units })
            })
            .await
            .map_err(|error: anyhow::Error| error)
    }

    fn offers_from_page(
        &self,
        query: &P2pSearchQuery,
        from: &CatalogUnit,
        to: &CatalogUnit,
        html: &str,
    ) -> Vec<P2pOffer> {
        parse_rows(html)
            .into_iter()
            .enumerate()
            .filter_map(|(index, row)| {
                if row.source_amount <= 0.0 || row.target_amount <= 0.0 {
                    return None;
                }
                let (price, available_asset, min_fiat, max_fiat) = match query.side {
                    P2pSide::BuyCrypto => (
                        row.source_amount / row.target_amount,
                        row.target_amount,
                        row.minimum_source.unwrap_or(0.0),
                        DEFAULT_MAX_AMOUNT,
                    ),
                    P2pSide::SellCrypto => (
                        row.target_amount / row.source_amount,
                        DEFAULT_MAX_AMOUNT,
                        row.minimum_source
                            .map(|minimum| minimum * row.target_amount / row.source_amount)
                            .unwrap_or(0.0),
                        DEFAULT_MAX_AMOUNT,
                    ),
                };
                if !price.is_finite() || price <= 0.0 {
                    return None;
                }
                Some(P2pOffer {
                    market: P2pOfferMarket::P2p,
                    source: self.slug.clone(),
                    ad_id: format!(
                        "bestchange-public-{}-{}-{}-{}",
                        from.slug,
                        to.slug,
                        normalize(&row.name),
                        index
                    ),
                    side: query.side,
                    fiat: query.fiat.clone(),
                    asset: query.asset.clone(),
                    price: number(price),
                    available_asset: number(available_asset),
                    min_fiat: number(min_fiat),
                    max_fiat: number(max_fiat),
                    payment_methods: query.payment_method.clone().into_iter().collect(),
                    pay_time_limit_minutes: None,
                    advertiser: Advertiser {
                        id: None,
                        nickname: row.name,
                        user_type: Some("service".into()),
                        is_merchant: true,
                        is_verified: false,
                        completed_orders_30d: None,
                        completion_rate_30d: None,
                        positive_rate: None,
                    },
                    advertiser_profile_url: None,
                    source_url: row.source_url,
                    source_url_is_exact: false,
                })
            })
            .collect()
    }
}

#[async_trait]
impl P2pSource for BestChangeSource {
    fn name(&self) -> &str {
        &self.slug
    }

    fn timeout(&self, _default: Duration) -> Duration {
        Duration::from_millis(self.config.timeout_ms)
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        let catalog = self.metadata().await?;
        let (from_code, to_code) = match query.side {
            P2pSide::BuyCrypto => (&query.fiat, &query.asset),
            P2pSide::SellCrypto => (&query.asset, &query.fiat),
        };
        let from_units = select_units(
            &catalog.units,
            from_code,
            query.payment_method.as_deref(),
            3,
        );
        let to_units = select_units(&catalog.units, to_code, None, 6);
        if from_units.is_empty() || to_units.is_empty() {
            return Err(anyhow!(
                "BestChange has no public exchange-unit mapping for {} → {}",
                from_code,
                to_code
            ));
        }

        let combinations = from_units
            .iter()
            .flat_map(|from| to_units.iter().map(move |to| (from, to)))
            .take(MAX_UNIT_COMBINATIONS)
            .collect::<Vec<_>>();
        let amount = query.amount;
        let pages = join_all(combinations.iter().map(|(from, to)| async move {
            let url = self.direction_url(from, to, amount);
            let result = self.html(url).await;
            (*from, *to, result)
        }))
        .await;

        let mut offers = Vec::new();
        let mut last_error = None;
        for (from, to, page) in pages {
            match page {
                Ok(html) => offers.extend(self.offers_from_page(query, from, to, &html)),
                Err(error) => last_error = Some(error),
            }
            if offers.len() >= self.config.max_results {
                break;
            }
        }
        if offers.is_empty() {
            if let Some(error) = last_error {
                return Err(error);
            }
        }
        offers.truncate(self.config.max_results);
        Ok(offers)
    }
}

fn select_units<'a>(
    units: &'a [CatalogUnit],
    code: &str,
    payment_method: Option<&str>,
    limit: usize,
) -> Vec<&'a CatalogUnit> {
    let code = normalize(code);
    let payment = payment_method.map(normalize);
    let mut candidates = units
        .iter()
        .filter(|unit| normalize(&unit.code) == code)
        .filter(|unit| {
            payment.as_deref().is_none_or(|payment| {
                unit.search_text.contains(payment)
                    || (payment.contains("visa") && unit.slug.starts_with("visa-mastercard-"))
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        let left_exact = payment
            .as_deref()
            .is_some_and(|payment| normalize(&left.slug).contains(payment));
        let right_exact = payment
            .as_deref()
            .is_some_and(|payment| normalize(&right.slug).contains(payment));
        right_exact
            .cmp(&left_exact)
            .then_with(|| right.popularity.cmp(&left.popularity))
    });
    candidates.dedup_by(|left, right| left.slug == right.slug);
    candidates.truncate(limit);
    candidates
}

fn parse_catalog(html: &str) -> Vec<CatalogUnit> {
    let mut units = Vec::new();
    let mut cursor = 0;
    let mut seen = HashSet::new();
    while let Some(relative) = html[cursor..].find("\"code\":\"") {
        let start = cursor + relative;
        let end = html[start..]
            .find('}')
            .map(|offset| start + offset)
            .unwrap_or(html.len());
        let record = &html[start..end];
        let Some(code) = json_string(record, "code") else {
            cursor = start + 8;
            continue;
        };
        let Some(slug) = json_string(record, "slug") else {
            cursor = start + 8;
            continue;
        };
        if !seen.insert(slug.clone()) {
            cursor = end.saturating_add(1);
            continue;
        }
        let title = json_string(record, "title").unwrap_or_default();
        let description = json_string(record, "description").unwrap_or_default();
        units.push(CatalogUnit {
            code,
            slug,
            search_text: normalize(&format!("{record} {title} {description}")),
            popularity: json_number(record, "popularityScoreBoth").unwrap_or_default(),
        });
        cursor = end.saturating_add(1);
    }
    units
}

fn parse_rows(html: &str) -> Vec<PublicRow> {
    let marker = "data-test-id=\"exchange-rates-table-row\"";
    let mut rows = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = html[cursor..].find(marker) {
        let marker_start = cursor + relative;
        let start = html[..marker_start].rfind("<tr").unwrap_or(marker_start);
        let Some(end_relative) = html[marker_start..].find("</tr>") else {
            break;
        };
        let end = marker_start + end_relative + "</tr>".len();
        if let Some(row) = parse_row(&html[start..end]) {
            rows.push(row);
        }
        cursor = end;
    }
    rows
}

fn parse_row(row: &str) -> Option<PublicRow> {
    let source_url = html_attribute(row, "data-href")?;
    let name_start = row.find("text-label-primary")?;
    let name = span_text(row, name_start)?;
    let offer_start = row.find("Offer-module")?;
    let offer = &row[offer_start..];
    let amounts = class_span_numbers(offer, "text-body-regular-14");
    let [source_amount, target_amount, ..] = amounts.as_slice() else {
        return None;
    };
    Some(PublicRow {
        name,
        source_url,
        source_amount: *source_amount,
        target_amount: *target_amount,
        minimum_source: minimum_amount(row),
    })
}

fn class_span_numbers(html: &str, class_fragment: &str) -> Vec<f64> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = html[cursor..].find("<span") {
        let start = cursor + relative;
        let Some(tag_end_relative) = html[start..].find('>') else {
            break;
        };
        let tag_end = start + tag_end_relative;
        let tag = &html[start..=tag_end];
        if tag.contains("class=") && tag.contains(class_fragment) {
            if let Some(close_relative) = html[tag_end + 1..].find("</span>") {
                let close = tag_end + 1 + close_relative;
                if let Some(value) = parse_amount(&strip_tags(&html[tag_end + 1..close])) {
                    values.push(value);
                }
                cursor = close + "</span>".len();
                continue;
            }
        }
        cursor = tag_end + 1;
    }
    values
}

fn minimum_amount(row: &str) -> Option<f64> {
    [
        "Минимальная сумма обмена по данному курсу",
        "Minimum exchange amount at this rate",
    ]
    .into_iter()
    .find_map(|marker| {
        let start = row.find(marker)?;
        parse_amount(&strip_tags(&row[start + marker.len()..]))
    })
}

fn span_text(html: &str, marker_start: usize) -> Option<String> {
    let start = html[..marker_start].rfind("<span")?;
    let tag_end = start + html[start..].find('>')?;
    let end = tag_end + 1 + html[tag_end + 1..].find("</span>")?;
    let text = strip_tags(&html[tag_end + 1..end]);
    (!text.trim().is_empty()).then(|| text.trim().to_string())
}

fn html_attribute(html: &str, attribute: &str) -> Option<String> {
    let marker = format!("{attribute}=\"");
    let start = html.find(&marker)? + marker.len();
    let end = html[start..].find('"')? + start;
    Some(html_unescape(&html[start..end]))
}

fn json_string(record: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":\"");
    let start = record.find(&marker)? + marker.len();
    let mut value = String::new();
    let mut escaped = false;
    for character in record[start..].chars() {
        if escaped {
            value.push(match character {
                '"' | '\\' | '/' => character,
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some(value);
        } else {
            value.push(character);
        }
    }
    None
}

fn json_number(record: &str, key: &str) -> Option<i64> {
    let marker = format!("\"{key}\":");
    let start = record.find(&marker)? + marker.len();
    let value = record[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '-')
        .collect::<String>();
    value.parse().ok()
}

fn parse_amount(value: &str) -> Option<f64> {
    let value = html_unescape(value).replace('\u{a0}', " ");
    let value = value.trim();
    let mut number = value
        .chars()
        .filter(|character| character.is_ascii_digit() || matches!(character, '.' | ',' | '-'))
        .collect::<String>();
    if number.contains(',') && !number.contains('.') {
        number = number.replace(',', ".");
    } else {
        number = number.replace(',', "");
    }
    number.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn strip_tags(value: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for character in html_unescape(value).chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    text
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn number(value: f64) -> String {
    let mut formatted = format!("{value:.12}");
    while formatted.contains('.') && formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    formatted
}

fn truncate(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_public_units_and_rows() {
        let catalog = parse_catalog(
            r#"{"code":"RUB","slug":"visa-mastercard-rub","title":"Visa/Mastercard","description":"Bank card RUB","popularityScoreBoth":300},{"code":"USDT","slug":"tether-trc20","title":"Tether","description":"USDT TRC20","popularityScoreBoth":400}"#,
        );
        assert_eq!(catalog.len(), 2);
        assert_eq!(
            select_units(&catalog, "RUB", Some("Visa Mastercard"), 3)[0].slug,
            "visa-mastercard-rub"
        );
        assert_eq!(
            select_units(&catalog, "USDT", None, 3)[0].slug,
            "tether-trc20"
        );

        let rows = parse_rows(
            r#"<tr data-test-id="exchange-rates-table-row" data-href="https://example.test/?a=1&amp;b=2"><td><span class="text-label-primary">Example</span></td><td><div class="Offer-module"><span class="text-body-regular-14 whitespace-nowrap">10 000</span><span class="text-body-regular-14 whitespace-nowrap">114.54</span></div></td></tr>"#,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Example");
        assert_eq!(rows[0].source_url, "https://example.test/?a=1&b=2");
        assert_eq!(rows[0].source_amount, 10_000.0);
        assert_eq!(rows[0].target_amount, 114.54);
    }
}
