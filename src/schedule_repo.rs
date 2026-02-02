use std::collections::HashMap;

use crate::structures::{
    Schedule,
    ScheduleVariant::{Group, Teacher},
};

pub struct ScheduleRepo {
    schedule: Vec<Schedule>,
    last_updated: Option<i64>,
    number_to_index: HashMap<i32, usize>,
    isu_id_to_index: HashMap<i32, usize>,
    names_to_index: Vec<(String, usize)>,
}

impl ScheduleRepo {
    pub fn new() -> Self {
        Self {
            schedule: vec![],
            last_updated: None,
            number_to_index: HashMap::new(),
            isu_id_to_index: HashMap::new(),
            names_to_index: vec![],
        }
    }

    pub fn update(&mut self, schedule: Vec<Schedule>, last_updated: i64) {
        self.schedule = schedule;
        self.last_updated = Some(last_updated);

        let mut number_to_index = HashMap::new();
        let mut isu_id_to_index = HashMap::new();
        let mut names_to_index = vec![];
        for (index, schedule) in self.schedule.iter().enumerate() {
            match &schedule.schedule_type {
                Group(group) => {
                    number_to_index.insert(group.number, index);
                }
                Teacher(teacher) => {
                    names_to_index.push((teacher.name.to_lowercase(), index));
                    isu_id_to_index.insert(teacher.isu_id, index);
                }
            }
        }
        self.number_to_index = number_to_index;
        self.isu_id_to_index = isu_id_to_index;
        self.names_to_index = names_to_index;
    }

    pub fn search(&self, query: &str) -> Vec<&Schedule> {
        match query.parse::<i32>() {
            Ok(number) => self
                .number_to_index
                .get(&number)
                .into_iter()
                .map(|f| &self.schedule[*f])
                .collect(),
            Err(_) => {
                let lowercase = query.to_lowercase();
                self.names_to_index
                    .iter()
                    .filter(|(name, _)| name.contains(&lowercase))
                    .map(|(_, index)| &self.schedule[*index])
                    .collect()
            }
        }
    }

    pub fn get_by_isu_id(&self, teacher_isu_id: i32) -> Option<&Schedule> {
        self.isu_id_to_index
            .get(&teacher_isu_id)
            .and_then(|index| self.schedule.get(*index))
    }

    pub fn get_by_group_number(&self, group_number: i32) -> Option<&Schedule> {
        self.number_to_index
            .get(&group_number)
            .and_then(|index| self.schedule.get(*index))
    }

    pub fn last_updated(&self) -> Option<i64> {
        self.last_updated
    }
}
