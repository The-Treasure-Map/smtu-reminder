use crate::structures::{
    Schedule,
    ScheduleVariant::{Group, Teacher},
    WeekType,
};
use chrono::Weekday;

pub trait ScheduleFormatting {
    fn format_for_day(&self, day: impl chrono::Datelike, week_type: WeekType) -> String;

    fn format_info(&self) -> String;
}

impl ScheduleFormatting for Schedule {
    fn format_for_day(&self, day: impl chrono::Datelike, week_type: WeekType) -> String {
        let weekday_name = format_weekday(day.weekday());
        let date = format!("{:02}.{:02}.{}", day.day(), day.month(), day.year());
        let mut string = format!(
            "{}\n{}, {} ({})",
            self.format_info(),
            weekday_name,
            date,
            format_week_type_header(&week_type)
        );

        let classes = &self.week[day.weekday().num_days_from_monday() as usize];
        let visible_classes = classes
            .iter()
            .filter(|class| matches_week_type(&week_type, &class.week_type))
            .collect::<Vec<_>>();

        if visible_classes.is_empty() {
            string.push_str("\nЗанятий нет.");
            return string;
        }

        string.push('\n');
        string.push('\n');

        let mut class_blocks = Vec::new();
        for class in visible_classes {
            let class_title = format_class_title(class);
            let time_range = format_time_range(&class.time.time_from, &class.time.time_to);

            let mut class_lines = Vec::new();
            class_lines.push(format!("{} - {}", time_range, class_title));

            match &self.schedule_type {
                Group(_) => {
                    let teacher = if class.teacher.trim().is_empty() {
                        "не указан"
                    } else {
                        class.teacher.trim()
                    };
                    class_lines.push(format!("Преподаватель: {}", teacher));
                }
                Teacher(_) => {
                    class_lines.push(format!("Группа: {}", class.group_number));
                }
            }

            let audience = format_audience(&class.location, &class.room);
            class_lines.push(format!("Ауд.: {}", audience));

            if matches!(week_type, WeekType::All) && !matches!(class.week_type, WeekType::All) {
                class_lines.push(format!(
                    "Неделя: {}",
                    format_week_type_short(&class.week_type)
                ));
            }

            if !class.note.is_empty() {
                class_lines.push(format!("Примечание: {}", class.note));
            }

            class_blocks.push(class_lines.join("\n"));
        }

        string.push_str(&class_blocks.join("\n\n"));
        string
    }

    fn format_info(&self) -> String {
        match &self.schedule_type {
            Group(group) => format!("Расписание группы {}", group.number),
            Teacher(teacher) => {
                format!(
                    "Расписание преподавателя {} ({})",
                    teacher.name, teacher.position
                )
            }
        }
    }
}

fn format_weekday(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "Понедельник",
        Weekday::Tue => "Вторник",
        Weekday::Wed => "Среда",
        Weekday::Thu => "Четверг",
        Weekday::Fri => "Пятница",
        Weekday::Sat => "Суббота",
        Weekday::Sun => "Воскресенье",
    }
}

fn format_week_type_header(week_type: &WeekType) -> &'static str {
    match week_type {
        WeekType::Top => "верхняя неделя",
        WeekType::Bottom => "нижняя неделя",
        WeekType::All => "обе недели",
    }
}

fn format_week_type_short(week_type: &WeekType) -> &'static str {
    match week_type {
        WeekType::Top => "верхняя",
        WeekType::Bottom => "нижняя",
        WeekType::All => "обе",
    }
}

fn matches_week_type(requested: &WeekType, class_week_type: &WeekType) -> bool {
    match requested {
        WeekType::All => true,
        WeekType::Top => matches!(class_week_type, WeekType::Top | WeekType::All),
        WeekType::Bottom => matches!(class_week_type, WeekType::Bottom | WeekType::All),
    }
}

fn format_class_title(class: &crate::structures::Class) -> String {
    let mut title = class.subject.to_string();

    title.push_str(" (");
    title.push_str(class.class_type.trim());
    title.push(')');

    title
}

fn format_time_range(time_from: &str, time_to: &str) -> String {
    format!("{}-{}", time_from, time_to)
}

fn format_audience(location: &str, room: &str) -> String {
    if location.is_empty() && room.is_empty() {
        return "не указана".to_string();
    }

    format!("{} {}", location, room)
}
