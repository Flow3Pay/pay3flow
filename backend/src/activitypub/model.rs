use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::activitypub::context::{AS_CONTEXT, FEP0837_CONTEXT};

pub const AS_PUBLIC: &str = "https://www.w3.org/ns/activitystreams#Public";
pub const ACTIVITY_OFFER: &str = "Offer";
pub const ACTIVITY_CREATE: &str = "Create";
pub const ACTIVITY_ACCEPT: &str = "Accept";
pub const ACTIVITY_FOLLOW: &str = "Follow";
pub const ACTIVITY_REJECT: &str = "Reject";
pub const OBJECT_PROPOSAL: &str = "Proposal";
pub const OBJECT_NOTE: &str = "Note";
pub const OBJECT_INTENT: &str = "Intent";

/// ActivityStreams 2.0 actor document as served on the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorDocument {
    #[serde(rename = "@context")]
    pub context: Value,
    pub id: String,
    #[serde(rename = "type")]
    pub atype: Vec<String>,
    #[serde(rename = "preferredUsername")]
    pub preferred_username: String,
    pub name: String,
    pub summary: String,
    pub inbox: String,
    pub outbox: String,
    pub url: String,
    pub endpoints: Value,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "publicKey")]
    pub public_key: Option<PublicKey>,
    #[serde(default)]
    pub attachment: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub id: String,
    pub owner: String,
    #[serde(rename = "publicKeyPem")]
    pub public_key_pem: String,
}

/// A validated fep/0837 proposal (offer or request) that Pay3Flow publishes.
#[derive(Debug, Clone)]
pub struct Proposal {
    pub id: String,
    pub purpose: String,
    pub attributed_to: String,
    pub name: String,
    pub content: String,
    pub resource_conforms_to: String,
    pub action: String,
    pub resource_unit: String,
    pub attachments: Vec<Value>,
}

impl Proposal {
    /// Render as a full fep/0837 Proposal activity (purpose-aware).
    pub fn to_activity(&self) -> Value {
        let mut activity = json!({
            "@context": [
                AS_CONTEXT,
                FEP0837_CONTEXT
            ],
            "id": self.id,
            "type": OBJECT_PROPOSAL,
            "purpose": self.purpose,
            "attributedTo": self.attributed_to,
            "name": self.name,
            "content": self.content,
            "publishes": {
                "id": format!("{}#intent", self.id),
                "type": OBJECT_INTENT,
                "action": self.action,
                "resourceConformsTo": self.resource_conforms_to,
                "resourceQuantity": {
                    "hasUnit": self.resource_unit,
                    "hasNumericalValue": "1"
                }
            },
            "to": [AS_PUBLIC]
        });
        if !self.attachments.is_empty() {
            activity["attachment"] = Value::Array(self.attachments.clone());
        }
        activity
    }
}

/// outbound `Follow` for the marketplace subscription handshake.
pub fn follow_activity(id: &str, actor: &str, object: &str) -> Value {
    json!({
        "@context": AS_CONTEXT,
        "id": format!("{id}/follow"),
        "type": ACTIVITY_FOLLOW,
        "actor": actor,
        "object": object,
    })
}

/// inbound `Accept` reply to a `Follow`.
pub fn accept_follow(activity: &Value, actor_id: &str) -> Value {
    let object = activity.get("id").cloned().unwrap_or_else(|| json!({}));
    json!({
        "@context": AS_CONTEXT,
        "id": format!("{actor_id}/accepts/{}", uuid::Uuid::new_v4()),
        "type": ACTIVITY_ACCEPT,
        "actor": actor_id,
        "object": object,
    })
}

/// A fep/0837 request proposal (task submission) sent into the fmatch inbox.
/// Top-level `command` and `resultInbox` are read by fmatch's request router.
pub fn request_proposal(
    id: &str,
    actor: &str,
    resource: &str,
    command: &str,
    content: &str,
    result_inbox: &str,
) -> Value {
    json!({
        "@context": [
            AS_CONTEXT,
            FEP0837_CONTEXT
        ],
        "id": id,
        "type": OBJECT_PROPOSAL,
        "purpose": "request",
        "attributedTo": actor,
        "name": format!("pay3flow request: {command}"),
        "content": content,
        "command": command,
        "resultInbox": result_inbox,
        "publishes": {
            "id": format!("{id}#intent"),
            "type": OBJECT_INTENT,
            "action": "deliverService",
            "resourceConformsTo": resource,
            "resourceQuantity": {
                "hasUnit": "one",
                "hasNumericalValue": "1"
            }
        },
        "to": [AS_PUBLIC]
    })
}

