use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::Subcommand;
use log::warn;
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset};

use crate::task_manager::{ClockType, Task, TaskContext, TaskID};

static TZDIFF: OnceCell<UtcOffset> = OnceCell::new();

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Request {
    // name, clock_type, image_path, sound_path
    Add(String, ClockType, Option<String>, Option<String>),
    Cancel(TaskID),
    Show,
    ContextRequest(ContextCommand),
}

#[derive(Subcommand, Debug, Serialize, Deserialize, Clone)]
pub enum ContextCommand {
    Set { context: TaskContext },
    List,
    Rm { context: TaskContext },
    Define { context: TaskContext },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    AddSuccess,    // for add task / define context
    RemoveSuccess, // for rm task/context
    Fail(String),
    GetTasks(Vec<Task>),
    GetContexts(Vec<TaskContext>), // for list context
    SetContextSuccess,             // for set context
}

pub fn parse_duration(duration: &str) -> Result<Duration> {
    let re = Regex::new(
        r"^(?:(?P<day>\d+)d)?(?:(?P<hour>\d+)h)?(?:(?P<minute>\d+)m)?(?:(?P<second>\d+)s)?$",
    )
    .unwrap();
    if !re.is_match(duration) {
        return Err(anyhow!(
            "invalid duration format; valid examples: 1d1h1m1s, 2h, 30s, 55m"
        ));
    }
    if let Some(captures) = re.captures(duration) {
        let mut components = [0_u64; 4];
        for (i, component) in ["day", "hour", "minute", "second"].into_iter().enumerate() {
            components[i] = captures
                .name(component)
                .map(|m| {
                    // dbg!(component, m.as_str());
                    m.as_str()
                })
                .unwrap_or_else(|| "0")
                .parse()
                .context(format!("invalid {component}"))?;
        }

        let secs =
            components[0] * 3600 * 24 + components[1] * 3600 + components[2] * 60 + components[3];
        Ok(Duration::from_secs(secs))
    } else {
        Ok(Duration::from_secs(0))
    }
}

pub fn get_tzdiff() -> UtcOffset {
    TZDIFF.get_or_init(|| {
        UtcOffset::current_local_offset().expect("fail to get local timezone difference")
    });
    let offset = *TZDIFF.get().unwrap();
    offset
}

pub fn get_local_now() -> OffsetDateTime {
    OffsetDateTime::now_utc().to_offset(get_tzdiff())
}

// only used for at
pub fn parse_at(next_fire: &str) -> Result<OffsetDateTime> {
    let re = Regex::new(r"(?P<hour>\d+):(?P<minute>\d+)").unwrap();
    let mut components = [0_u8; 3];
    if let Some(captures) = re.captures(next_fire) {
        for (i, component) in ["hour", "minute"].into_iter().enumerate() {
            components[i] = captures
                .name(component)
                .map(|m| {
                    // dbg!(component, m.as_str());
                    m.as_str()
                })
                .ok_or_else(|| anyhow!("invalid time! correct examples: 13:11:04, 23:01:59"))?
                .parse()
                .context(format!("invalid {component}"))?;
        }
        let hour = components[0];
        let minute = components[1];
        let now = get_local_now();
        let mut next_fire = now
            .replace_millisecond(0)?
            .replace_nanosecond(0)?
            .replace_microsecond(0)?
            .replace_hour(hour)?
            .replace_minute(minute)?;

        if now >= next_fire {
            warn!(
                "clock next_fire time {} shouldn't be in the past! would reschedule it tomorrow",
                next_fire
            );
            next_fire = next_fire
                .replace_day(now.day() + 1)
                .expect("fail to reschedule the next day");
        }
        Ok(next_fire)
    } else {
        Err(anyhow!("fail to parse next_fire!"))
    }
}
