use crate::comm::parse_duration;
use crate::notify::desktop_notification;
use crate::task_manager::{ClockType, Task, TaskID};

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use log::{debug, error, info, warn};
use time::{OffsetDateTime, UtcOffset};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::time::sleep;

const SUMMARY: &str = "forget-me-not";

pub struct Scheduler {
    task_sender: mpsc::Sender<SchedulerCommand>,
}

pub struct InnerScheduler {
    cancel_channels: HashMap<TaskID, broadcast::Sender<TaskCommand>>,
    tzdiff: UtcOffset,
}

#[derive(Debug)]
enum SchedulerCommand {
    Add(Task),
    Cancel(Task),
}

#[derive(Clone, Debug)]
enum TaskCommand {
    Stop,
}

impl Scheduler {
    pub fn new() -> Self {
        let tzdiff =
            UtcOffset::current_local_offset().expect("fail to get local timezone difference");
        // if we use multi-threaded runtime, time-rs or chrono's now method is not reliable
        let (sender, receiver) = mpsc::channel(8);
        std::thread::spawn(
            move || match Builder::new_current_thread().enable_all().build() {
                Ok(rt) => {
                    let mut inner = InnerScheduler::new(tzdiff);
                    inner.start(rt, receiver);
                }
                Err(e) => {
                    error!("fail to create async runtime: {}", e)
                }
            },
        );
        Scheduler {
            task_sender: sender,
        }
    }

    pub fn add_task(&mut self, task: Task) -> Result<()> {
        if self.check_inner_scheduler_crashed() {
            panic!("the inner scheduler has paniced!");
        }
        let clock_type = task.clock_type.clone();
        match self.task_sender.blocking_send(SchedulerCommand::Add(task)) {
            Ok(()) => {
                debug!(
                    "successfully send new task to inner scheduler: {}",
                    clock_type
                );
                Ok(())
            }
            Err(e) => Err(anyhow!("fail to send new task to inner scheduler: {}", e)),
        }
    }

    pub fn cancel_task(&self, task: Task) -> Result<()> {
        if self.check_inner_scheduler_crashed() {
            panic!("the inner scheduler has paniced!");
        }
        let task_id = task.task_id.clone();
        match self
            .task_sender
            .blocking_send(SchedulerCommand::Cancel(task))
        {
            Ok(()) => {
                debug!(
                    "successfully cancel new task to inner scheduler: {}",
                    task_id
                );
                Ok(())
            }
            Err(e) => Err(anyhow!(
                "fail to send cancel task to inner scheduler: {}",
                e
            )),
        }
    }

    fn check_inner_scheduler_crashed(&self) -> bool {
        self.task_sender.is_closed()
    }
}

impl InnerScheduler {
    fn new(tzdiff: UtcOffset) -> Self {
        InnerScheduler {
            cancel_channels: HashMap::new(),
            tzdiff,
        }
    }

    fn start(&mut self, rt: Runtime, mut task_receiver: mpsc::Receiver<SchedulerCommand>) {
        rt.block_on(async {
            while let Some(scheduler_command) = task_receiver.recv().await {
                match scheduler_command {
                    SchedulerCommand::Add(task) => {
                        self.add_task(task);
                    }
                    SchedulerCommand::Cancel(task_id) => {
                        if let Err(e) = self.cancel_task(task_id) {
                            // no active receivers
                            // https://docs.rs/tokio/latest/tokio/sync/broadcast/error/struct.SendError.html
                            error!("fail to cancel task: {}", e);
                        }
                    }
                }
            }
        });
    }

