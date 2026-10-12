use super::{
    amount,
    config::{Config, PROFILE},
    error::{invalid, Result},
    model::{Direction, Terms},
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub(crate) fn context() -> Value {
    serde_json::from_str(include_str!("../../tests/fixtures/otc/context.json"))
        .expect("checked-in JSON-LD context")
}
pub(crate) fn digest(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}
pub(crate) fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid(&format!("missing {key}")))
}
pub(crate) fn author(value: &Value) -> Result<&str> {
    text(
        value,
        if value["type"] == "Proposal" {
            "attributedTo"
        } else {
            "actor"
        },
    )
}
pub(crate) fn addressed(value: &Value, actor: &str) -> bool {
    value.get("to").is_some_and(|v| {
        v.as_str() == Some(actor)
            || v.as_array()
                .is_some_and(|a| a.iter().any(|x| x.as_str() == Some(actor)))
    })
}
pub(crate) fn private(value: &Value) -> bool {
    !["to", "cc", "bto", "bcc"].iter().any(|key| {
        addressed(
            &json!({"to":value.get(key)}),
            "https://www.w3.org/ns/activitystreams#Public",
        )
    })
}
fn intent(id: &str, resource: &str, value: Option<&str>, deadline: Option<DateTime<Utc>>) -> Value {
    let mut result = json!({"id":id,"type":"Intent","action":"transfer","resourceConformsTo":resource,"resourceQuantity":{"hasUnit":"one"}});
    if let Some(value) = value {
        result["resourceQuantity"]["hasNumericalValue"] = json!(value);
    }
    if let Some(deadline) = deadline {
        result["endTime"] = json!(deadline);
    }
    result
}
pub(crate) fn proposal(id: &str, terms: &Terms, cfg: &Config) -> Value {
    let (out_resource, in_resource) = match terms.direction {
        Direction::Buy => (&cfg.ever_resource, &cfg.usdt_resource),
        Direction::Sell => (&cfg.usdt_resource, &cfg.ever_resource),
    };
    json!({"@context":context(),"id":id,"type":"Proposal","purpose":"offer","attributedTo":terms.desk_actor,"to":[terms.customer_actor],"unitBased":false,"endTime":terms.quote_by,
        "publishes":intent(&format!("{id}#primary"),out_resource,Some(&terms.output),Some(terms.payout_by)),
        "reciprocal":intent(&format!("{id}#reciprocal"),in_resource,Some(&terms.input),Some(terms.pay_by))})
}
pub(crate) fn listing(id: &str, direction: Direction, cfg: &Config) -> Value {
    let (out_resource, in_resource) = match direction {
        Direction::Buy => (&cfg.ever_resource, &cfg.usdt_resource),
        Direction::Sell => (&cfg.usdt_resource, &cfg.ever_resource),
    };
    json!({"@context":context(),"id":id,"type":"Proposal","purpose":"offer","attributedTo":cfg.desk_actor,"to":["https://www.w3.org/ns/activitystreams#Public",cfg.matcher_actor],"name":format!("{}: {} EVER", cfg.desk_name, if direction == Direction::Buy { "Buy" } else { "Sell" }),"publishes":intent(&format!("{id}#primary"),out_resource,None,None),"reciprocal":intent(&format!("{id}#reciprocal"),in_resource,None,None)})
}
pub(crate) fn offer(id: &str, terms: &Terms) -> Value {
    let commitment = |field: &str| json!({"type":"Commitment","satisfies":terms.proposal[field]["id"],"resourceQuantity":terms.proposal[field]["resourceQuantity"],"endTime":terms.proposal[field]["endTime"]});
    json!({"@context":context(),"id":id,"type":"OfferAgreement","actor":terms.customer_actor,"to":[terms.desk_actor],"object":{"type":"Agreement","stipulates":commitment("publishes"),"stipulatesReciprocal":commitment("reciprocal")}})
}
pub(crate) fn acceptance(id: &str, offered: &Value, terms: &Terms) -> Value {
    let mut agreement = offered["object"].clone();
    let agreement_id = format!("{id}/agreement");
    agreement["id"] = json!(agreement_id);
    agreement["attributedTo"] = json!(terms.desk_actor);
    agreement["stipulates"]["id"] = json!(format!("{agreement_id}#primary"));
    agreement["stipulatesReciprocal"]["id"] = json!(format!("{agreement_id}#reciprocal"));
    json!({"@context":context(),"id":id,"type":"AcceptAgreement","actor":terms.desk_actor,"to":[terms.customer_actor],"object":offered["id"],"result":agreement})
}
pub(crate) fn validate_offer(value: &Value, terms: &Terms, now: DateTime<Utc>) -> Result<()> {
    if now > terms.quote_by
        || value["type"] != "OfferAgreement"
        || author(value)? != terms.customer_actor
        || !addressed(value, &terms.desk_actor)
        || !private(value)
        || value["object"]["type"] != "Agreement"
    {
        return Err(invalid("invalid or expired OTC offer"));
    }
    text(value, "id")?;
    validate_commitments(&value["object"], terms, false)
}
pub(crate) fn validate_acceptance(value: &Value, offered: &Value, terms: &Terms) -> Result<()> {
    if value["type"] != "AcceptAgreement"
        || author(value)? != terms.desk_actor
        || value["object"] != offered["id"]
        || value["result"]["attributedTo"] != terms.desk_actor
        || !addressed(value, &terms.customer_actor)
        || !private(value)
    {
        return Err(invalid(
            "acceptance does not match the exact customer offer",
        ));
    }
    text(value, "id")?;
    let agreement_id = text(&value["result"], "id")?;
    if value["result"]["type"] != "Agreement"
        || !agreement_id.starts_with(&format!("{}/", terms.desk_actor))
    {
        return Err(invalid("invalid finalized Agreement identity"));
    }
    let mut comparable = value["result"].clone();
    if let Some(object) = comparable.as_object_mut() {
        object.remove("id");
        object.remove("attributedTo");
    }
    for field in ["stipulates", "stipulatesReciprocal"] {
        if let Some(object) = comparable[field].as_object_mut() {
            object.remove("id");
        }
        if !text(&value["result"][field], "id")?.starts_with(&format!("{agreement_id}#")) {
            return Err(invalid("invalid finalized Commitment identity"));
        }
    }
    if comparable != offered["object"] {
        return Err(invalid("finalized Agreement changed the customer offer"));
    }
    validate_commitments(&value["result"], terms, true)
}
fn validate_commitments(agreement: &Value, terms: &Terms, finalized: bool) -> Result<()> {
    for (field, source, precision) in [
        (
            "stipulates",
            "publishes",
            if terms.direction.output_chain() == "ethereum" {
                6
            } else {
                9
            },
        ),
        (
            "stipulatesReciprocal",
            "reciprocal",
            if terms.direction.input_chain() == "ethereum" {
                6
            } else {
                9
            },
        ),
    ] {
        let c = &agreement[field];
        let original = &terms.proposal[source];
        if c["type"] != "Commitment"
            || c["satisfies"] != original["id"]
            || c["resourceQuantity"]["hasUnit"] != original["resourceQuantity"]["hasUnit"]
            || c["endTime"] != original["endTime"]
        {
            return Err(invalid("commitment references, units or deadlines differ"));
        }
        let actual = amount::units(
            text(&c["resourceQuantity"], "hasNumericalValue")?,
            precision,
        )?;
        let expected = amount::units(
            text(&original["resourceQuantity"], "hasNumericalValue")?,
            precision,
        )?;
        if actual != expected {
            return Err(invalid("commitment amount differs"));
        }
        if finalized {
            text(c, "id")?;
        }
    }
    if finalized && agreement["stipulates"]["id"] == agreement["stipulatesReciprocal"]["id"] {
        return Err(invalid("commitment IDs must differ"));
    }
    Ok(())
}
pub(crate) fn profile() -> Value {
    json!({"id":PROFILE,"peer_commit":"848e0c59b41d28a1e4654db5a1803c614de81a51","context_sha256":digest(&context())})
}
