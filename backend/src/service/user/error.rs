#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("invalid auth code")]
    InvalidCode,
    #[error("invalid email")]
    InvalidEmail,
    #[error("user not found")]
    NotFound,
    #[error("email already registered")]
    AlreadyExists,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
