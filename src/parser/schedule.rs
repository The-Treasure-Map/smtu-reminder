use std::{
    array,
    sync::Arc,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::{Context, anyhow};
use chrono::{Local, Weekday};
use futures::stream::{self, StreamExt, TryStreamExt};
use scraper::{ElementRef, Html, Selector};
use tracing::info;

use crate::{
    parser::urls::{GROUP_SCHEDULE, HOST, TEACHER_SCHEDULE},
    structures::{Class, ClassTime, Schedule, ScheduleVariant, WeekType},
};

pub async fn fetch_all(client: &reqwest::Client) -> anyhow::Result<Vec<Schedule>> {
    let start_time = Local::now().format("%H:%M:%S");
    info!("schedule fetch started at {}", start_time);

    let (groups, teachers) = tokio::try_join!(
        super::groups::fetch_groups(client),
        super::teachers::fetch_teachers(client)
    )?;

    let total = groups.len() + teachers.len();

    let mut schedule_types = Vec::with_capacity(total);
    schedule_types.extend(groups.into_iter().map(ScheduleVariant::Group));
    schedule_types.extend(teachers.into_iter().map(ScheduleVariant::Teacher));

    let completed = Arc::new(AtomicUsize::new(0));
    let next_threshold = Arc::new(AtomicUsize::new(10));

    let schedules = stream::iter(schedule_types)
        .map(|schedule_type| {
            let completed = Arc::clone(&completed);
            let next_threshold = Arc::clone(&next_threshold);
            async move {
                let result = fetch_schedule(client, schedule_type).await;

                let finished = completed.fetch_add(1, Ordering::Relaxed) + 1;
                let percent = finished * 100 / total;
                let mut threshold = next_threshold.load(Ordering::Relaxed);
                while percent >= threshold && threshold <= 100 {
                    let time = Local::now().format("%H:%M:%S");
                    info!(
                        "schedule fetch progress {}% ({}/{}) at {}",
                        threshold, finished, total, time
                    );
                    threshold = threshold.saturating_add(10);
                    next_threshold.store(threshold, Ordering::Relaxed);
                }

                result
            }
        })
        .buffer_unordered(10)
        .try_collect::<Vec<_>>()
        .await?;

    let end_time = Local::now().format("%H:%M:%S");
    info!("schedule fetch finished at {}", end_time);

    Ok(schedules)
}

pub async fn fetch_schedule(
    client: &reqwest::Client,
    schedule_type: ScheduleVariant,
) -> anyhow::Result<Schedule> {
    let path = match &schedule_type {
        ScheduleVariant::Group(group) => format!("{}{}{}{}", HOST, GROUP_SCHEDULE, group.id, "/"),
        ScheduleVariant::Teacher(teacher) => {
            format!("{}{}{}{}", HOST, TEACHER_SCHEDULE, teacher.isu_id, "/")
        }
    };

    let html = client.get(path).send().await?.text().await?;

    let doc = Html::parse_document(&html);
    let mut week: [Vec<Class>; 7] = array::from_fn(|_| vec![]);

    for day in doc.select(&Selector::parse(".card.my-4").unwrap()) {
        let day_name = day
            .select(&Selector::parse(".card-header").unwrap())
            .next()
            .context("day has no header")?
            .text()
            .collect::<String>()
            .replace('\n', "")
            .trim()
            .to_string();

        let day_index = day_ordinal(&day_name).context("unknown day name")?;
        week[day_index] = process_day(day)?;
    }

    Ok(Schedule {
        schedule_type,
        week,
    })
}

fn process_day(day: ElementRef) -> anyhow::Result<Vec<Class>> {
    let tbody = day
        .select(&Selector::parse("tbody").unwrap())
        .next()
        .context("day has no table body")?;

    tbody
        .select(&Selector::parse("tr").unwrap())
        .map(process_class)
        .collect()
}

fn process_class(class: ElementRef) -> anyhow::Result<Class> {
    let time_element = class
        .select(&Selector::parse("th[scope=\"row\"]").unwrap())
        .next()
        .context("class has no time cell")?;
    let time_text = time_element.text().collect::<String>();
    let mut time_parts = time_text.split('-');
    let time_from = time_parts.next().unwrap_or("").trim().to_string();
    let time_to = time_parts.next().unwrap_or("").trim().to_string();

    let td_selector = Selector::parse("td").unwrap();
    let mut tds = class.select(&td_selector);
    let week_type_element = tds.next().context("class has no week type cell")?;
    let room_element = tds.next().context("class has no room cell")?;
    let group_element = tds.next().context("class has no group cell")?;
    let subject_element = tds.next().context("class has no subject cell")?;
    let teacher_element = tds.next().context("class has no teacher cell")?;

    let week_type_title = week_type_element
        .select(&Selector::parse("i").unwrap())
        .next()
        .and_then(|el| el.value().attr("data-bs-title"))
        .context("class has no week type title")?;
    let week_type = parse_week_type(week_type_title)?;

    let room_text = room_element.text().collect::<String>();
    let room_text = room_text.trim();
    let (location, room) = if let Some((building, room)) = room_text.split_once(' ') {
        if building == "Спортивный" {
            (format!("{} {}", building, room), String::new())
        } else {
            (building.to_string(), room.to_string())
        }
    } else {
        (room_text.to_string(), String::new())
    };

    let group_number = group_element
        .text()
        .collect::<String>()
        .replace('\n', "")
        .trim()
        .parse::<i32>()
        .context("group number is not a number")?;

    let subject_parts = subject_element
        .text()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect::<Vec<String>>();

    let subject = subject_parts.first().cloned().unwrap_or_default();
    let class_type = subject_parts.get(1).cloned().unwrap_or_default();
    let note = subject_parts.get(2).cloned().unwrap_or_default();

    let teacher = teacher_element
        .text()
        .collect::<String>()
        .replace('\n', "")
        .trim()
        .to_string();

    Ok(Class {
        location,
        room,
        teacher,
        group_number,
        time: ClassTime { time_from, time_to },
        week_type,
        subject,
        class_type,
        note,
    })
}

fn parse_week_type(title: &str) -> anyhow::Result<WeekType> {
    match title.trim() {
        "Обе недели" => Ok(WeekType::All),
        "Верхняя неделя" => Ok(WeekType::Top),
        "Нижняя неделя" => Ok(WeekType::Bottom),
        _ => Err(anyhow!("unknown week type: {title}")),
    }
}

fn day_ordinal(name: &str) -> Option<usize> {
    let weekday = match name.trim() {
        "Понедельник" => Weekday::Mon,
        "Вторник" => Weekday::Tue,
        "Среда" => Weekday::Wed,
        "Четверг" => Weekday::Thu,
        "Пятница" => Weekday::Fri,
        "Суббота" => Weekday::Sat,
        "Воскресенье" => Weekday::Sun,
        _ => return None,
    };

    Some((weekday.number_from_monday() - 1) as usize)
}
