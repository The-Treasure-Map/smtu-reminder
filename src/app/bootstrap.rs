use std::sync::Arc;

use anyhow::Context;
use reqwest::retry;
use tracing::{debug, warn};

use crate::{db::Db, parser::urls::HOST, schedule_repo::ScheduleRepo, state::AppState};

pub async fn build_http_client() -> anyhow::Result<reqwest::Client> {
    let policy = retry::for_host(HOST).max_retries_per_request(u32::MAX);
    reqwest::Client::builder()
        .cookie_store(true)
        .retry(policy)
        .build()
        .context("failed to build reqwest client")
}

pub async fn build_db() -> anyhow::Result<Db> {
    let database_url = std::env::var("DATABASE_URL").context("failed te get DATABASE_URL env")?;
    Db::new(&database_url).await.context("failed to init db")
}

pub async fn restore_schedule_cache(db: &Db, schedule_repo: &mut ScheduleRepo) {
    let schedules = match db.load_schedule_cache().await {
        Ok(schedules) => schedules,
        Err(err) => {
            warn!(?err, "failed to load schedule cache");
            return;
        }
    };

    let last_updated = match db.load_schedule_last_updated().await {
        Ok(last_updated) => last_updated,
        Err(err) => {
            warn!(?err, "failed to load schedule last updated");
            return;
        }
    };

    match last_updated {
        Some(last_updated) => {
            if !schedules.is_empty() {
                schedule_repo.update(schedules, last_updated);
            }
        }
        None => {
            debug!("schedule last_updated is empty");
        }
    }
}

pub fn build_state(db: Db, client: reqwest::Client, schedule_repo: ScheduleRepo) -> Arc<AppState> {
    Arc::new(AppState {
        db,
        client,
        schedule_repo: Arc::new(tokio::sync::RwLock::new(schedule_repo)),
    })
}
