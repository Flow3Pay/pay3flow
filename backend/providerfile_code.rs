use std::borrow::Cow;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CodeSource {
    Inline(String),
    External(CodePath),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodePath {
    path: String,
}

impl CodeSource {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_for("code")
    }

    pub fn validate_for(&self, section: &str) -> Result<(), String> {
        match self {
            Self::Inline(source) if source.trim().is_empty() => {
                Err(format!("[{section}].source must not be empty"))
            }
            Self::Inline(_) => Ok(()),
            Self::External(source) => validate_relative_path(&source.path, section),
        }
    }

    pub fn external_path(&self, providerfile: &Path) -> Result<Option<PathBuf>, String> {
        self.validate()?;
        let Self::External(source) = self else {
            return Ok(None);
        };
        let directory = providerfile
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", providerfile.display()))?;
        Ok(Some(directory.join(&source.path)))
    }

    pub fn load(&self, providerfile: &Path) -> Result<String, String> {
        self.validate()?;
        match self {
            Self::Inline(source) => Ok(source.clone()),
            Self::External(_) => {
                let path = self
                    .external_path(providerfile)?
                    .expect("external source always has a path");
                let source = fs::read_to_string(&path)
                    .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
                if source.trim().is_empty() {
                    return Err(format!("{}: Rust source must not be empty", path.display()));
                }
                Ok(source)
            }
        }
    }
}

fn validate_relative_path(path: &str, section: &str) -> Result<(), String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() {
        return Err(format!("[{section}].source path must not be empty"));
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return Err(format!("[{section}].source path must point to a .rs file"));
    }
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "[{section}].source path must stay inside the Providerfile directory"
        ));
    }
    Ok(())
}

/// Converts `[code]` / `[test]` shorthand `source = path["file.rs"]` into an
/// equivalent TOML inline table before deserialization.
pub fn normalize_path_source(contents: &str) -> Result<Cow<'_, str>, String> {
    let mut section = None;
    let mut rewritten = None::<String>;
    let mut copied_until = 0;
    let mut offset = 0;
    let mut string_state = None;

    for line in contents.split_inclusive('\n') {
        let without_newline = line.strip_suffix('\n').unwrap_or(line);
        let body = without_newline
            .strip_suffix('\r')
            .unwrap_or(without_newline);
        let trimmed = body.trim();
        let inside_string = string_state.is_some();
        scan_strings(body, &mut string_state);

        if inside_string {
            offset += line.len();
            continue;
        }
        if trimmed.starts_with('[') {
            section = match trimmed.split('#').next().unwrap_or_default().trim() {
                "[code]" => Some("code"),
                "[test]" => Some("test"),
                "[review_code]" => Some("review_code"),
                _ => None,
            };
        } else if let Some(section) = section {
            let Some(equals) = body.find('=') else {
                offset += line.len();
                continue;
            };
            if body[..equals].trim() != "source" {
                offset += line.len();
                continue;
            }

            let value_start = equals
                + 1
                + body[equals + 1..]
                    .len()
                    .saturating_sub(body[equals + 1..].trim_start().len());
            let value = &body[value_start..];
            let Some(after_path) = value.strip_prefix("path") else {
                offset += line.len();
                continue;
            };
            let after_path = after_path.trim_start();
            let Some(bracketed) = after_path.strip_prefix('[') else {
                offset += line.len();
                continue;
            };
            let Some(closing) = bracketed.rfind(']') else {
                return Err(format!("[{section}].source path is missing a closing `]`"));
            };
            let path_literal = bracketed[..closing].trim();
            if path_literal.is_empty() {
                return Err(format!("[{section}].source path must not be empty"));
            }
            let suffix = &bracketed[closing + 1..];
            if !suffix.trim().is_empty() && !suffix.trim_start().starts_with('#') {
                return Err(format!(
                    "only a comment may follow [{section}].source path shorthand"
                ));
            }

            let output =
                rewritten.get_or_insert_with(|| String::with_capacity(contents.len() + 16));
            output.push_str(&contents[copied_until..offset + value_start]);
            output.push_str("{ path = ");
            output.push_str(path_literal);
            output.push_str(" }");
            output.push_str(suffix);
            let line_end = offset + line.len();
            if line.ends_with('\n') {
                output.push('\n');
            }
            copied_until = line_end;
        }

        offset += line.len();
    }

    match rewritten {
        Some(mut output) => {
            output.push_str(&contents[copied_until..]);
            Ok(Cow::Owned(output))
        }
        None => Ok(Cow::Borrowed(contents)),
    }
}

// Track TOML strings so path-like lines inside inline Rust are left untouched.
fn scan_strings(line: &str, state: &mut Option<(u8, bool)>) {
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match *state {
            Some((b'"', _)) if bytes[index] == b'\\' => index += 2,
            Some((quote, multiline)) if bytes[index] == quote => {
                let count = bytes[index..]
                    .iter()
                    .take_while(|&&byte| byte == quote)
                    .count();
                if !multiline || count >= 3 {
                    *state = None;
                    index += if multiline { count } else { 1 };
                } else {
                    index += count;
                }
            }
            Some(_) => index += 1,
            None if bytes[index] == b'#' => break,
            None if matches!(bytes[index], b'"' | b'\'') => {
                let quote = bytes[index];
                let multiline = bytes[index..].starts_with(&[quote; 3]);
                *state = Some((quote, multiline));
                index += if multiline { 3 } else { 1 };
            }
            None => index += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_path_shorthand_only_in_code_section() {
        let input =
            "value = path[\"untouched.rs\"]\n[code]\nsource = path[\"adapter.rs\"] # code\n";
        let normalized = normalize_path_source(input).unwrap();

        assert_eq!(
            normalized,
            "value = path[\"untouched.rs\"]\n[code]\nsource = { path = \"adapter.rs\" } # code\n"
        );
    }

    #[test]
    fn rejects_paths_outside_the_provider_directory() {
        let source = CodeSource::External(CodePath {
            path: "../shared.rs".to_string(),
        });

        assert!(source.validate().is_err());
    }

    #[test]
    fn rewrites_test_paths_and_preserves_inline_rust() {
        let input = "[code]\nsource = '''\n[test]\nsource = path[\"untouched.rs\"]\n'''\n[test] # tests\nsource = path[\"tests.rs\"]\n";
        assert_eq!(
            normalize_path_source(input).unwrap(),
            input.replace(
                "source = path[\"tests.rs\"]",
                "source = { path = \"tests.rs\" }"
            )
        );
    }
}
