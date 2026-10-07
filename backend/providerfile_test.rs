use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::providerfile_code::CodeSource;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestBlock {
    pub source_url: Option<String>,
    pub language: Option<String>,
    pub source: Option<CodeSource>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    pub expected_status: Option<u16>,
    pub body_contains: Option<String>,
    #[serde(default)]
    pub json_equals: BTreeMap<String, serde_json::Value>,
}

fn default_timeout_ms() -> u64 {
    10_000
}

impl TestBlock {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_url.is_none() && self.source.is_none() {
            return Err("[test] requires source_url or source".into());
        }
        if !(250..=30_000).contains(&self.timeout_ms) {
            return Err("[test].timeout_ms must be between 250 and 30000".into());
        }
        if let Some(url) = &self.source_url {
            let url = reqwest::Url::parse(url)
                .map_err(|error| format!("[test].source_url is invalid: {error}"))?;
            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
                return Err("[test].source_url must use http or https with a host".into());
            }
        } else if self.expected_status.is_some()
            || self.body_contains.is_some()
            || !self.json_equals.is_empty()
        {
            return Err("[test] HTTP assertions require source_url".into());
        }
        if self
            .expected_status
            .is_some_and(|status| !(100..=599).contains(&status))
        {
            return Err("[test].expected_status must be between 100 and 599".into());
        }
        for pointer in self.json_equals.keys() {
            if !valid_json_pointer(pointer) {
                return Err(format!(
                    "[test].json_equals has invalid JSON pointer {pointer:?}"
                ));
            }
        }
        if let Some(source) = &self.source {
            if self
                .language
                .as_deref()
                .is_some_and(|language| language != "rust")
            {
                return Err("[test].language must be \"rust\"".into());
            }
            source.validate_for("test")?;
        } else if self.language.is_some() {
            return Err("[test].language requires source".into());
        }
        Ok(())
    }

    pub fn run(&self, providerfile: &Path) -> Result<(), String> {
        self.validate()?;
        if let Some(url) = &self.source_url {
            self.run_http(url)?;
        }
        if let Some(source) = &self.source {
            self.run_rust(source, providerfile)?;
        }
        Ok(())
    }

    fn run_http(&self, url: &str) -> Result<(), String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(self.timeout_ms))
            .build()
            .map_err(|error| format!("[test]: cannot create HTTP client: {error}"))?;
        let response = client
            .get(url)
            .send()
            .map_err(|error| format!("[test]: GET {url} failed: {error}"))?;
        let status = response.status().as_u16();
        let expected = self.expected_status.unwrap_or(200);
        if status != expected {
            return Err(format!(
                "[test]: GET {url}: expected status {expected}, got {status}"
            ));
        }
        if self.body_contains.is_none() && self.json_equals.is_empty() {
            return Ok(());
        }
        let body = response
            .text()
            .map_err(|error| format!("[test]: cannot read GET {url} response: {error}"))?;
        if let Some(expected) = &self.body_contains {
            if !body.contains(expected) {
                return Err(format!(
                    "[test]: GET {url}: response does not contain {expected:?}"
                ));
            }
        }
        if !self.json_equals.is_empty() {
            let json: serde_json::Value = serde_json::from_str(&body)
                .map_err(|error| format!("[test]: GET {url}: invalid JSON: {error}"))?;
            for (pointer, expected) in &self.json_equals {
                let actual = json.pointer(pointer).ok_or_else(|| {
                    format!("[test]: GET {url}: missing JSON pointer {pointer:?}")
                })?;
                if actual != expected {
                    return Err(format!(
                        "[test]: GET {url}: {pointer:?}: expected {expected}, got {actual}"
                    ));
                }
            }
        }
        Ok(())
    }

    fn run_rust(&self, source: &CodeSource, providerfile: &Path) -> Result<(), String> {
        let directory = tempfile::tempdir()
            .map_err(|error| format!("[test]: cannot create temporary directory: {error}"))?;
        let providerfile = fs::canonicalize(providerfile)
            .map_err(|error| format!("[test]: cannot resolve Providerfile: {error}"))?;
        let parent = providerfile
            .parent()
            .ok_or_else(|| "[test]: Providerfile has no parent directory".to_string())?;
        let source_path = match source.external_path(&providerfile)? {
            Some(path) => {
                source.load(&providerfile)?;
                path
            }
            None => {
                let path = directory.path().join("tests.rs");
                fs::write(&path, source.load(&providerfile)?)
                    .map_err(|error| format!("[test]: cannot write Rust tests: {error}"))?;
                path
            }
        };
        let executable = directory
            .path()
            .join(format!("provider_tests{}", env::consts::EXE_SUFFIX));
        let compiler = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = Command::new(compiler)
            .args(["--test", "--edition=2021", "--crate-name", "provider_tests"])
            .arg(&source_path)
            .arg("-o")
            .arg(&executable)
            .env("PROVIDERFILE", &providerfile)
            .current_dir(parent)
            .output()
            .map_err(|error| format!("[test]: cannot start rustc: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "[test]: Rust tests did not compile:\n{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let listing = Command::new(&executable)
            .arg("--list")
            .output()
            .map_err(|error| format!("[test]: cannot list Rust tests: {error}"))?;
        if !listing.status.success()
            || !String::from_utf8_lossy(&listing.stdout)
                .lines()
                .any(|line| line.ends_with(": test"))
        {
            return Err("[test].source must define at least one #[test] function".into());
        }
        let stdout_path = directory.path().join("stdout");
        let stderr_path = directory.path().join("stderr");
        let stdout = File::create(&stdout_path).map_err(|error| error.to_string())?;
        let stderr = File::create(&stderr_path).map_err(|error| error.to_string())?;
        let mut child = Command::new(&executable)
            .arg("--include-ignored")
            .env("PROVIDERFILE", &providerfile)
            .current_dir(parent)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| format!("[test]: cannot start Rust tests: {error}"))?;
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(_)) => {
                    let stdout = fs::read_to_string(&stdout_path).unwrap_or_default();
                    let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
                    return Err(format!("[test]: Rust tests failed:\n{stdout}{stderr}"));
                }
                Ok(None) if started.elapsed() < Duration::from_millis(self.timeout_ms) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                result => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(match result {
                        Err(error) => format!("[test]: cannot wait for Rust tests: {error}"),
                        _ => format!(
                            "[test]: Rust tests exceeded timeout_ms ({})",
                            self.timeout_ms
                        ),
                    });
                }
            }
        }
    }
}

