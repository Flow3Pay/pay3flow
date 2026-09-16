use serde_json::Value;

pub const AS_CONTEXT: &str = "https://www.w3.org/ns/activitystreams";
pub const FEP0837_CONTEXT: &str = "https://w3id.org/fep/0837";
pub const VF_CONTEXT: &str = "https://w3id.org/valueflows/ont/vf";
pub const SEC_CONTEXT: &str = "https://w3id.org/security/v2";

/// fep/0837 prohibits the "to" audience when other naked properties are used.
/// fmatch only inspects string entries in the @context array, so we keep it
/// plain strings for the wire format.
pub fn marketplace_context() -> Vec<Value> {
    vec![
        Value::String(AS_CONTEXT.to_string()),
        Value::String(FEP0837_CONTEXT.to_string()),
    ]
}

pub fn actor_context() -> Vec<Value> {
    vec![
        Value::String(AS_CONTEXT.to_string()),
        Value::String(SEC_CONTEXT.to_string()),
    ]
}
