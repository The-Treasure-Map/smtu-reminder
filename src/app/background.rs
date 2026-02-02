use std::sync::Arc;

use teloxide::prelude::Bot;

use crate::{
    state::AppState,
    tasks::{hourly_notifier_loop, schedule_fetch_loop},
};

pub fn spawn(bot: Bot, state: Arc<AppState>) {
    tokio::spawn(schedule_fetch_loop(Arc::clone(&state)));
    tokio::spawn(hourly_notifier_loop(bot, state));
}
