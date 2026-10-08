pub mod activitypub;
pub mod banks;
pub mod config;
pub mod compiled_provider_code {
    include!(concat!(env!("OUT_DIR"), "/provider_code.rs"));
}
pub mod compiled_review_code {
    include!(concat!(env!("OUT_DIR"), "/provider_reviews.rs"));
}
pub mod core;
pub mod db;
pub mod exchange;
pub mod external_reviews;
pub mod market_prices;
pub mod networks;
pub mod observability;
pub mod p2p;
pub mod pairs;
pub mod payments;
pub mod provider_adapter;
pub mod providers;
pub mod quotes;
pub mod referrals;
pub mod route_engine;
pub mod route_execution;
pub mod routing;
pub mod server;
pub mod service;
pub mod service_reputation;