fn valid_json_pointer(pointer: &str) -> bool {
    if !pointer.is_empty() && !pointer.starts_with('/') {
        return false;
    }
    let mut chars = pointer.chars();
    while let Some(ch) = chars.next() {
        if ch == '~' && !matches!(chars.next(), Some('0' | '1')) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    fn block(config: &str) -> TestBlock {
        toml::from_str(config).unwrap()
    }

    fn serve(status: u16, body: &'static str, delay: Duration) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/health", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4096];
            let len = stream.read(&mut request).unwrap();
            assert!(request[..len].starts_with(b"GET /health HTTP/1.1\r\n"));
            thread::sleep(delay);
            let response = format!(
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        (url, server)
    }

    #[test]
    fn validates_optional_modes_and_rejects_misconfigured_tests() {
        for config in [
            "source_url = 'https://example.com'",
            "source = '#[test] fn works() {}'",
            "language = 'rust'\nsource = { path = 'tests.rs' }",
        ] {
            assert!(block(config).validate().is_ok(), "{config}");
        }
        for config in [
            "",
            "source_url = 'file:///tmp/response'",
            "source_url = 'http://'",
            "source_url = 'https://example.com'\ntimeout_ms = 0",
            "source_url = 'https://example.com'\nexpected_status = 600",
            "source_url = 'https://example.com'\njson_equals = { status = 'ok' }",
            "source_url = 'https://example.com'\njson_equals = { '/a~2b' = 'ok' }",
            "language = 'rust'",
            "language = 'python'\nsource = 'x'",
            "source = ''",
            "source = { path = '../tests.rs' }",
            "source = { path = 'tests.txt' }",
            "source = 'x'\nbody_contains = 'ok'",
        ] {
            assert!(block(config).validate().is_err(), "{config}");
        }
        assert!(toml::from_str::<TestBlock>("souerсe_url = 'https://example.com'").is_err());
    }

    #[test]
    fn checks_http_status_body_and_json_values() {
        let (url, server) = serve(
            200,
            r#"{"status":"ok","count":3,"a/b":true}"#,
            Duration::ZERO,
        );
        let test = block(&format!(
            "source_url = '{url}'\nbody_contains = 'ok'\njson_equals = {{ '/status' = 'ok', '/count' = 3, '/a~1b' = true }}"
        ));
        let result = test.run_http(&url);
        server.join().unwrap();
        result.unwrap();
    }

    #[test]
    fn rejects_unexpected_http_responses() {
        for (status, body, assertion, message) in [
            (503, "unavailable", "", "expected status 200, got 503"),
            (200, "no", "body_contains = 'ok'", "does not contain"),
            (
                200,
                "invalid",
                "json_equals = { '/status' = 'ok' }",
                "invalid JSON",
            ),
            (
                200,
                "{}",
                "json_equals = { '/status' = 'ok' }",
                "missing JSON pointer",
            ),
            (
                200,
                r#"{"status":"error"}"#,
                "json_equals = { '/status' = 'ok' }",
                "expected",
            ),
        ] {
            let (url, server) = serve(status, body, Duration::ZERO);
            let test = block(&format!("source_url = '{url}'\n{assertion}"));
            let result = test.run_http(&url);
            server.join().unwrap();
            assert!(result.unwrap_err().contains(message), "{assertion}");
        }
    }

    #[test]
    fn accepts_a_configured_http_status() {
        let (url, server) = serve(204, "", Duration::ZERO);
        let result = block(&format!("source_url = '{url}'\nexpected_status = 204")).run_http(&url);
        server.join().unwrap();
        result.unwrap();
    }

    #[test]
    fn stops_waiting_for_slow_http_responses() {
        let (url, server) = serve(200, "ok", Duration::from_millis(500));
        let result = block(&format!("source_url = '{url}'\ntimeout_ms = 250")).run_http(&url);
        server.join().unwrap();
        assert!(result.unwrap_err().contains("failed"));
    }

    fn rust_test(source: &str, external: bool, timeout_ms: u64) -> Result<(), String> {
        let directory = tempfile::tempdir().unwrap();
        let providerfile = directory.path().join("Providerfile");
        fs::write(&providerfile, "fixture").unwrap();
        fs::write(
            directory.path().join("helpers.rs"),
            "pub fn answer() -> u32 { 42 }",
        )
        .unwrap();
        let config = if external {
            fs::write(directory.path().join("tests.rs"), source).unwrap();
            "source = { path = 'tests.rs' }".to_string()
        } else {
            format!("source = '''{source}'''")
        };
        block(&format!("{config}\ntimeout_ms = {timeout_ms}")).run(&providerfile)
    }

    #[test]
    fn runs_inline_rust_with_providerfile_context() {
        rust_test(
            r#"#[test] fn reads_providerfile() {
                assert_eq!(std::fs::read_to_string(env!("PROVIDERFILE")).unwrap(), "fixture");
                assert_eq!(std::fs::read_to_string("Providerfile").unwrap(), "fixture");
            }"#,
            false,
            10_000,
        )
        .unwrap();
    }

    #[test]
    fn runs_external_rust_with_sibling_modules() {
        rust_test(
            r#"#[path = "helpers.rs"] mod helpers;
            #[test] fn checks_helper() { assert_eq!(helpers::answer(), 42); }"#,
            true,
            10_000,
        )
        .unwrap();
    }

    #[test]
    fn reports_rust_assertions_compilation_failures_and_empty_suites() {
        for (source, message) in [
            (
                "#[test] fn fails() { assert_eq!(1, 2); }",
                "Rust tests failed",
            ),
            (
                "#[test] #[ignore] fn fails() { panic!(\"failure\"); }",
                "Rust tests failed",
            ),
            ("invalid Rust code", "did not compile"),
            ("pub fn no_tests() {}", "at least one #[test]"),
        ] {
            assert!(
                rust_test(source, false, 10_000)
                    .unwrap_err()
                    .contains(message),
                "{source}"
            );
        }
    }

    #[test]
    fn stops_rust_tests_that_exceed_the_timeout() {
        let result = rust_test(
            "#[test] fn slow() { std::thread::sleep(std::time::Duration::from_secs(10)); }",
            false,
            250,
        );
        assert!(result.unwrap_err().contains("exceeded timeout_ms"));
    }
}
