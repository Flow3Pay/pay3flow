//! FEP-8fba semantic interface for the Pay3Flow exchange service.
//!
//! The wire contract deliberately contains only ActivityStreams objects and
//! links.  A consumer can render the interface with its own UI, CLI, or agent
//! without loading Pay3Flow frontend code.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::core::state::AppState;
use crate::quotes;
use crate::routing::PaymentRequest;

pub const FEP8_CONTEXT: &str = "https://w3id.org/fep/8fba#";
pub const PAY3FLOW_CONTEXT: &str = "https://pay3flow.lefine.pro/ns/pay3flow#";
pub const EXCHANGE_RESOURCE: &str = "marketplace/resources/exchange";

const SOURCE_ASSET: &str = "https://pay3flow.lefine.pro/ns/pay3flow#sourceAsset";
const SOURCE_AMOUNT: &str = "https://pay3flow.lefine.pro/ns/pay3flow#sourceAmount";
const SOURCE_COUNTRY: &str = "https://pay3flow.lefine.pro/ns/pay3flow#sourceCountry";
const SOURCE_METHOD: &str = "https://pay3flow.lefine.pro/ns/pay3flow#sourceMethod";
const TARGET_ASSET: &str = "https://pay3flow.lefine.pro/ns/pay3flow#targetAsset";
const TARGET_AMOUNT: &str = "https://pay3flow.lefine.pro/ns/pay3flow#targetAmount";
const TARGET_COUNTRY: &str = "https://pay3flow.lefine.pro/ns/pay3flow#targetCountry";
const TARGET_METHOD: &str = "https://pay3flow.lefine.pro/ns/pay3flow#targetMethod";
const FEE: &str = "https://pay3flow.lefine.pro/ns/pay3flow#fee";
const ROUTE: &str = "https://pay3flow.lefine.pro/ns/pay3flow#route";
const STATUS: &str = "https://pay3flow.lefine.pro/ns/pay3flow#status";
const CANDIDATES: &str = "https://pay3flow.lefine.pro/ns/pay3flow#candidates";
const QUOTED_AT: &str = "https://pay3flow.lefine.pro/ns/pay3flow#quotedAt";

const PREVIEW: &str = "https://w3id.org/fep/8fba#preview";
const INVOKE: &str = "https://w3id.org/fep/8fba#invoke";

fn context() -> Value {
    json!([
        "https://www.w3.org/ns/activitystreams",
        "https://w3id.org/fep/0837",
        {
            "fep": FEP8_CONTEXT,
            "pay3flow": PAY3FLOW_CONTEXT,
            "interface": {"@id": "fep:interface", "@type": "@id"},
            "bind": {"@id": "fep:bind", "@type": "@id"},
            "inputShape": {"@id": "fep:inputShape", "@type": "@id"},
            "outputShape": {"@id": "fep:outputShape", "@type": "@id"},
            "shape": {"@id": "fep:shape", "@type": "@id"}
        }
    ])
}

fn shacl_context() -> Value {
    json!({
        "sh": "http://www.w3.org/ns/shacl#",
        "xsd": "http://www.w3.org/2001/XMLSchema#",
        "pay3flow": PAY3FLOW_CONTEXT
    })
}

fn origin_path(origin: &str, path: &str) -> String {
    format!("{}/{}", origin.trim_end_matches('/'), path)
}

pub fn exchange_resource(origin: &str) -> Value {
    let interface = origin_path(origin, "marketplace/interfaces/exchange");
    json!({
        "@context": context(),
        "id": origin_path(origin, EXCHANGE_RESOURCE),
        "type": "ResourceSpecification",
        "name": "Cross-border exchange",
        "summary": "Discover and execute a consent-gated cross-border exchange route.",
        "action": "deliverService",
        "purpose": "offer",
        "resourceConformsTo": "https://w3id.org/fep/0837",
        "interface": interface
    })
}

pub fn exchange_proposal(origin: &str, actor_id: &str, audience: &str) -> Value {
    let proposal_id = origin_path(origin, "marketplace/proposals/exchange");
    let interface = origin_path(origin, "marketplace/interfaces/exchange");
    json!({
        "@context": context(),
        "id": proposal_id,
        "type": "Proposal",
        "purpose": "offer",
        "attributedTo": actor_id,
        "name": "Pay3Flow cross-border exchange",
        "content": "Exchange a source asset and amount for a target asset through a discovered route. Quotes are previews; funding requires explicit user consent.",
        "interface": interface,
        "publishes": {
            "id": format!("{proposal_id}#intent"),
            "type": "Intent",
            "action": "deliverService",
            "resourceConformsTo": origin_path(origin, EXCHANGE_RESOURCE),
            "resourceQuantity": {
                "hasUnit": "exchange",
                "hasNumericalValue": "1"
            },
            "interface": interface
        },
        "to": [audience]
    })
}

