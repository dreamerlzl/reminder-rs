use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::{to_string, Deserializer};
#[macro_use]
extern crate prettytable;
use std::env;
use std::io::{BufReader, Write};
use std::net::TcpStream;

use prettytable::Table;
use task_reminder::comm::{get_local_now, parse_at, parse_duration, Request, Response};
use task_reminder::task_manager::ClockType;

#[derive(Parser)]
#[command(author, version, about, long_about=None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Add {
        description: String,
        #[command(subcommand)]
        command: AddCommand,

        #[arg(short, long)]
        image_path: Option<String>,

        #[arg(short, long)]
        sound_path: Option<String>,
    },
    Rm {
        task_id: String,
    },
    List,
}

#[derive(Subcommand)]
enum AddCommand {
    After {
        duration: String,
    },
    At {
        time: String,
        #[arg(short, long)]
        per_day: bool,
    },
    Per {
        duration: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let request = match cli.command {
        Command::Add {
            description,
            command,
            mut image_path,
            mut sound_path,
        } => {
            let clock_type = match command {
                AddCommand::At { time, per_day } => {
                    let next_fire = parse_at(&time)?;
                    if per_day {
                        ClockType::OncePerDay(next_fire.hour(), next_fire.minute())
                    } else {
                        ClockType::Once(next_fire)
                    }
                }
                AddCommand::After { duration } => {
                    let duration = parse_duration(&duration)?;
                    if duration.as_secs() == 0 {
                        return Err(anyhow!("after <duration> should not be 0"));
                    }
                    let next_fire = get_local_now() + duration;
                    ClockType::Once(next_fire)
                }
                AddCommand::Per { duration } => {
                    let _ = parse_duration(&duration)?;
                    ClockType::Period(duration)
                }
            };
            if image_path.is_none() {
                if let Ok(system_image_path) = env::var("FMN_IMAGE_PATH") {
                    image_path = Some(system_image_path);
                }
            }
            if sound_path.is_none() {
                if let Ok(system_sound_path) = env::var("FMN_SOUND_PATH") {
                    sound_path = Some(system_sound_path);
                }
            }
            Request::Add(description, clock_type, image_path, sound_path)
        }
        Command::Rm { task_id } => Request::Cancel(task_id),
        Command::List => Request::Show,
    };

    //println!("request is {:?}", request);
    let dest = env::var("FMN_DAEMON_ADDR").unwrap_or_else(|_| "127.0.0.1:8082".to_owned());
    match send_request(request.clone(), &dest) {
        Ok(response) => match response {
            Response::GetTasks(tasks) => {
                let mut table = Table::new();
                table.add_row(row!["ID", "TYPE", "DESCRIPTION"]);
                for task in tasks {
                    table.add_row(row![task.task_id, task.clock_type, task.description]);
                }
                table.printstd();
            }
            _ => println!("success: {:?}", response),
        },
        Err(e) => {
            println!("request \"{:?}\" failed: {}", request, e);
        }
    }
    Ok(())
}

fn send_request(request: Request, dest: &str) -> Result<Response> {
    let mut stream = TcpStream::connect(dest).context("fail to connect to fmn-deamon")?;
    let serialized = to_string(&request).expect("fail to serialize request");
    stream
        .write_all(serialized.as_bytes())
        .context("fail to send requests to fmn-daemon")?;

    // receive response
    let mut reader = Deserializer::from_reader(BufReader::new(stream.try_clone()?));
    let response: Response =
        Response::deserialize(&mut reader).context("fail to deserialize response")?;
    Ok(response)
}
