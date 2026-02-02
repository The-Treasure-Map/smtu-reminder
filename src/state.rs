use std::sync::Arc;

use crate::{db::Db, schedule_repo::ScheduleRepo};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub client: reqwest::Client,
    pub schedule_repo: Arc<tokio::sync::RwLock<ScheduleRepo>>,
}
