use crate::models::{Task, TaskStatus};
use owo_colors::OwoColorize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    pub fn new(msg: impl Into<String> + Send + 'static) -> Self {
        use std::io::{IsTerminal, Write};
        let running = Arc::new(AtomicBool::new(true));
        if !std::io::stdout().is_terminal() {
            return Self { running, handle: None };
        }
        let r = running.clone();
        let msg = msg.into();
        let handle = std::thread::spawn(move || {
            let frames = ["◐", "◓", "◑", "◒"];
            let mut i = 0usize;
            while r.load(Ordering::Relaxed) {
                let frame = frames[i].truecolor(187, 154, 247).to_string();
                let label = msg.truecolor(86, 95, 137).to_string();
                print!("\r  {frame}  {label}");
                let _ = std::io::stdout().flush();
                std::thread::sleep(std::time::Duration::from_millis(120));
                i = (i + 1) % frames.len();
            }
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
        });
        Self { running, handle: Some(handle) }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

pub fn status_glyph(s: TaskStatus) -> String {
    match s {
        TaskStatus::Active => "▸".yellow().to_string(),
        TaskStatus::Queued => "○".blue().to_string(),
        TaskStatus::Done => "✓".green().to_string(),
        TaskStatus::Stuck => "✗".red().to_string(),
    }
}

pub fn ok(msg: &str) {
    println!("{} {msg}", "✓".green());
}

pub fn warn(msg: &str) {
    println!("{} {msg}", "!".red());
}

pub fn header(msg: &str) {
    println!("{}", format!("── {msg} ──").dimmed());
}

fn split_criteria(s: &str) -> Vec<String> {
    if s.contains("; ") {
        return s.split("; ").map(|c| c.trim().to_owned()).collect();
    }
    if s.contains(" — ") {
        return s.split(" — ").map(|c| c.trim().to_owned()).collect();
    }
    vec![s.to_owned()]
}

fn readiness(s: TaskStatus) -> String {
    match s {
        TaskStatus::Queued => "● ready".truecolor(158, 206, 106).to_string(),
        TaskStatus::Active => "◐ in progress".truecolor(224, 175, 104).to_string(),
        TaskStatus::Stuck  => "✗ stuck".red().to_string(),
        TaskStatus::Done   => "✓ done".green().to_string(),
    }
}

pub fn today(tasks: &[Task], free_minutes: u32, handoff: Option<&str>) {
    let date = chrono::Local::now().format("%a %b %-d").to_string().to_lowercase();
    println!(
        "  {}  {}  {}",
        "◷".truecolor(187, 154, 247),
        format!("today · {date}").bold(),
        format!("1 session free · {free_minutes}m").truecolor(86, 95, 137),
    );
    println!();

    if tasks.is_empty() {
        println!("  nothing queued — {} something 💡", "idea capture".bold());
        return;
    }

    let (first, rest) = tasks.split_first().expect("non-empty checked above");
    let bar = "│".truecolor(122, 162, 247).to_string();

    // metadata row
    println!(
        "{bar}  {}  {}  {}  {}  {}  {}  {}",
        first.kind.glyph(),
        format!("#{}", first.id).truecolor(224, 175, 104),
        "·".truecolor(86, 95, 137),
        first.kind.as_str().truecolor(122, 162, 247),
        "·".truecolor(86, 95, 137),
        format!("◷ {}m", first.session_minutes).truecolor(158, 206, 106),
        readiness(first.status),
    );
    println!("{bar}");

    // description
    println!("{bar}  {}", first.description.bold().truecolor(192, 202, 245));
    println!("{bar}");

    // done when checkboxes
    println!("{bar}  {}", "✓ done when".truecolor(158, 206, 106));
    for c in split_criteria(&first.definition_of_done) {
        println!(
            "{bar}    {}  {}",
            "▢".truecolor(86, 95, 137),
            c.truecolor(169, 177, 214),
        );
    }

    // last time note
    if let Some(note) = handoff {
        println!("{bar}");
        println!("{bar}  {}", "⟲ last time".truecolor(187, 154, 247));
        println!("{bar}    {}", format!("\"{note}\"").truecolor(115, 122, 162));
    }

    // up next
    if !rest.is_empty() {
        println!();
        println!("  {}", "up next".truecolor(86, 95, 137).bold());
        for t in rest.iter().take(4) {
            println!(
                "  {} {}  {}  {}",
                status_glyph(t.status),
                format!("#{}", t.id).truecolor(86, 95, 137),
                t.kind.glyph(),
                truncate(&t.description, 70).truecolor(169, 177, 214),
            );
        }
        if rest.len() > 4 {
            println!("  {}", format!("… and {} more", rest.len() - 4).truecolor(86, 95, 137));
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

pub fn week(tasks: &[Task]) {
    if tasks.is_empty() {
        println!("nothing scheduled for the next 7 days");
        return;
    }
    let today_str = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut current_date = String::new();
    for t in tasks {
        let date_str = t.scheduled_date.as_deref().unwrap_or("");
        if date_str != current_date {
            if !current_date.is_empty() {
                println!();
            }
            if let Ok(d) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                let label = d.format("%a %b %-d").to_string().to_uppercase();
                let header = if date_str == today_str {
                    format!("── {label} · TODAY ──").bold().cyan().to_string()
                } else {
                    format!("── {label} ──").bold().cyan().to_string()
                };
                println!("{header}");
            }
            current_date = date_str.to_owned();
        }
        // metadata chips
        println!(
            "  {}  {}  {}  {}  {}",
            t.kind.glyph(),
            format!("#{}", t.id).truecolor(224, 175, 104),
            t.kind.as_str().truecolor(122, 162, 247),
            format!("◷ {}m", t.session_minutes).truecolor(158, 206, 106),
            readiness(t.status),
        );
        // description indented below
        println!("     {}", truncate(&t.description, 80).truecolor(169, 177, 214));
    }
}

pub fn idea_list(ideas: &[crate::models::Idea]) {
    if ideas.is_empty() {
        println!("  nothing yet — {}", "idea capture \"something\"".truecolor(192, 202, 245));
        return;
    }
    let mut current_status = "";
    for idea in ideas {
        let status = idea.status.as_str();
        if status != current_status {
            if !current_status.is_empty() {
                println!();
            }
            let (glyph, r, g, b): (&str, u8, u8, u8) = match status {
                "inbox"   => ("✎", 125, 207, 255),
                "planned" => ("⊞", 86,  95,  137),
                _         => ("·", 86,  95,  137),
            };
            println!(
                "  {}  {}",
                glyph.truecolor(r, g, b),
                status.to_uppercase().truecolor(86, 95, 137),
            );
            current_status = status;
        }
        println!(
            "    {}  {}",
            format!("#{}", idea.id).truecolor(224, 175, 104),
            idea.title.truecolor(192, 202, 245),
        );
    }
}

fn help_cmd(glyph: &str, r: u8, g: u8, b: u8, name: &str, desc: &str) {
    println!(
        "  {}  {}  {}",
        glyph.truecolor(r, g, b),
        format!("{name:<9}").truecolor(158, 206, 106),
        desc.truecolor(115, 122, 162),
    );
}

pub fn help() {
    let logo = [
        "       ▄█▄",
        "  ▄██▄ ███",
        "  ████ ███",
        "  ████ ▀█▀",
        "  ▀██",
    ];
    let version = env!("CARGO_PKG_VERSION");
    for (i, &line) in logo.iter().enumerate() {
        match i {
            1 => println!(
                "{}   {}  {}",
                line.truecolor(79, 124, 255),
                "cairn".bold().truecolor(192, 202, 245),
                format!("v{version}").truecolor(255, 158, 100),
            ),
            2 => println!(
                "{}   {}",
                line.truecolor(79, 124, 255),
                "session-sized planning for people with no free time".truecolor(86, 95, 137),
            ),
            _ => println!("{}", line.truecolor(79, 124, 255)),
        }
    }
    println!();
    println!("  {}", "USAGE".truecolor(86, 95, 137));
    println!(
        "  {}  {}  {}",
        "cairn".truecolor(192, 202, 245),
        "<command>".truecolor(125, 207, 255),
        "[args]".truecolor(86, 95, 137),
    );
    println!();
    println!("  {}", "PLAN".truecolor(86, 95, 137));
    help_cmd("✎", 125, 207, 255, "capture", "Capture an idea into the inbox — offline, instant");
    help_cmd("⊞", 125, 207, 255, "plan",    "Break an idea into session-sized tasks");
    println!();
    println!("  {}", "TONIGHT".truecolor(86, 95, 137));
    help_cmd("◷", 187, 154, 247, "today", "Tonight's session — one active task, what's next");
    help_cmd("✓", 187, 154, 247, "done",  "Mark a task done");
    help_cmd("▲", 187, 154, 247, "stuck", "Mark stuck; Claude slots a learning task ahead");
    help_cmd("»", 187, 154, 247, "skip",  "Skip today; push open tasks to the next session");
    println!();
    println!("  {}", "LOOK AHEAD".truecolor(86, 95, 137));
    help_cmd("▦", 224, 175, 104, "week", "What's scheduled for the next 7 days");
    help_cmd("≡", 224, 175, 104, "list", "List all ideas — inbox and planned");
    println!();
    println!("  {}", "FIX".truecolor(86, 95, 137));
    help_cmd("⟲", 247, 118, 142, "requeue",   "Reset a stuck task back to queued — no Claude call");
    help_cmd("?",  86,  95,  137, "help",      "Show this message, or help for a subcommand");
    help_cmd("·",  86,  95,  137, "--version", "Print version  (-V)");
    println!();
    println!(
        "  {}  {}  {}",
        "▸ try".truecolor(158, 206, 106),
        "cairn capture".truecolor(192, 202, 245),
        "\"documentary on lisbon gentrification\"".truecolor(115, 122, 162),
    );
}

pub fn plan_table(tasks: &[(i64, &crate::models::TaskDraft)]) {
    for (id, d) in tasks {
        println!(
            "  {}  {}  {}  {}",
            d.kind.glyph(),
            format!("#{id}").truecolor(224, 175, 104),
            format!("◷ {}m", d.session_minutes).truecolor(158, 206, 106),
            truncate(&d.description, 72).truecolor(169, 177, 214),
        );
    }
}

pub fn unblock_result(id: i64, desc: &str, minutes: u32, resource_url: Option<&str>) {
    println!(
        "  {}  {}  {}  {}",
        "📚",
        format!("#{id}").truecolor(224, 175, 104),
        format!("◷ {}m", minutes).truecolor(158, 206, 106),
        truncate(desc, 72).truecolor(169, 177, 214),
    );
    if let Some(url) = resource_url {
        println!(
            "     {}  {}",
            "↗".truecolor(86, 95, 137),
            url.truecolor(115, 122, 162),
        );
    }
}
