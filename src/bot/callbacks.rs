use std::sync::Arc;

use anyhow::{Context, anyhow};
use chrono::{Duration, NaiveDate, Utc};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};
use tracing::{debug, error};

use crate::{
    formatting::ScheduleFormatting,
    state::AppState,
    structures::{ScheduleVariant, WeekType},
};

#[derive(Clone, Debug)]
enum ScheduleTarget {
    Group(i32),
    Teacher(i32),
}

#[derive(Clone, Debug)]
struct CallbackQueryData {
    target: ScheduleTarget,
    date: NaiveDate,
    week_type: WeekType,
}

impl CallbackQueryData {
    fn encode(&self) -> String {
        let (kind, id) = match self.target {
            ScheduleTarget::Group(id) => ("g", id),
            ScheduleTarget::Teacher(id) => ("t", id),
        };
        let week = match self.week_type {
            WeekType::All => "a",
            WeekType::Top => "t",
            WeekType::Bottom => "b",
        };
        format!("v1|{}|{}|{}|{}", kind, id, self.date, week)
    }

    fn decode(string: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = string.split('|').collect();
        if parts.len() != 5 {
            return Err(anyhow!("invalid callback data format"));
        }

        if parts[0] != "v1" {
            return Err(anyhow!("unsupported callback data version"));
        }

        let target = match parts[1] {
            "g" => ScheduleTarget::Group(parts[2].parse::<i32>().context("group id invalid")?),
            "t" => ScheduleTarget::Teacher(parts[2].parse::<i32>().context("teacher id invalid")?),
            _ => return Err(anyhow!("invalid target kind")),
        };

        let date = parts[3].parse::<NaiveDate>().context("invalid date")?;
        let week_type = match parts[4] {
            "a" => WeekType::All,
            "t" => WeekType::Top,
            "b" => WeekType::Bottom,
            _ => return Err(anyhow!("invalid week type")),
        };

        Ok(Self {
            target,
            date,
            week_type,
        })
    }
}

pub async fn callback_endpoint(
    bot: Bot,
    callback_query: CallbackQuery,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    if let Err(err) = handle_callback(&bot, callback_query, state).await {
        error!(?err, "callback handler failed");
    }
    Ok(())
}

async fn handle_callback(
    bot: &Bot,
    callback_query: CallbackQuery,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    let Some(message) = callback_query.message else {
        debug!("Message is None");
        return Ok(());
    };
    let Some(callback_data) = callback_query.data else {
        debug!("Callback data is None");
        return Ok(());
    };

    let schedule_query = match CallbackQueryData::decode(&callback_data) {
        Ok(schedule_query) => schedule_query,
        Err(err) => {
            error!(?err, callback_data = %callback_data, "failed to parse schedule query");
            return Ok(());
        }
    };

    let repo = state.schedule_repo.read().await;
    let schedule = match schedule_query.target {
        ScheduleTarget::Group(group_number) => repo.get_by_group_number(group_number),
        ScheduleTarget::Teacher(isu_id) => repo.get_by_isu_id(isu_id),
    };

    let Some(schedule) = schedule else {
        debug!(?schedule_query.target, "schedule not found for callback");

        return Ok(());
    };

    let chat_id = message.chat().id;
    let message_id = message.id();
    bot.edit_message_text(
        message.chat().id,
        message.id(),
        schedule.format_for_day(schedule_query.date, schedule_query.week_type.clone()),
    )
    .await?;

    bot.edit_message_reply_markup(chat_id, message_id)
        .reply_markup(inline_buttons(&schedule.schedule_type, schedule_query.date))
        .await?;

    Ok(())
}

fn target_from_variant(schedule_variant: &ScheduleVariant) -> ScheduleTarget {
    match schedule_variant {
        ScheduleVariant::Group(group) => ScheduleTarget::Group(group.number),
        ScheduleVariant::Teacher(teacher) => ScheduleTarget::Teacher(teacher.isu_id),
    }
}

pub fn inline_buttons(schedule_variant: &ScheduleVariant, day: NaiveDate) -> InlineKeyboardMarkup {
    let today = Utc::now().naive_local().date();
    let days_buttons = [day - Duration::days(1), today, day + Duration::days(1)]
        .into_iter()
        .enumerate()
        .map(|(index, day)| {
            let text = if index == 1 {
                "сегодня".to_string()
            } else {
                day.to_string()
            };

            let query = CallbackQueryData {
                target: target_from_variant(schedule_variant),
                date: day,
                week_type: WeekType::for_date(day),
            }
            .encode();

            InlineKeyboardButton::callback(text, query)
        });

    let week_types_buttons = [
        (WeekType::All, "обе"),
        (WeekType::Bottom, "нижняя"),
        (WeekType::Top, "верхняя"),
    ]
    .map(|(week_type, text)| {
        let query = CallbackQueryData {
            target: target_from_variant(schedule_variant),
            date: day,
            week_type,
        }
        .encode();

        InlineKeyboardButton::callback(text.to_string(), query)
    });

    InlineKeyboardMarkup::default()
        .append_row(days_buttons)
        .append_row(week_types_buttons)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_data_roundtrip_group() {
        let data = CallbackQueryData {
            target: ScheduleTarget::Group(42),
            date: NaiveDate::from_ymd_opt(2026, 2, 2).unwrap(),
            week_type: WeekType::Bottom,
        };

        let encoded = data.encode();
        let decoded = CallbackQueryData::decode(&encoded).unwrap();

        assert!(matches!(decoded.target, ScheduleTarget::Group(42)));
        assert_eq!(decoded.date, data.date);
        assert_eq!(decoded.week_type, data.week_type);
    }

    #[test]
    fn callback_data_roundtrip_teacher() {
        let data = CallbackQueryData {
            target: ScheduleTarget::Teacher(314),
            date: NaiveDate::from_ymd_opt(2026, 2, 3).unwrap(),
            week_type: WeekType::Top,
        };

        let encoded = data.encode();
        let decoded = CallbackQueryData::decode(&encoded).unwrap();

        assert!(matches!(decoded.target, ScheduleTarget::Teacher(314)));
        assert_eq!(decoded.date, data.date);
        assert_eq!(decoded.week_type, data.week_type);
    }
}
