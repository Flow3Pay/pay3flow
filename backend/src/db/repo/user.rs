use anyhow::Result;
use uuid::Uuid;

use crate::db::DbPool;

pub struct User {
    pub id: Uuid,
    pub email: String,
    pub referral_code: String,
}

const GET_ID_BY_EMAIL: &str = "SELECT id FROM users WHERE email = $1";
const GET_BY_ID: &str = "SELECT id, email, referral_code FROM users WHERE id = $1";

pub enum InsertUserOutcome {
    Created(Uuid),
    EmailExists,
    ReferralCodeNotFound,
}

pub async fn insert_user(
    pool: &DbPool,
    email: &str,
    referred_by_code: Option<&str>,
) -> Result<InsertUserOutcome> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let referred_by_user_id = if let Some(code) = referred_by_code {
        let Some(row) = tx
            .query_opt("SELECT id FROM users WHERE referral_code = $1", &[&code])
            .await?
        else {
            return Ok(InsertUserOutcome::ReferralCodeNotFound);
        };
        Some(row.get::<_, Uuid>(0))
    } else {
        None
    };

    let id = Uuid::new_v4();
    let referral_code = id
        .simple()
        .to_string()
        .chars()
        .take(16)
        .collect::<String>()
        .to_ascii_uppercase();
    let row = tx
        .query_opt(
            r#"
INSERT INTO users (id, email, referral_code, referred_by_user_id, referred_at)
VALUES ($1, $2, $3, $4, CASE WHEN $4::UUID IS NULL THEN NULL ELSE now() END)
ON CONFLICT (email) DO NOTHING
RETURNING id
"#,
            &[&id, &email, &referral_code, &referred_by_user_id],
        )
        .await?;
    let Some(row) = row else {
        return Ok(InsertUserOutcome::EmailExists);
    };
    let id = row.get(0);
    tx.commit().await?;
    Ok(InsertUserOutcome::Created(id))
}

pub async fn user_id_by_email(pool: &DbPool, email: &str) -> Result<Option<Uuid>> {
    let client = pool.get().await?;
    let stmt = client.prepare_cached(GET_ID_BY_EMAIL).await?;
    let row = client.query_opt(&stmt, &[&email]).await?;
    Ok(row.map(|row| row.get(0)))
}

pub async fn user_by_id(pool: &DbPool, id: &Uuid) -> Result<Option<User>> {
    let client = pool.get().await?;
    let stmt = client.prepare_cached(GET_BY_ID).await?;
    let row = client.query_opt(&stmt, &[id]).await?;
    Ok(row.map(|row| User {
        id: row.get(0),
        email: row.get(1),
        referral_code: row.get(2),
    }))
}
