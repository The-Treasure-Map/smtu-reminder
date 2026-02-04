use std::{sync::Arc, time::Duration};

use chrono::{Duration as ChronoDuration, FixedOffset, Timelike, Utc};
use teloxide::{prelude::*, types::ChatId};
use tracing::{error, info};

use crate::{
    formatting::ScheduleFormatting, parser::schedule, state::AppState, structures::WeekType,
};

pub async fn schedule_fetch_loop(state: Arc<AppState>) {
    let refresh_interval = Duration::from_secs(12 * 60 * 60);
    let retry_interval = Duration::from_secs(5 * 60);
    let mut retries = 0u32;
    loop {
        let now = Utc::now().timestamp();
        let last_updated = {
            let repo = state.schedule_repo.read().await;
            repo.last_updated()
        };

        if let Some(last_updated) = last_updated {
            let age = now.saturating_sub(last_updated) as u64;
            if age < refresh_interval.as_secs() {
                let sleep_for = refresh_interval.as_secs() - age;
                info!("schedule cache is fresh, sleeping for {}s", sleep_for);
                tokio::time::sleep(Duration::from_secs(sleep_for)).await;
                continue;
            }
        }

        info!("schedule fetch loop tick");
        match schedule::fetch_all(&state.client).await {
            Ok(schedules) => {
                retries = 0;
                let now = Utc::now().timestamp();
                {
                    let mut repo = state.schedule_repo.write().await;
                    repo.update(schedules.clone(), now);
                }

                if let Err(err) = state.db.replace_schedule_cache(&schedules, now).await {
                    error!("schedule cache update failed: {err}");
                } else {
                    info!("schedule cache updated, count: {}", schedules.len());
                }
            }
            Err(err) => {
                retries += 1;
                error!("schedule fetch failed: {err}");
            }
        }

        let sleep_duration = if retries == 0 {
            refresh_interval
        } else {
            info!("retry in 5 mins");
            retries * retry_interval
        };

        tokio::time::sleep(sleep_duration).await;
    }
}

pub async fn hourly_notifier_loop(bot: Bot, state: Arc<AppState>) {
    loop {
        let sleep_duration = duration_until_next_hour_msk();
        tokio::time::sleep(sleep_duration).await;

        let hour = current_hour_msk();
        info!("hourly notifier tick, hour: {}", hour);

        let users = match state.db.get_users_for_hour(hour).await {
            Ok(users) => users,
            Err(err) => {
                error!("failed to load users for hour: {err}");
                continue;
            }
        };

        let repo = state.schedule_repo.read().await;
        for user in users {
            let Some(group_number) = user.group_number else {
                continue;
            };

            let Some(schedule) = repo.get_by_group_number(group_number) else {
                continue;
            };

            let mut datetime = Utc::now().naive_local();
            if datetime.hour() > 11 {
                datetime += chrono::Duration::days(1);
            }

            if let Err(err) = bot
                .send_message(
                    ChatId(user.telegram_id),
                    schedule.format_for_day(datetime.date(), WeekType::for_date(datetime.date())),
                )
                .await
            {
                error!("failed to send schedule remind. {err}");
            }
        }
    }
}

fn current_hour_msk() -> i32 {
    let msk = FixedOffset::east_opt(3 * 60 * 60).unwrap();
    Utc::now().with_timezone(&msk).hour() as i32
}

fn duration_until_next_hour_msk() -> Duration {
    let msk = FixedOffset::east_opt(3 * 60 * 60).unwrap();
    let now = Utc::now().with_timezone(&msk);
    let next_hour = (now + ChronoDuration::hours(1))
        .with_minute(0)
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap();
    (next_hour - now)
        .to_std()
        .unwrap_or(Duration::from_secs(60 * 60))
}
