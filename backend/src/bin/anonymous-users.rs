use std::env;

use anyhow::{Context, Result};
use tokio_postgres::NoTls;

const DEFAULT_DATABASE_URL: &str = "postgres://pay3flow:pay3flow@localhost:5435/pay3flow";

#[tokio::main]
async fn main() -> Result<()> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.into());
    let (client, connection) = tokio_postgres::connect(&database_url, NoTls)
        .await
        .with_context(|| format!("cannot connect to PostgreSQL at {database_url}"))?;

    tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("PostgreSQL connection error: {error}");
        }
    });

    let summary = client
        .query_one(
            r#"
SELECT
    COUNT(*)::BIGINT,
    COALESCE(SUM(request_count), 0)::BIGINT,
    COALESCE(MIN(first_seen_at)::TEXT, '[none]'),
    COALESCE(MAX(last_seen_at)::TEXT, '[none]')
FROM anonymous_users
"#,
            &[],
        )
        .await
        .context("cannot read anonymous_users")?;

    println!("Anonymous usage summary");
    println!("  users:          {}", summary.get::<_, i64>(0));
    println!("  total requests: {}", summary.get::<_, i64>(1));
    println!("  first seen:     {}", summary.get::<_, String>(2));
    println!("  last seen:      {}", summary.get::<_, String>(3));
    println!();
    println!("Requests by anonymous user:");
    println!("{:<38} {:>12}  {:<25}  last seen", "anonymous_id", "requests", "first seen");

    for row in client
        .query(
            r#"
SELECT id, request_count, first_seen_at::TEXT, last_seen_at::TEXT
FROM anonymous_users
ORDER BY request_count DESC, last_seen_at DESC, id
"#,
            &[],
        )
        .await
        .context("cannot read anonymous user details")?
    {
        println!(
            "{:<38} {:>12}  {:<25}  {}",
            row.get::<_, uuid::Uuid>(0),
            row.get::<_, i64>(1),
            row.get::<_, String>(2),
            row.get::<_, String>(3),
        );
    }

    Ok(())
}
