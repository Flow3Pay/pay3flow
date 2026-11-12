use crate::otc::error::Result;
use deadpool_postgres::GenericClient;

const RECORD_AVATAR: &str = "INSERT INTO otc_avatars(chain,address,public_key,avatar,updated_at) VALUES($1,$2,$3,$4,now()) \
             ON CONFLICT(chain,address) DO UPDATE SET \
             public_key=COALESCE(EXCLUDED.public_key,otc_avatars.public_key), \
             avatar=EXCLUDED.avatar,updated_at=now()";

pub(crate) async fn record_avatar(
    client: &impl GenericClient,
    chain: &str,
    address: &str,
    public_key: &Option<String>,
    avatar: &str,
) -> Result<()> {
    client
        .execute(RECORD_AVATAR, &[&chain, &address, public_key, &avatar])
        .await?;
    Ok(())
}