pub fn create_exchange_proposal(origin: &str, actor_id: &str, audience: &str) -> Value {
    let proposal = exchange_proposal(origin, actor_id, audience);
    json!({
        "@context": context(),
        "id": format!("{}/activities/exchange-proposal", actor_id.trim_end_matches('/')),
        "type": "Create",
        "actor": actor_id,
        "object": proposal,
        "to": [audience]
    })
}

pub fn interface_collection(origin: &str) -> Value {
    let interface_id = origin_path(origin, "marketplace/interfaces/exchange");
    let input_shape = origin_path(origin, "marketplace/shapes/exchange-input");
    let output_shape = origin_path(origin, "marketplace/shapes/exchange-output");
    let preview = origin_path(origin, "marketplace/preview");
    let invoke = origin_path(origin, "api/exchange/orders");

    json!({
        "@context": context(),
        "id": interface_id,
        "type": "OrderedCollection",
        "name": "Pay3Flow exchange",
        "summary": "Semantic input, quote preview, and consent-gated exchange invocation.",
        "inputShape": input_shape,
        "outputShape": output_shape,
        "orderedItems": [
            {"type": "Object", "name": "Source asset", "bind": SOURCE_ASSET, "shape": format!("{input_shape}#sourceAsset")},
            {"type": "Object", "name": "Source amount", "bind": SOURCE_AMOUNT, "shape": format!("{input_shape}#sourceAmount")},
            {"type": "Object", "name": "Source country", "bind": SOURCE_COUNTRY, "shape": format!("{input_shape}#sourceCountry")},
            {"type": "Object", "name": "Source method", "bind": SOURCE_METHOD, "shape": format!("{input_shape}#sourceMethod")},
            {"type": "Object", "name": "Target asset", "bind": TARGET_ASSET, "shape": format!("{input_shape}#targetAsset")},
            {"type": "Object", "name": "Target country", "bind": TARGET_COUNTRY, "shape": format!("{input_shape}#targetCountry")},
            {"type": "Object", "name": "Target method", "bind": TARGET_METHOD, "shape": format!("{input_shape}#targetMethod")},
            {"type": "Link", "name": "Preview exchange route", "href": preview, "rel": [PREVIEW], "mediaType": "application/ld+json"},
            {"type": "Object", "name": "Target amount", "bind": TARGET_AMOUNT},
            {"type": "Object", "name": "Fee", "bind": FEE},
            {"type": "Object", "name": "Selected route", "bind": ROUTE},
            {"type": "Object", "name": "Quote status", "bind": STATUS},
            {"type": "Object", "name": "Quote candidates", "bind": CANDIDATES},
            {"type": "Object", "name": "Quoted at", "bind": QUOTED_AT},
            {"type": "Link", "name": "Create exchange order", "href": invoke, "rel": [INVOKE], "mediaType": "application/ld+json"}
        ]
    })
}

pub fn input_shape(origin: &str) -> Value {
    let id = origin_path(origin, "marketplace/shapes/exchange-input");
    json!({
        "@context": shacl_context(),
        "@id": id,
        "@type": "sh:NodeShape",
        "sh:property": [
            property_shape(&format!("{id}#sourceAsset"), SOURCE_ASSET, "Source asset", "xsd:string", 1, 1, 1),
            property_shape(&format!("{id}#sourceAmount"), SOURCE_AMOUNT, "Source amount", "xsd:decimal", 1, 1, 2),
            property_shape(&format!("{id}#sourceCountry"), SOURCE_COUNTRY, "Source country", "xsd:string", 0, 1, 3),
            property_shape(&format!("{id}#sourceMethod"), SOURCE_METHOD, "Source method", "xsd:string", 1, 1, 4),
            property_shape(&format!("{id}#targetAsset"), TARGET_ASSET, "Target asset", "xsd:string", 1, 1, 5),
            property_shape(&format!("{id}#targetCountry"), TARGET_COUNTRY, "Target country", "xsd:string", 0, 1, 6),
            property_shape(&format!("{id}#targetMethod"), TARGET_METHOD, "Target method", "xsd:string", 1, 1, 7)
        ]
    })
}

pub fn output_shape(origin: &str) -> Value {
    let id = origin_path(origin, "marketplace/shapes/exchange-output");
    json!({
        "@context": shacl_context(),
        "@id": id,
        "@type": "sh:NodeShape",
        "sh:property": [
            output_property(TARGET_AMOUNT, "Target amount", "xsd:decimal"),
            output_property(FEE, "Fee", "xsd:decimal"),
            output_property(ROUTE, "Selected route", "xsd:string"),
            output_property(STATUS, "Quote status", "xsd:string"),
            output_property(CANDIDATES, "Quote candidates", "sh:BlankNodeOrIRI"),
            output_property(QUOTED_AT, "Quoted at", "xsd:dateTime")
        ]
    })
}

