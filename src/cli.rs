use crate::{
    model::{Task, safe},
    store::Store,
};
use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::io::{self, IsTerminal};

#[derive(Parser, Debug)]
#[command(
    name = "todo",
    version,
    about = "Your tasks. Nothing left behind.",
    after_help = "Run `todo` without a command to open the dashboard.\nTask numbers are stable IDs, not list positions."
)]
pub struct Cli {
    #[arg(long, value_enum, default_value_t = ColorMode::Auto, global = true)]
    pub color: ColorMode,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl Cli {
    pub fn colors(&self) -> bool {
        match self.color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => {
                io::stdout().is_terminal()
                    && std::env::var_os("NO_COLOR").is_none()
                    && std::env::var("TERM").as_deref() != Ok("dumb")
            }
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Open the interactive dashboard (also the default).
    Tui,
    /// Add a task. Quotes are optional for multi-word text.
    Add {
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
    },
    /// List open tasks, or choose all/completed tasks.
    List {
        #[arg(long, conflicts_with = "done")]
        all: bool,
        #[arg(long)]
        done: bool,
    },
    /// Edit a task using its stable ID.
    #[command(visible_alias = "edit")]
    Update {
        id: u64,
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
    },
    /// Mark a task as completed.
    Done { id: u64 },
    /// Reopen a completed task.
    Undo { id: u64 },
    /// Permanently delete a task by ID.
    #[command(visible_alias = "del")]
    Delete { id: u64 },
}

pub fn run(command: &Command, store: &Store, color: bool) -> Result<()> {
    match command {
        Command::List { all, done } => {
            let db = store.load()?;
            let tasks: Vec<_> = db
                .tasks
                .iter()
                .filter(|task| *all || task.completed == *done)
                .collect();
            if tasks.is_empty() {
                println!(
                    "{}",
                    if *all {
                        "No tasks yet. Add one with `todo add \"Your task\"`."
                    } else if *done {
                        "No completed tasks yet."
                    } else {
                        "No open tasks. All clear!"
                    }
                );
            }
            for task in tasks {
                print_task(task, color);
            }
        }
        Command::Add { text } => {
            let (_, task) = store.transact(|db| db.add(&text.join(" ")))?;
            success("Added", &task, color);
        }
        Command::Update { id, text } => {
            let (_, task) = store.transact(|db| db.update(*id, &text.join(" ")))?;
            success("Updated", &task, color);
        }
        Command::Done { id } | Command::Undo { id } => {
            let completed = matches!(command, Command::Done { .. });
            let (_, task) = store.transact(|db| db.set_completed(*id, completed))?;
            success(
                if completed { "Completed" } else { "Reopened" },
                &task,
                color,
            );
        }
        Command::Delete { id } => {
            let (_, task) = store.transact(|db| db.delete(*id))?;
            success("Deleted", &task, color);
        }
        Command::Tui => unreachable!("Dashboard commands are handled by main"),
    }
    Ok(())
}

fn print_task(task: &Task, color: bool) {
    let marker = if task.completed { "[x]" } else { "[ ]" };
    if color {
        let shade = if task.completed {
            "38;2;159;149;183"
        } else {
            "38;2;167;139;250"
        };
        println!(
            "\x1b[{shade}m{marker} #{}\x1b[0m {}",
            task.id,
            safe(&task.title)
        );
    } else {
        println!("{marker} #{} {}", task.id, safe(&task.title));
    }
}

fn success(action: &str, task: &Task, color: bool) {
    if color {
        println!(
            "\x1b[38;2;167;139;250m{action}\x1b[0m #{}: {}",
            task.id,
            safe(&task.title)
        );
    } else {
        println!("{action} #{}: {}", task.id, safe(&task.title));
    }
}
