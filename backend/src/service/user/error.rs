#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("invalid auth code")]
    InvalidCode,
    #[error("too many requests")]
    RateLimited { retry_after_secs: i64 },
    #[error("invalid email")]
    InvalidEmail,
    #[error("user not found")]
    NotFound,
    #[error("email already registered")]
    AlreadyExists,
    #[error("invalid referral code")]
    InvalidReferralCode,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<tokio_postgres::Error> for UserError {
    fn from(error: tokio_postgres::Error) -> Self {
        Self::Other(error.into())
    }
}

impl From<deadpool_postgres::PoolError> for UserError {
    fn from(error: deadpool_postgres::PoolError) -> Self {
        Self::Other(error.into())
    }
}