fn property_shape(
    id: &str,
    path: &str,
    name: &str,
    datatype: &str,
    min_count: u8,
    max_count: u8,
    order: u8,
) -> Value {
    json!({
        "@id": id,
        "sh:path": path,
        "sh:name": name,
        "sh:datatype": datatype,
        "sh:minCount": min_count,
        "sh:maxCount": max_count,
        "sh:order": order
    })
}

fn output_property(path: &str, name: &str, datatype: &str) -> Value {
    json!({"sh:path": path, "sh:name": name, "sh:datatype": datatype})
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewInput {
    pub source_asset: String,
    pub source_amount: f64,
    #[serde(default)]
    pub source_country: Option<String>,
    pub source_method: String,
    pub target_asset: String,
    #[serde(default)]
    pub target_country: Option<String>,
    pub target_method: String,
}

pub async fn preview(State(state): State<AppState>, Json(input): Json<PreviewInput>) -> Response {
    if let Err(message) = validate_preview_input(&input) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": message}))).into_response();
    }

    let request = PaymentRequest {
        amount: input.source_amount,
        currency: input.source_asset.trim().to_ascii_uppercase(),
        from: input.source_method.trim().to_string(),
        to: input.target_method.trim().to_string(),
        to_geo: input.target_country.clone(),
        method: Some(input.source_method.trim().to_string()),
        to_currency: Some(input.target_asset.trim().to_ascii_uppercase()),
    };
    let quote =
        quotes::compute_quote(&state.ap, &state.picker, request, state.redis.as_ref()).await;
    let best = quote.best.as_ref().map(|candidate| candidate.name.clone());
    let candidates = quote
        .candidates
        .iter()
        .map(|candidate| {
            json!({
                "name": candidate.name,
                "shortId": candidate.short_id,
                "rank": candidate.rank,
                "price": candidate.price,
                "quality": candidate.quality
            })
        })
        .collect::<Vec<_>>();

    Json(json!({
        "@context": context(),
        "type": "Object",
        SOURCE_ASSET: input.source_asset.trim().to_ascii_uppercase(),
        SOURCE_AMOUNT: input.source_amount,
        SOURCE_COUNTRY: input.source_country,
        SOURCE_METHOD: input.source_method.trim(),
        TARGET_ASSET: input.target_asset.trim().to_ascii_uppercase(),
        TARGET_COUNTRY: input.target_country,
        TARGET_METHOD: input.target_method.trim(),
        STATUS: "preview",
        ROUTE: best,
        CANDIDATES: candidates,
        QUOTED_AT: quote.quoted_at
    }))
    .into_response()
}

fn validate_preview_input(input: &PreviewInput) -> Result<(), &'static str> {
    if input.source_asset.trim().is_empty() || input.target_asset.trim().is_empty() {
        return Err("sourceAsset and targetAsset are required");
    }
    if !input.source_amount.is_finite() || input.source_amount <= 0.0 {
        return Err("sourceAmount must be a positive finite number");
    }
    if input.source_method.trim().is_empty() || input.target_method.trim().is_empty() {
        return Err("sourceMethod and targetMethod are required");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_exposes_fep8_interface_on_proposal_and_intent() {
        let proposal = exchange_proposal(
            "https://pay3flow.lefine.pro",
            "https://pay3flow.lefine.pro/actor/pay3flow",
            "https://lefine.pro/actors/actra",
        );
        let interface = "https://pay3flow.lefine.pro/marketplace/interfaces/exchange";
        assert_eq!(proposal["type"], "Proposal");
        assert_eq!(proposal["interface"], interface);
        assert_eq!(proposal["publishes"]["interface"], interface);
        assert_eq!(proposal["publishes"]["action"], "deliverService");
    }

    #[test]
    fn interface_orders_preview_before_invoke() {
        let interface = interface_collection("https://pay3flow.lefine.pro");
        let items = interface["orderedItems"].as_array().expect("items");
        let preview = items
            .iter()
            .position(|item| {
                item["rel"]
                    .as_array()
                    .is_some_and(|rels| rels.iter().any(|rel| rel == PREVIEW))
            })
            .expect("preview link");
        let invoke = items
            .iter()
            .position(|item| {
                item["rel"]
                    .as_array()
                    .is_some_and(|rels| rels.iter().any(|rel| rel == INVOKE))
            })
            .expect("invoke link");
        assert!(preview < invoke);
        assert_eq!(
            interface["inputShape"],
            "https://pay3flow.lefine.pro/marketplace/shapes/exchange-input"
        );
        assert_eq!(
            interface["outputShape"],
            "https://pay3flow.lefine.pro/marketplace/shapes/exchange-output"
        );
    }

    #[test]
    fn preview_input_rejects_non_positive_amounts() {
        let input = PreviewInput {
            source_asset: "AMD".into(),
            source_amount: 0.0,
            source_country: None,
            source_method: "bank".into(),
            target_asset: "RUB".into(),
            target_country: None,
            target_method: "bank".into(),
        };
        assert_eq!(
            validate_preview_input(&input),
            Err("sourceAmount must be a positive finite number")
        );
    }
}
