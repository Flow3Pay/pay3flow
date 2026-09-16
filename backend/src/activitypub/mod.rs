pub mod actor;
pub mod context;
pub mod delivery;
pub mod error;
pub mod inbox;
pub mod model;
pub mod service;
pub mod signature;

pub use model::AcquirerCandidate;
pub use service::Service;

pub const ACTIVITY_JSON: &str = "application/activity+json";
pub const JSON: &str = "application/ld+json; profile=\"https://www.w3.org/ns/activitystreams\"";
