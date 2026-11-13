use anyhow::Result;

use crate::db::DbPoolObject;

const UPSERT: &str = "INSERT INTO share_links (id, target, preview) VALUES ($1, $2, $3) \
     ON CONFLICT (id) DO UPDATE SET id = EXCLUDED.id WHERE share_links.target = EXCLUDED.target RETURNING id, target, preview";
const GET: &str = "SELECT id, target, preview FROM share_links WHERE id = $1";

pub struct ShareLinkRow {
    pub id: String,
    pub target: String,
    pub preview: Option<serde_json::Value>,
}

fn to_row(row: tokio_postgres::Row) -> ShareLinkRow {
    ShareLinkRow {
        id: row.get(0),
        target: row.get(1),
        preview: row.get(2),
    }
}

pub async fn upsert(
    client: &DbPoolObject,
    id: &str,
    target: &str,
    preview: Option<&serde_json::Value>,
) -> Result<ShareLinkRow> {
    let row = client.query_one(UPSERT, &[&id, &target, &preview]).await?;
    Ok(to_row(row))
}

pub async fn get(client: &DbPoolObject, id: &str) -> Result<Option<ShareLinkRow>> {
    let row = client.query_opt(GET, &[&id]).await?;
    Ok(row.map(to_row))
}
