use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdeaStatus {
    Inbox,
    Planned,
    Archived,
}

impl IdeaStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            IdeaStatus::Inbox => "inbox",
            IdeaStatus::Planned => "planned",
            IdeaStatus::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Build,
    Learn,
}

impl TaskKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskKind::Build => "build",
            TaskKind::Learn => "learn",
        }
    }

    pub fn glyph(&self) -> &'static str {
        match self {
            TaskKind::Build => "🔧",
            TaskKind::Learn => "📚",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Active,
    Done,
    Stuck,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Queued => "queued",
            TaskStatus::Active => "active",
            TaskStatus::Done => "done",
            TaskStatus::Stuck => "stuck",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Idea {
    pub id: i64,
    pub title: String,
    pub notes: Option<String>,
    pub status: IdeaStatus,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: i64,
    pub idea_id: i64,
    pub description: String,
    pub definition_of_done: String,
    pub kind: TaskKind,
    pub status: TaskStatus,
    pub session_minutes: u32,
    pub scheduled_date: Option<String>,
    pub sort_order: i64,
}

/// The shape Claude must return when planning an idea.
/// Kept separate from `Task` so the API contract can evolve
/// without touching the database model.
#[derive(Debug, Deserialize)]
pub struct TaskDraft {
    pub description: String,
    pub definition_of_done: String,
    pub kind: TaskKind,
    pub session_minutes: u32,
    #[serde(default)]
    pub resource_url: Option<String>,
}
