use std::sync::Arc;

use anyhow::Context;
use chrono::{NaiveDate, Utc};
use teloxide::{macros::BotCommands as BotCommandsMacro, prelude::*, utils::command::BotCommands};
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::{
    db::Db,
    formatting::ScheduleFormatting,
    schedule_repo::ScheduleRepo,
    state::AppState,
    structures::{Schedule, WeekType},
};

use super::callbacks::inline_buttons;

const INTERNAL_ERROR_MESSAGE: &str = "Произошла внутренняя ошибка, попробуйте позже.";

#[derive(BotCommandsMacro, Clone, Debug)]
#[command(rename_rule = "lowercase")]
pub enum Command {
    #[command(description = "Показать список команд")]
    Help,
    #[command(description = "Показать моё расписание на сегодня")]
    MySchedule,
    #[command(description = "Выбрать свою группу")]
    SetGroup(String),
    #[command(description = "Найти расписание (номер группы точно или преподаватель вхождение)")]
    Schedule(String),
    #[command(
        description = "Включить напоминание расписания, /remind 21 - будет отсылать напоминание в 9 часов вечера (можно выставить любое время с 16 до 8)"
    )]
    Remind(String),
    #[command(description = "Выключить напоминание расписания")]
    RemindOff,
}

pub async fn command_endpoint(
    bot: Bot,
    msg: Message,
    cmd: Command,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    let Some(user) = msg.from.clone() else {
        return Ok(());
    };

    let telegram_id = user.id.0 as i64;
    let chat_id = msg.chat.id;
    if let Err(err) = state.db.ensure_user(telegram_id).await {
        error!(?err, telegram_id, chat_id = ?chat_id, "failed to ensure user");
    }

    let cmd_for_log = cmd.clone();
    let result: anyhow::Result<()> = match cmd {
        Command::Help => send_help(&bot, chat_id).await,
        Command::MySchedule => {
            handle_my_schedule(
                &bot,
                telegram_id,
                chat_id,
                state.db.clone(),
                state.schedule_repo.clone(),
            )
            .await
        }
        Command::SetGroup(query) => {
            handle_set_group(
                &bot,
                &query,
                telegram_id,
                chat_id,
                state.db.clone(),
                state.schedule_repo.clone(),
            )
            .await
        }
        Command::Schedule(query) => {
            handle_schedule_search(
                &bot,
                telegram_id,
                chat_id,
                &query,
                state.schedule_repo.clone(),
            )
            .await
        }
        Command::Remind(remind_hour) => {
            handle_remind_set(&bot, telegram_id, chat_id, &remind_hour, state.db.clone()).await
        }

        Command::RemindOff => handle_remind_off(&bot, telegram_id, chat_id, state.db.clone()).await,
    };

    if let Err(err) = result {
        error!(
            ?err,
            telegram_id,
            chat_id = ?chat_id,
            cmd = ?cmd_for_log,
            message_text = msg.text(),
            "command handler failed"
        );
        let _ = bot.send_message(chat_id, INTERNAL_ERROR_MESSAGE).await;
    }

    Ok(())
}

async fn handle_remind_set(
    bot: &Bot,
    telegram_id: i64,
    id: ChatId,
    remind_hour: &str,
    db: Db,
) -> anyhow::Result<()> {
    let remind_hour = remind_hour
        .parse::<u32>()
        .ok()
        .filter(|hour| (0..9).contains(hour) || (16..24).contains(hour));

    match remind_hour {
        Some(remind_hour) => {
            db.set_remind_hour(telegram_id, remind_hour).await?;
            bot.send_message(id, "Время для напоминания установлено.")
                .await?;
        }
        None => {
            bot.send_message(
                id,
                "Укажите час для напоминания в промежутке от 16 до 8 часов",
            )
            .await?;
        }
    }

    Ok(())
}

async fn handle_remind_off(bot: &Bot, telegram_id: i64, id: ChatId, db: Db) -> anyhow::Result<()> {
    db.remove_remind_hour(telegram_id).await?;
    bot.send_message(id, "Напоминания отключены.").await?;
    Ok(())
}

