use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Teacher {
    pub name: String,
    pub position: String,
    pub isu_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub number: i32,
    pub id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WeekType {
    Top,
    Bottom,
    All,
}

impl WeekType {
    pub fn to_ru_string(&self) -> &str {
        match self {
            WeekType::Top => "верхняя",
            WeekType::Bottom => "нижняя",
            WeekType::All => "обе",
        }
    }
    pub fn for_date(now: impl chrono::Datelike) -> WeekType {
        let week = now.iso_week().week();
        if week % 2 == 1 {
            WeekType::Top
        } else {
            WeekType::Bottom
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WeekType;
    use chrono::NaiveDate;

    #[test]
    fn week_type_from_iso_week_parity() {
        let cases = [
            ("2026-01-26", WeekType::Top),
            ("2026-02-01", WeekType::Top),
            ("2026-02-02", WeekType::Bottom),
            ("2026-02-08", WeekType::Bottom),
        ];

        for (date, expected) in cases {
            let date = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
            let actual = WeekType::for_date(date);
            assert_eq!(actual, expected);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassTime {
    pub time_from: String,
    pub time_to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Class {
    pub location: String,
    pub room: String,
    pub teacher: String,
    pub group_number: i32,
    pub time: ClassTime,
    pub week_type: WeekType,
    pub subject: String,
    pub class_type: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ScheduleVariant {
    Group(Group),
    Teacher(Teacher),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub schedule_type: ScheduleVariant,
    pub week: [Vec<Class>; 7],
}
