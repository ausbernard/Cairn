use crate::config::Config;
use crate::models::{Idea, Task, TaskDraft};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::process::Command;

/// Calls the locally installed Claude Code CLI in headless mode (`claude -p`).
/// Uses your Claude subscription — no API key, no per-token billing.
///
/// ANTHROPIC_API_KEY is deliberately stripped from the child environment:
/// if it leaks in, Claude Code silently switches to API billing.
fn call(cfg: &Config, system: &str, user: &str) -> Result<String> {
    let output = Command::new("claude")
        .arg("-p")
        .arg(user)
        .arg("--append-system-prompt")
        .arg(system)
        .arg("--model")
        .arg(&cfg.model)
        .arg("--output-format")
        .arg("json")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .context(
            "could not run `claude` — is Claude Code installed and on your PATH? \
             (npm install -g @anthropic-ai/claude-code, then `claude` once to log in)",
        )?;

    if !output.status.success() {
        bail!(
            "claude -p failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let envelope: Value = serde_json::from_slice(&output.stdout)
        .context("claude -p returned non-JSON output")?;
    let text = envelope["result"]
        .as_str()
        .context("no `result` field in claude -p output")?;
    Ok(text.to_string())
}

/// Strips markdown fences if the model wrapped its JSON despite instructions.
fn extract_json(raw: &str) -> &str {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .unwrap_or(trimmed);
    inner.trim()
}

fn parse_drafts(raw: &str, max_minutes: u32) -> Result<Vec<TaskDraft>> {
    let drafts: Vec<TaskDraft> =
        serde_json::from_str(extract_json(raw)).context("response was not valid task JSON")?;
    if drafts.is_empty() {
        bail!("model returned an empty task list");
    }
    for d in &drafts {
        if d.session_minutes > max_minutes {
            bail!(
                "task '{}' is {}m, over the {}m session cap",
                d.description,
                d.session_minutes,
                max_minutes
            );
        }
    }
    Ok(drafts)
}

fn per_task_cap(cfg: &Config) -> u32 {
    cfg.max_session_minutes / cfg.tasks_per_day.max(1)
}

fn system_prompt(cfg: &Config) -> String {
    let focus_line = if cfg.current_focus.is_empty() {
        String::new()
    } else {
        format!(" Current focus areas: {}.", cfg.current_focus.join(", "))
    };
    format!(
        "You are a planning assistant for a busy engineer with very limited time. \
         Their strengths: {strengths}. Known gaps: {gaps}.{focus_line} \
         Every task you produce MUST be completable in a single focused session of at most \
         {max} minutes and MUST have a concrete, verifiable definition of done. \
         Do not use any tools. Respond with ONLY a JSON array, no prose, no markdown fences. \
         Each element: {{\"description\": string, \"definition_of_done\": string, \
         \"kind\": \"build\"|\"learn\", \"session_minutes\": number}}",
        strengths = cfg.strengths.join(", "),
        gaps = cfg.gaps.join(", "),
        max = per_task_cap(cfg),
    )
}

/// Asks for plan JSON; on a parse/validation failure, retries once with the
/// error message included so the model can correct itself.
fn plan_with_retry(cfg: &Config, user_prompt: &str) -> Result<Vec<TaskDraft>> {
    let cap = per_task_cap(cfg);
    let system = system_prompt(cfg);
    let first = call(cfg, &system, user_prompt)?;
    match parse_drafts(&first, cap) {
        Ok(drafts) => Ok(drafts),
        Err(e) => {
            let retry_prompt = format!(
                "{user_prompt}\n\nYour previous response failed validation: {e}. \
                 Respond again with ONLY the corrected JSON array."
            );
            let second = call(cfg, &system, &retry_prompt)?;
            parse_drafts(&second, cap)
        }
    }
}

pub fn plan_idea(cfg: &Config, idea: &Idea) -> Result<Vec<TaskDraft>> {
    let notes = idea.notes.as_deref().unwrap_or("(no notes)");
    let pair_instruction = if cfg.tasks_per_day > 1 {
        format!(
            "Tasks MUST come in strict pairs: a 'learn' task (watch/read/study one concept) \
             followed immediately by a 'build' task that applies exactly what was just learned. \
             Generate an even number of tasks. Never group all learns or all builds together. \
             Each task must fit in {cap}m — two tasks share one session day.",
            cap = per_task_cap(cfg),
        )
    } else {
        String::new()
    };
    let prompt = format!(
        "Break this idea into ordered, session-sized tasks. {pair_instruction}\
         Front-load anything that de-risks the idea. If the idea touches my gaps, \
         insert 'learn' tasks before the build tasks that need them.\n\n\
         Idea: {title}\nNotes: {notes}",
        title = idea.title,
    );
    plan_with_retry(cfg, &prompt)
}

pub fn unblock_task(cfg: &Config, task: &Task, blocker: Option<&str>) -> Result<TaskDraft> {
    let blocker_line = blocker
        .filter(|s| !s.is_empty())
        .map(|s| format!(" The user says the blocker is: \"{s}\"."))
        .unwrap_or_default();
    let prompt = format!(
        "I am stuck on this task: \"{desc}\" (definition of done: {dod}).{blocker_line} \
         Produce exactly ONE 'learn' task — a JSON array with a single element — \
         that teaches me the most likely missing concept so I can unblock it. \
         Keep it tightly scoped to the blocker, not general education. \
         Also include a `resource_url` field with one authoritative link \
         (official docs, a specific article, or a well-known tutorial) for the concept.",
        desc = task.description,
        dod = task.definition_of_done,
    );
    let mut drafts = plan_with_retry(cfg, &prompt)?;
    Ok(drafts.remove(0))
}
