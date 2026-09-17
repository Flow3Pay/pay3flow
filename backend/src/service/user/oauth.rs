use anyhow::{anyhow, Result};

pub fn authorize(_provider: &str) -> Result<String> {
    Err(anyhow!("oauth providers are not set up yet"))
}
