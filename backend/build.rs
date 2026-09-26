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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeBlock {
    language: String,
    source: CodeSource,
}

fn main() {
    let providers = PathBuf::from("providers");
    println!("cargo:rerun-if-changed={}", providers.display());
    let mut files = Vec::new();
    collect(&providers, &mut files);
    files.sort();

    let mut generated =
        String::from("// Generated from providers/**/Providerfile [code] blocks.\n");
    for path in files {
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let normalized = normalize_path_source(&contents)
            .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()));
        let provider: Providerfile = toml::from_str(&normalized)
            .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()));
        let Some(code) = provider.code else { continue };
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
        writeln!(generated, "pub mod {module} {{").unwrap();
        generated.push_str(&source);
        generated.push_str("\n}\n");
    }

    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set")).join("provider_code.rs");
    fs::write(&output, generated)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", output.display()));
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
