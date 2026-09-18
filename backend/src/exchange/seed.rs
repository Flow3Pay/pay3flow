use anyhow::Result;

use crate::db::DbPool;
use crate::exchange::{repo, solver};

pub async fn seed_fake_solvers(pool: &DbPool) -> Result<usize> {
    let mut seeded = 0;
    for fake_solver in solver::fake_solvers() {
        repo::upsert_solver(pool, &fake_solver.as_new_solver()).await?;
        seeded += 1;
    }
    Ok(seeded)
}
