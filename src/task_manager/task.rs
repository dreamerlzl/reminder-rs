use std::fmt::Display;
use std::time::Duration;

use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use serde_json::to_vec;
use time::{format_description, OffsetDateTime};

pub type TaskID = String;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    created_at: OffsetDateTime, // just a metadata
    pub description: String,
    pub task_id: TaskID, // used as the unique id of the task
    pub clock_type: ClockType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClockType {
    Once(OffsetDateTime),
    Period(Duration),
}

impl Display for ClockType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let format = format_description::parse("[year]-[month]-[day] [hour]:[minute]")
            .expect("fail to parse display format for OffsetDatetime");
        match self {
            ClockType::Once(next_fire) => {
                write!(
                    f,
                    "at {}",
                    next_fire
                        .format(&format)
                        .expect("fail to display custom OffsetDatetime format")
                )
            }
            ClockType::Period(period) => {
                write!(f, "every {} secs", period.as_secs())
            }
        }
    }
}

impl Task {
    pub fn new(description: String, clock_type: ClockType) -> Self {
        Task {
            description,
            clock_type,
            created_at: OffsetDateTime::now_utc(),
            task_id: nanoid!(),
            // task_id: Uuid::new_v4(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        to_vec(self).expect(&format!("fail to serialize task {:?}", &self))
    }
}

macro_rules! my_format {
    (task) => {
        "{: <22} {: <19} {:}"
    };
}

pub fn prompt_task() {
    println!(my_format!(task), "task_id", "clock", "description");
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            my_format!(task),
            self.task_id, self.clock_type, self.description,
        )
    }
}