    pub fn add_task(&mut self, task: Task) {
        // we finally need to insert task_id as a key so it's fine to clone here
        let task_id = task.task_id.clone();
        let clock_type = task.clock_type.clone();
        info!("add new clock task: {}, {}", task_id, clock_type);
        let (sender, receiver) = broadcast::channel(1);
        // enter the tokio rt context so that we can use tokio::spawn
        match clock_type {
            ClockType::Once(next_fire) => tokio::spawn(once_clock(task, next_fire, receiver)),
            ClockType::Period(period) => {
                let duration = parse_duration(&period)
                    .expect("this shall have been verified by the client side");
                tokio::spawn(period_clock(task, duration, sender.clone(), receiver))
            }
            ClockType::OncePerDay(hour, minute) => {
                let (hour_diff, minute_diff, _) = self.tzdiff.clone().as_hms();
                let sender = sender.clone();
                tokio::spawn(period_do(
                    Duration::from_secs(60),
                    receiver,
                    move || {
                        info!("everyday task at {}:{} is cancelled!", hour, minute);
                    },
                    move || {
                        let now = OffsetDateTime::now_utc();
                        let now_hour = now.hour() as i8 + hour_diff;
                        let now_minute = now.minute() as i8 + minute_diff;
                        if (now_hour as u8, now_minute as u8) == (hour, minute) {
                            info!(
                                "a clock at {}:{} everyday and description {} fire!",
                                hour, minute, &task.description
                            );
                            if let Err(e) = desktop_notification(
                                SUMMARY,
                                &task.description,
                                task.get_image(),
                                task.get_sound(),
                            ) {
                                error!("fail to send de notification: {}", e);
                                sender
                                    .send(TaskCommand::Stop)
                                    .expect("fail to stop after de notify err");
                            }
                        }
                    },
                ))
            }
        };
        self.cancel_channels.insert(task_id, sender);
    }

    pub fn cancel_task(&mut self, task: Task) -> Result<()> {
        let task_id = task.task_id;
        if let Some(sender) = self.cancel_channels.get(&task_id) {
            if let Err(e) = sender
                .send(TaskCommand::Stop)
                .context("fail to send stop to clock")
            {
                // no active receivers
                return Err(anyhow!("fail to send cancel task: {}", e));
            }
        } else {
            warn!("fail to find sender channel for task id: {}", task_id);
        }
        Ok(())
    }
}

async fn period_clock(
    task: Task,
    period: Duration,
    sender: broadcast::Sender<TaskCommand>,
    receiver: broadcast::Receiver<TaskCommand>,
) {
    period_do(
        period,
        receiver,
        || {
            info!("periodic task with period {:?} is cancelled!", period);
        },
        || {
            info!(
                "a clock with period {} and description {} fire!",
                period.as_secs(),
                &task.description
            );
            if let Err(e) = desktop_notification(
                SUMMARY,
                &task.description,
                task.get_image(),
                task.get_sound(),
            ) {
                error!("fail to send de notification: {}", e);
                sender
                    .send(TaskCommand::Stop)
                    .expect("fail to stop after de notify err");
            }
        },
    )
    .await;
}

async fn period_do<F1, F2>(
    period: Duration,
    mut receiver: broadcast::Receiver<TaskCommand>,
    after_cancel: F1,
    after_wake: F2,
) where
    F1: Fn(),
    F2: Fn(),
{
    loop {
        tokio::select! {
            val = receiver.recv() => {
                if is_canceled(val) {
                    after_cancel();
                    return
                }
            }
            _ = sleep(period) => {
                after_wake();
            }
        }
    }
}

async fn once_clock(
    task: Task,
    next_fire: OffsetDateTime,
    mut receiver: broadcast::Receiver<TaskCommand>,
) {
    let now = OffsetDateTime::now_utc();
    if now >= next_fire {
        warn!(
            "clock next_fire time {} shouldn't be in the past!",
            next_fire
        );
        return;
    }
    let duration = (next_fire - now).unsigned_abs();
    tokio::select! {
        val = receiver.recv() => {
            if is_canceled(val) {
                info!("once clock with next_fire {:?} is cancelled!", next_fire);
                return
            }
        }
        _ = sleep(duration) => {
            info!("a clock fire!");
            if let Err(e) = desktop_notification(SUMMARY, &task.description, task.get_image(), task.get_sound()) {
                error!("fail to send notification: {}", e);
            }
        }
    }
}

fn is_canceled(val: std::result::Result<TaskCommand, RecvError>) -> bool {
    match val {
        Ok(command) => match command {
            TaskCommand::Stop => true,
        },
        Err(e) => {
            error!("fail to receive command: {}", e);
            true
        }
    }
}
