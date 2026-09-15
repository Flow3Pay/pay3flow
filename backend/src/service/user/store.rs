use anyhow::Result;
use uuid::Uuid;

use crate::core::db::DbPool;

pub struct User {
    pub id: Uuid,
    pub email: String,
}

const INSERT_USER: &str =
    "INSERT INTO users (email) VALUES ($1) ON CONFLICT (email) DO NOTHING RETURNING id";
const GET_ID_BY_EMAIL: &str = "SELECT id FROM users WHERE email = $1";
const GET_BY_ID: &str = "SELECT id, email FROM users WHERE id = $1";

pub async fn insert_user(pool: &DbPool, email: &str) -> Result<Option<Uuid>> {
    let client = pool.get().await?;
    let stmt = client.prepare_cached(INSERT_USER).await?;
    let row = client.query_opt(&stmt, &[&email]).await?;
    Ok(row.map(|row| row.get(0)))
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
    Ok(row.map(|row| User { id: row.get(0), email: row.get(1) }))
}
