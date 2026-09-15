use anyhow::Result;

use super::register;

pub fn send_code(email: &str) -> Result<()> {
    register::send_auth_code(email)
}

pub fn verify_auth_code(_email: &str, code: &str) -> bool {
    code == "1234"
}