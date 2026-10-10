use anyhow::Context;
use axum::{
    extract::{Path, State},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::core::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateLink {
    target: String,
    #[serde(default)]
    preview: Option<OtcPreview>,
}

#[derive(Debug, Serialize)]
pub struct ShareLink {
    id: String,
    target: String,
    preview: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OtcPreview {
    closes: Vec<f64>,
    bids: Vec<PreviewLevel>,
    asks: Vec<PreviewLevel>,
    captured_at: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PreviewLevel {
    price: f64,
    amount: f64,
}

impl OtcPreview {
    fn valid(&self) -> bool {
        let positive = |value: f64| value.is_finite() && value > 0.0 && value <= 1e15;
        self.closes.len() <= 64
            && self.closes.iter().copied().all(positive)
            && self.bids.len() <= 6
            && self.asks.len() <= 6
            && self
                .bids
                .iter()
                .chain(&self.asks)
                .all(|level| positive(level.price) && positive(level.amount))
            && (0..=10_000_000_000_000).contains(&self.captured_at)
    }
}

fn valid_target(target: &str) -> bool {
    if target.len() > 4096 || target.chars().any(char::is_control) {
        return false;
    }
    if let Some(query) = target.strip_prefix("/#/otc?") {
        return !query.is_empty() && !query.contains('#');
    }
    let path = target.split('?').next().unwrap_or_default();
    let pair = path
        .strip_prefix("/swap/")
        .or_else(|| path.strip_prefix("/share/guide/"));
    let Some(pair) = pair else {
        return false;
    };
    let currencies: Vec<_> = pair.split('/').collect();
    currencies.len() == 2
        && currencies.iter().all(|currency| {
            !currency.is_empty()
                && *currency != "."
                && *currency != ".."
                && currency.len() <= 20
                && currency
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
        && !target.contains('#')
}

fn link_id(target: &str, preview: &Option<serde_json::Value>) -> String {
    let mut hash = Sha256::new();
    hash.update(target.as_bytes());
    if let Some(preview) = preview {
        hash.update(preview.to_string().as_bytes());
    }
    URL_SAFE_NO_PAD.encode(&hash.finalize()[..12])
}

/// Store only an internal settings URL, never an arbitrary redirect destination.
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateLink>,
) -> Result<Json<ShareLink>, AppError> {
    if !valid_target(&input.target) {
        return Err(AppError::BadRequest("Unsupported share settings".into()));
    }
    if let Some(preview) = &input.preview {
        if !input.target.starts_with("/#/otc?") || !preview.valid() {
            return Err(AppError::BadRequest("Invalid OTC preview".into()));
        }
    }
    let preview = input
        .preview
        .map(serde_json::to_value)
        .transpose()
        .context("encode OTC preview")?;
    let client = state
        .pool
        .get()
        .await
        .context("share-link database connection")?;
    let row = client.query_one(
        "INSERT INTO share_links (id, target, preview) VALUES ($1, $2, $3) \
         ON CONFLICT (id) DO UPDATE SET id = EXCLUDED.id WHERE share_links.target = EXCLUDED.target RETURNING id, target, preview",
        &[&link_id(&input.target, &preview), &input.target, &preview],
    ).await.context("store share settings")?;
    Ok(Json(ShareLink {
        id: row.get(0),
        target: row.get(1),
        preview: row.get(2),
    }))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ShareLink>, AppError> {
    if id.len() != 16
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(AppError::NotFound("Share link not found".into()));
    }
    let client = state
        .pool
        .get()
        .await
        .context("share-link database connection")?;
    let row = client
        .query_opt(
            "SELECT id, target, preview FROM share_links WHERE id = $1",
            &[&id],
        )
        .await
        .context("read share settings")?
        .ok_or_else(|| AppError::NotFound("Share link not found".into()))?;
    Ok(Json(ShareLink {
        id: row.get(0),
        target: row.get(1),
        preview: row.get(2),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_accept_only_bounded_internal_exchange_destinations() {
        for target in [
            "/#/otc?market=EVER-USDT&side=sell&amount=100.25&price=0.01234",
            "/swap/USDT/KZT?amount=287.0062069",
            "/share/guide/AMD/RUB?venues=bybit",
        ] {
            assert!(valid_target(target));
            let id = link_id(target, &None);
            assert_eq!(id.len(), 16);
            assert_eq!(id, link_id(target, &None));
        }
        for target in [
            "https://example.com",
            "//example.com",
            "/swap/../example.com",
            "/swap/USD/../../secret",
            "/swap/USD/RUB#bad",
            "/#/otc?\n",
            "/#/otc?market=ETH-USDT#bad",
            "/unknown",
        ] {
            assert!(!valid_target(target), "{target}");
        }
        assert!(!valid_target(&format!(
            "/#/otc?amount={}",
            "1".repeat(4096)
        )));
        assert_ne!(
            link_id("/#/otc?amount=1", &None),
            link_id("/#/otc?amount=2", &None)
        );
    }

    #[test]
    fn create_payload_matches_the_frontend_and_rejects_unexpected_fields() {
        let input: CreateLink =
            serde_json::from_str(r#"{"target":"/#/otc?market=EVER-USDT&amount=1000"}"#).unwrap();
        assert!(valid_target(&input.target));
        assert!(serde_json::from_str::<CreateLink>(
            r#"{"target":"/#/otc?amount=1","redirect":"https://example.com"}"#
        )
        .is_err());
    }

    #[test]
    fn otc_snapshots_validate_real_client_fields_and_limit_rendering_work() {
        let input: CreateLink = serde_json::from_str(r#"{"target":"/#/otc?market=EVER-USDT","preview":{"closes":[0.01,0.0102],"bids":[{"price":0.00999,"amount":1000}],"asks":[{"price":0.01001,"amount":2000}],"capturedAt":1791648000000}}"#).unwrap();
        let mut preview = input.preview.unwrap();
        assert!(preview.valid());
        let value = Some(serde_json::to_value(&preview).unwrap());
        assert_ne!(
            link_id(&input.target, &value),
            link_id(&input.target, &None)
        );
        preview.closes = vec![0.01; 65];
        assert!(!preview.valid());
        preview.closes.clear();
        preview.bids[0].price = f64::INFINITY;
        assert!(!preview.valid());
    }
}
