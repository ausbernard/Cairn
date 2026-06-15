use anyhow::{Context, Result};
use chrono::Datelike;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Hard cap on how long a single task is allowed to take, in minutes.
    pub max_session_minutes: u32,
    /// Days of the week you actually get a session. Lowercase three-letter.
    pub available_days: Vec<String>,
    /// Fed to Claude so plans build on what you know.
    pub strengths: Vec<String>,
    /// Fed to Claude so it schedules learning tasks where you're thin.
    pub gaps: Vec<String>,
    /// What you're actively focused on right now — biases plans toward these areas.
    pub current_focus: Vec<String>,
    /// How many tasks to schedule per available day (default 1). Set to 2
    /// for paired learn+build sessions that share the same day.
    pub tasks_per_day: u32,
    /// Anthropic model used for planning calls.
    pub model: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            max_session_minutes: 45,
            available_days: vec!["mon", "tue", "wed", "thu", "fri", "sun"]
                .into_iter()
                .map(String::from)
                .collect(),
            strengths: vec![],
            gaps: vec![],
            current_focus: vec![],
            tasks_per_day: 1,
            model: "sonnet".to_string(),
        }
    }
}

pub fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("dev", "ausby", "cairn").context("could not resolve home directory")
}

pub fn data_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("CAIRN_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    Ok(project_dirs()?.data_dir().to_path_buf())
}

pub fn config_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("config.toml"))
}

impl Config {
    /// Returns `n` scheduled dates spread across available days, repeating
    /// each day `tasks_per_day` times so paired tasks land on the same date.
    pub fn schedule_dates(&self, n: usize) -> Vec<chrono::NaiveDate> {
        let per_day = self.tasks_per_day.max(1) as usize;
        let today = chrono::Local::now().date_naive();
        if self.available_days.is_empty() {
            return (0..n)
                .map(|i| today + chrono::TimeDelta::days((i / per_day) as i64))
                .collect();
        }
        let mut dates = Vec::with_capacity(n);
        let mut candidate = today;
        while dates.len() < n {
            let abbr = match candidate.weekday() {
                chrono::Weekday::Mon => "mon",
                chrono::Weekday::Tue => "tue",
                chrono::Weekday::Wed => "wed",
                chrono::Weekday::Thu => "thu",
                chrono::Weekday::Fri => "fri",
                chrono::Weekday::Sat => "sat",
                chrono::Weekday::Sun => "sun",
            };
            if self.available_days.iter().any(|d| d == abbr) {
                for _ in 0..per_day {
                    if dates.len() < n {
                        dates.push(candidate);
                    }
                }
            }
            candidate += chrono::TimeDelta::days(1);
        }
        dates
    }

    /// Returns the next calendar day (starting tomorrow) that matches one of
    /// the user's `available_days`. Scans up to 7 days so a fully-blocked
    /// week returns `None` rather than looping forever.
    pub fn next_available_day(&self) -> Option<chrono::NaiveDate> {
        let today = chrono::Local::now().date_naive();
        for i in 1..=7i64 {
            let date = today + chrono::TimeDelta::days(i);
            let abbr = match date.weekday() {
                chrono::Weekday::Mon => "mon",
                chrono::Weekday::Tue => "tue",
                chrono::Weekday::Wed => "wed",
                chrono::Weekday::Thu => "thu",
                chrono::Weekday::Fri => "fri",
                chrono::Weekday::Sat => "sat",
                chrono::Weekday::Sun => "sun",
            };
            if self.available_days.iter().any(|d| d == abbr) {
                return Some(date);
            }
        }
        None
    }
}

/// Loads config, writing a commented default on first run so the file
/// is discoverable and editable without reading docs.
pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        let dir = path.parent().expect("config path has parent");
        fs::create_dir_all(dir)?;
        let default = Config::default();
        let body = toml::to_string_pretty(&default)?;
        fs::write(&path, body)?;
        return Ok(default);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let cfg: Config = toml::from_str(&raw)
        .with_context(|| format!("parsing {}", path.display()))?;
    Ok(cfg)
}
