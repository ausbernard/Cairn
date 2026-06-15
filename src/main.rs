mod claude;
mod config;
mod db;
mod models;
mod render;

use anyhow::Result;
use clap::{Parser, Subcommand};
use models::TaskStatus;
use owo_colors::OwoColorize;

#[derive(Parser)]
#[command(
    name = "cairn",
    version,
    about = "💡 session-sized planning for people with no free time",
    arg_required_else_help = false,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Capture an idea into the inbox (offline, instant)
    Capture {
        /// The idea, in quotes
        title: String,
        /// Optional longer notes
        #[arg(short, long)]
        notes: Option<String>,
    },
    /// Ask Claude to break an idea into session-sized tasks
    Plan {
        /// Idea id from the inbox
        id: i64,
    },
    /// Show tonight's session: one active task, what's up next
    Today,
    /// Mark a task done
    Done {
        /// Task id
        id: i64,
        /// Handoff note (skips the interactive prompt)
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Mark a task stuck; Claude inserts a learning task ahead of it
    Stuck {
        /// Task id
        id: i64,
        /// Blocker description (skips the interactive prompt)
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Show what's scheduled for the next 7 days
    Week,
    /// List all ideas (inbox and planned)
    List,
    /// Skip today; push all open tasks to the next available session
    Skip,
    /// Reset a stuck task back to queued (no Claude call)
    Requeue {
        /// Task id
        id: i64,
    },
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() || argv == ["-h"] || argv == ["--help"] || argv == ["help"] {
        render::help();
        return;
    }
    if let Err(e) = run() {
        render::warn(&format!("{e}"));
        std::process::exit(1);
    }
}

/// Prompts for a one-line note when stdin is a TTY. Returns None on empty
/// input, Esc, or when running non-interactively (scripts, cron).
fn prompt_note(message: &str) -> Result<Option<String>> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return Ok(None);
    }
    match inquire::Text::new(message).prompt_skippable() {
        Ok(Some(s)) if !s.trim().is_empty() => Ok(Some(s.trim().to_owned())),
        Ok(_) => Ok(None),
        Err(inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("{e}")),
    }
}

fn resolve_note(inline: Option<String>, prompt_msg: &str) -> Result<Option<String>> {
    match inline {
        Some(s) if !s.trim().is_empty() => Ok(Some(s)),
        Some(_) => Ok(None),
        None => prompt_note(prompt_msg),
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let conn = db::open()?;

    match cli.command {
        Command::Capture { title, notes } => {
            let id = db::capture_idea(&conn, &title, notes.as_deref())?;
            let waiting = db::inbox_count(&conn)?;
            render::ok(&format!(
                "captured #{id} → inbox {}",
                format!("({waiting} waiting)").dimmed()
            ));
        }

        Command::Plan { id } => {
            let cfg = config::load()?;
            let idea = db::get_idea(&conn, id)?;
            let spin = render::Spinner::new(format!("planning \"{}\"…", idea.title));
            let drafts = claude::plan_idea(&cfg, &idea)?;
            drop(spin);
            let dates = cfg.schedule_dates(drafts.len());
            let ids = db::insert_drafts(&conn, idea.id, &drafts, &dates)?;
            db::set_idea_status(&conn, idea.id, models::IdeaStatus::Planned)?;
            let n = drafts.len();
            render::ok(&format!("{n} task{} queued", if n == 1 { "" } else { "s" }));
            let rows: Vec<(i64, &models::TaskDraft)> =
                ids.into_iter().zip(drafts.iter()).collect();
            render::plan_table(&rows);
        }

        Command::Today => {
            let cfg = config::load()?;
            let tasks = db::open_tasks(&conn)?;
            let handoff = db::latest_handoff(&conn)?;
            render::today(&tasks, cfg.max_session_minutes, handoff.as_deref());
        }

        Command::Done { id, note } => {
            db::set_task_status(&conn, id, TaskStatus::Done)?;
            render::ok(&format!("#{id} done — nice work 🎉"));
            let note = resolve_note(note, "where'd you leave off / what's next? (enter to skip)")?;
            db::record_session(&conn, id, "done", note.as_deref())?;
            if note.is_some() {
                println!("{}", "   saved to session log".dimmed());
            }
        }

        Command::Week => {
            let tasks = db::week_tasks(&conn)?;
            render::week(&tasks);
        }

        Command::List => {
            let ideas = db::list_ideas(&conn)?;
            render::idea_list(&ideas);
        }

        Command::Requeue { id } => {
            db::set_task_status(&conn, id, TaskStatus::Queued)?;
            render::ok(&format!("#{id} back in the queue"));
        }

        Command::Skip => {
            let cfg = config::load()?;
            let next = cfg
                .next_available_day()
                .ok_or_else(|| anyhow::anyhow!("no available days configured — add some to your config.toml"))?;
            let next_str = next.format("%Y-%m-%d").to_string();
            let n = db::skip_today(&conn, &next_str)?;
            let label = next.format("%a %b %-d").to_string().to_lowercase();
            render::ok(&format!(
                "today skipped — {n} task{} pushed to {label}",
                if n == 1 { "" } else { "s" }
            ));
        }

        Command::Stuck { id, note } => {
            let task = db::get_task(&conn, id)?;
            let cfg = config::load()?;
            db::set_task_status(&conn, id, TaskStatus::Stuck)?;
            let note = resolve_note(note, "what's blocking you? (enter to skip)")?;
            db::record_session(&conn, id, "stuck", note.as_deref())?;
            if note.is_some() {
                println!("{}", "   saved to session log".dimmed());
            }
            let spin = render::Spinner::new("asking claude what's blocking…");
            let draft = claude::unblock_task(&cfg, &task, note.as_deref())?;
            drop(spin);
            let new_id = db::insert_before(&conn, &task, &draft)?;
            render::unblock_result(new_id, &draft.description, draft.session_minutes, draft.resource_url.as_deref());
            render::ok(&format!("scheduled {} #{id}", "before".yellow()));
        }
    }
    Ok(())
}
