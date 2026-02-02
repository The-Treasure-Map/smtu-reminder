mod background;
mod bootstrap;

use std::sync::Arc;

use teloxide::prelude::Bot;

use crate::{bot, schedule_repo::ScheduleRepo};

pub async fn run() -> anyhow::Result<()> {
    let client = bootstrap::build_http_client().await?;
    let db = bootstrap::build_db().await?;

    let mut schedule_repo = ScheduleRepo::new();
    bootstrap::restore_schedule_cache(&db, &mut schedule_repo).await;
    let state = bootstrap::build_state(db, client, schedule_repo);

    let bot = Bot::from_env();
    background::spawn(bot.clone(), Arc::clone(&state));
    bot::start(bot, state).await;

    Ok(())
}
