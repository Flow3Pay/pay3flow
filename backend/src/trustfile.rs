//! Runtime registry for the Trustfiles validated by `build.rs`.
//!
//! The registry is compiled into the binary from `providers/**/Trustfile`, so
//! the running service never reads TOML from disk or trusts client input when
//! deciding whether a provider may execute a swap.

use std::collections::BTreeMap;
use std::sync::OnceLock;

pub use crate::trustfile_model::Trustfile;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/trustfiles.rs"));
}

fn registry() -> &'static BTreeMap<&'static str, Trustfile> {
    static REGISTRY: OnceLock<BTreeMap<&'static str, Trustfile>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        generated::TRUSTFILE_SOURCES
            .iter()
            .map(|(provider, source)| {
                let trust: Trustfile = toml::from_str(source).unwrap_or_else(|error| {
                    panic!("embedded Trustfile for `{provider}` is invalid: {error}")
                });
                assert_eq!(
                    &trust.provider, provider,
                    "embedded Trustfile provider does not match its slug"
                );
                (*provider, trust)
            })
            .collect()
    })
}

/// Find the embedded Trustfile for a provider slug.
pub fn get(provider: &str) -> Option<&'static Trustfile> {
    registry().get(provider)
}

/// Whether the provider has a complete, enabled quote → build → wallet submit
/// → status path. Runtime configuration still has to be present as well; this
/// only answers what the checked-in Trustfile allows.
pub fn execution_enabled(provider: &str) -> bool {
    get(provider).is_some_and(Trustfile::executable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trustfile_model::TrustState;

    #[test]
    fn every_embedded_trustfile_matches_its_slug_and_validates() {
        assert!(!generated::TRUSTFILE_SOURCES.is_empty());
        for (provider, source) in generated::TRUSTFILE_SOURCES {
            let trust: Trustfile = toml::from_str(source).expect("embedded Trustfile parses");
            assert_eq!(&trust.provider, provider);
            trust
                .validate(provider)
                .unwrap_or_else(|error| panic!("{provider}: {error}"));
            assert_eq!(get(provider), Some(&trust));
        }
    }

    #[test]
    fn providers_with_a_complete_path_are_executable() {
        for provider in ["symbiosis", "near-intents", "cow-swap"] {
            let trust = get(provider).unwrap_or_else(|| panic!("missing {provider} Trustfile"));
            assert_eq!(trust.trust.state, TrustState::Enabled, "{provider}");
            assert!(execution_enabled(provider), "{provider}");
        }
    }

    #[test]
    fn quote_only_providers_and_unknown_slugs_cannot_execute() {
        assert!(!execution_enabled("0x"));
        assert!(!execution_enabled("barter"));
        assert!(!execution_enabled("not-a-provider"));
        assert!(get("not-a-provider").is_none());
    }
}