async fn handle_my_schedule(
    bot: &Bot,
    telegram_id: i64,
    id: ChatId,
    db: Db,
    schedule_repo: Arc<RwLock<ScheduleRepo>>,
) -> anyhow::Result<()> {
    let group = db
        .get_group(telegram_id)
        .await
        .context("failed to get user group")?;

    match group {
        Some(group) => {
            let repo = schedule_repo.read().await;
            let schedule = repo.get_by_group_number(group);

            match schedule {
                Some(schedule) => {
                    send_schedule(bot, id, schedule).await?;
                }
                None => {
                    bot.send_message(
                        id,
                        format!("Расписание для группы \"{}\" не найдено", group),
                    )
                    .await?;
                }
            }
        }
        None => {
            bot.send_message(id, "Сначала задайте группу").await?;

            send_help(bot, id).await?;
        }
    }

    Ok(())
}

async fn send_help(bot: &Bot, chat_id: ChatId) -> anyhow::Result<()> {
    bot.send_message(chat_id, Command::descriptions().to_string())
        .await?;
    Ok(())
}

async fn handle_set_group(
    bot: &Bot,
    query: &str,
    telegram_id: i64,
    chat_id: ChatId,
    db: Db,
    schedule_repo: Arc<RwLock<ScheduleRepo>>,
) -> anyhow::Result<()> {
    let group_number = resolve_group_number(query, schedule_repo).await;

    match group_number {
        Some(number) => {
            db.set_group(telegram_id, number)
                .await
                .context("failed to set group")?;
            bot.send_message(chat_id, format!("Группа \"{}\" задана.", number))
                .await?;
        }
        None => {
            bot.send_message(chat_id, format!("Группа \"{}\" не найдена", query))
                .await?;
        }
    }

    Ok(())
}

async fn resolve_group_number(
    query: &str,
    schedule_repo: Arc<RwLock<ScheduleRepo>>,
) -> Option<i32> {
    let group_number = query.parse::<i32>().ok()?;
    let repo = schedule_repo.read().await;
    repo.get_by_group_number(group_number).map(|_| group_number)
}

async fn handle_schedule_search(
    bot: &Bot,
    telegram_id: i64,
    chat_id: ChatId,
    query: &str,
    schedule_repo: Arc<RwLock<ScheduleRepo>>,
) -> anyhow::Result<()> {
    let repo = schedule_repo.read().await;
    let schedule = repo.search(query);

    info!(
        "User: {}, query: {}, found count: {}",
        telegram_id,
        query,
        schedule.len()
    );

    match schedule.len() {
        0 => {
            bot.send_message(
                chat_id,
                format!("По запросу \"{}\" ничего не найдено", query),
            )
            .await?;
        }
        1 => {
            let schedule = schedule.first().unwrap();
            send_schedule(bot, chat_id, schedule).await?;
        }
        _ => {
            bot.send_message(
                chat_id,
                format!(
                    "Найдено несколько вариантов ({}). Уточните запрос. Может быть, вы искали:\n{}",
                    schedule.len(),
                    schedule
                        .into_iter()
                        .take(10)
                        .map(|schedule| schedule.format_info())
                        .collect::<Vec<String>>()
                        .join("\n")
                ),
            )
            .await?;
        }
    }

    Ok(())
}

async fn send_schedule(bot: &Bot, chat_id: ChatId, schedule: &Schedule) -> anyhow::Result<()> {
    let date = Utc::now().naive_local().date();
    let week_type = WeekType::for_date(date);
    let text = schedule.format_for_day(date, week_type);

    bot.send_message(chat_id, text)
        .reply_markup(inline_buttons(&schedule.schedule_type, date))
        .await?;
    Ok(())
}

pub async fn send_schedule_date(
    bot: &Bot,
    chat_id: ChatId,
    schedule: &Schedule,
    date: NaiveDate,
) -> anyhow::Result<()> {
    let week_type = WeekType::for_date(date);
    let text = schedule.format_for_day(date, week_type);

    bot.send_message(chat_id, text)
        .reply_markup(inline_buttons(&schedule.schedule_type, date))
        .await?;
    Ok(())
}
