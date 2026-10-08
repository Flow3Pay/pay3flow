use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[path = "providerfile_code.rs"]
mod providerfile_code;

use providerfile_code::{normalize_path_source, CodeSource};

#[derive(Deserialize)]
struct Providerfile {
    code: Option<CodeBlock>,
    review_code: Option<ReviewCodeBlock>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeBlock {
    language: String,
    source: CodeSource,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewCodeBlock {
    language: String,
    source: CodeSource,
    profile_host: Option<String>,
    #[serde(default)]
    profile_hosts: Vec<String>,
}

fn main() {
    let providers = PathBuf::from("providers");
    println!("cargo:rerun-if-changed={}", providers.display());
    let mut files = Vec::new();
    collect(&providers, &mut files);
    files.sort();

    let mut generated =
        String::from("// Generated from providers/**/Providerfile [code] blocks.\n");
    let mut review_modules = String::from("// Generated from Providerfile [review_code] blocks.\n");
    let mut provider_arms = String::new();
    let mut profile_arms = String::new();
    let mut profile_fetch_arms = String::new();
    for path in files {
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let normalized = normalize_path_source(&contents)
            .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()));
        let provider: Providerfile = toml::from_str(&normalized)
            .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()));
        let relative = path
            .strip_prefix(&providers)
            .expect("Providerfile must be below providers/");
        let slug = relative
            .parent()
            .expect("Providerfile must have a parent directory")
            .components()
            .map(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .expect("Providerfile path must be UTF-8")
            })
            .collect::<Vec<_>>()
            .join("-");
        let module = slug.replace('-', "_");
        if let Some(code) = provider.code {
            assert_eq!(
                code.language,
                "rust",
                "{}: [code].language must be `rust`",
                path.display()
            );
            let source = code
                .source
                .load(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            if let Some(source_path) = code
                .source
                .external_path(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
            {
                println!("cargo:rerun-if-changed={}", source_path.display());
            }
            writeln!(generated, "pub mod {module} {{").unwrap();
            generated.push_str(&source);
            generated.push_str("\n}\n");
        }
        if let Some(code) = provider.review_code {
            assert_eq!(
                code.language,
                "rust",
                "{}: [review_code].language must be `rust`",
                path.display()
            );
            let source = code
                .source
                .load(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            if let Some(source_path) = code
                .source
                .external_path(&path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
            {
                println!("cargo:rerun-if-changed={}", source_path.display());
            }
            writeln!(review_modules, "pub mod {module} {{").unwrap();
            review_modules.push_str(&source);
            review_modules.push_str("\n}\n");
            writeln!(
                provider_arms,
                "\"{slug}\" => Some({module}::fetch_source(http, source_url, key).await),"
            )
            .unwrap();
            let hosts = code.profile_host.into_iter().chain(code.profile_hosts);
            let mut has_profile_host = false;
            for host in hosts {
                has_profile_host = true;
                writeln!(profile_arms, "\"{host}\" => Some({module}::profile_identity(url).map(|identity| (\"{slug}\", identity))),").unwrap();
            }
            if has_profile_host {
                writeln!(
                    profile_fetch_arms,
                    "\"{slug}\" => {module}::fetch_profile(http, url, identity).await,"
                )
                .unwrap();
            }
        }
    }

    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set")).join("provider_code.rs");
    fs::write(&output, generated)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", output.display()));
    review_modules.push_str("\npub async fn fetch_source(slug: &str, http: &reqwest::Client, source_url: &str, key: Option<&str>) -> Option<anyhow::Result<Vec<crate::external_reviews::ExternalReview>>> { match slug {\n");
    review_modules.push_str(&provider_arms);
    review_modules.push_str("_ => None, } }\n");
    review_modules.push_str("\npub fn profile_identity(url: &reqwest::Url) -> Option<anyhow::Result<(&'static str, (String, String))>> { match url.host_str()? {\n");
    review_modules.push_str(&profile_arms);
    review_modules.push_str("_ => None, } }\n");
    review_modules.push_str("\npub async fn fetch_profile(slug: &str, http: &reqwest::Client, url: &str, identity: &str) -> anyhow::Result<Vec<crate::external_reviews::ExternalReview>> { match slug {\n");
    review_modules.push_str(&profile_fetch_arms);
    review_modules.push_str("_ => anyhow::bail!(\"unsupported review provider\"), } }\n");
    let review_output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set")).join("provider_reviews.rs");
    fs::write(&review_output, review_modules)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", review_output.display()));
}

fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry.expect("provider directory entry is readable").path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.file_name().is_some_and(|name| name == "Providerfile") {
            files.push(path);
        }
    }
}
