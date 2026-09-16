use thiserror::Error;

#[derive(Debug, Error)]
pub enum ActivityPubError {
    #[error("activitypub: {0}")]
    Other(String),
    #[error("activitypub: signature error: {0}")]
    Signature(String),
    #[error("activitypub: key error: {0}")]
    Key(String),
    #[error("activitypub: delivery failed after retries: {0}")]
    Delivery(String),
    #[error("activitypub: invalid activity {type_name}: {detail}")]
    InvalidActivity { type_name: String, detail: String },
    #[error("activitypub: http error: {0}")]
    Http(String),
}