/// A solver's Offer answer to a delivered Ticket. fmatch extracts `content`
/// as the candidate answer (must not look like a pure acknowledgement or a
/// failure notice), so this is the synchronous body returned from our inbox.
pub fn solver_answer(actor_id: &str, task_ref: &str, in_reply_to: &str, content: &str) -> Value {
    let id = format!(
        "{}/activities/solver-answer/{}",
        actor_id.trim_end_matches('/'),
        uuid::Uuid::new_v4()
    );
    json!({
        "@context": [
            AS_CONTEXT,
            FEP0837_CONTEXT
        ],
        "id": id,
        "type": ACTIVITY_OFFER,
        "actor": actor_id,
        "object": {
            "type": "Ticket",
            "id": format!("{id}#ticket"),
            "taskRef": task_ref,
            "inReplyTo": in_reply_to,
            "activity": "result",
            "command": "#result",
            "status": "completed",
            "responseMode": "answer",
            "content": content,
        }
    })
}

/// Parse the public candidate summaries fmatch includes in its replies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquirerCandidate {
    pub name: String,
    #[serde(rename = "shortId", default)]
    pub short_id: String,
    pub rank: u64,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub quality: Option<f64>,
    #[serde(skip)]
    pub raw: Value,
}

impl AcquirerCandidate {
    /// Collect candidates from a fmatch reply.
    ///
    /// Supported surface: a top-level `candidates` array (executor response)
    /// or an `Offer(Agreement)` whose `object` carries `candidates`.
    pub fn from_reply(reply: &Value) -> Vec<AcquirerCandidate> {
        let mut out = Vec::new();
        if let Some(list) = reply.get("candidates").and_then(Value::as_array) {
            for raw in list {
                if let Ok(mut c) = serde_json::from_value::<AcquirerCandidate>(raw.clone()) {
                    c.raw = raw.clone();
                    out.push(c);
                }
            }
        }
        if out.is_empty() {
            if let Some(object) = reply.get("object") {
                if let Some(list) = object.get("candidates").and_then(Value::as_array) {
                    for raw in list {
                        if let Ok(mut c) = serde_json::from_value::<AcquirerCandidate>(raw.clone())
                        {
                            c.raw = raw.clone();
                            out.push(c);
                        }
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_has_fep_context_and_intent() {
        let p = Proposal {
            id: "https://pay3flow.local/acquirers/stripe#offer".into(),
            purpose: "offer".into(),
            attributed_to: "https://pay3flow.local/actor/pay3flow".into(),
            name: "Stripe".into(),
            content: "geo:US; fee:3.1%".into(),
            resource_conforms_to: "https://pay3flow.local/marketplace/resources/acquiring".into(),
            action: "deliverService".into(),
            resource_unit: "one".into(),
            attachments: vec![],
        };
        let activity = p.to_activity();
        assert_eq!(activity["type"], json!(OBJECT_PROPOSAL));
        assert_eq!(activity["purpose"], json!("offer"));
        let ctx = activity["@context"].as_array().unwrap();
        assert!(ctx.contains(&json!(AS_CONTEXT)));
        assert!(ctx.contains(&json!(FEP0837_CONTEXT)));
        assert_eq!(activity["publishes"]["action"], json!("deliverService"));
        assert_eq!(
            activity["publishes"]["resourceConformsTo"],
            json!("https://pay3flow.local/marketplace/resources/acquiring")
        );
    }

    #[test]
    fn follow_uses_as_context_and_target() {
        let f = follow_activity(
            "https://pay3flow.local/actor/pay3flow",
            "https://pay3flow.local/actor/pay3flow",
            "http://localhost:7277/actor/actra",
        );
        assert_eq!(f["type"], json!(ACTIVITY_FOLLOW));
        assert_eq!(f["actor"], json!("https://pay3flow.local/actor/pay3flow"));
        assert_eq!(f["object"], json!("http://localhost:7277/actor/actra"));
    }

    #[test]
    fn parses_candidates_from_executor_reply() {
        let reply = json!({
            "type": "Offer",
            "actor": "http://localhost:7277/actor/actra",
            "candidates": [
                {"name": "Stripe", "shortId": "s1", "rank": 1, "price": 0.031, "quality": 0.9}
            ]
        });
        let cands = AcquirerCandidate::from_reply(&reply);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].name, "Stripe");
        assert_eq!(cands[0].rank, 1);
        assert_eq!(cands[0].price, Some(0.031));
    }
}
