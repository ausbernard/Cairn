use crate::config;
use crate::models::{Idea, IdeaStatus, Task, TaskDraft, TaskKind, TaskStatus};
use anyhow::{Context, Result};
use rusqlite::{
    params,
    types::{FromSql, FromSqlError, FromSqlResult, ValueRef},
    Connection,
};

impl FromSql for IdeaStatus {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "inbox" => Ok(IdeaStatus::Inbox),
            "planned" => Ok(IdeaStatus::Planned),
            "archived" => Ok(IdeaStatus::Archived),
            s => Err(FromSqlError::Other(format!("unknown idea status: {s:?}").into())),
        }
    }
}

impl FromSql for TaskKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "build" => Ok(TaskKind::Build),
            "learn" => Ok(TaskKind::Learn),
            s => Err(FromSqlError::Other(format!("unknown task kind: {s:?}").into())),
        }
    }
}

impl FromSql for TaskStatus {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "queued" => Ok(TaskStatus::Queued),
            "active" => Ok(TaskStatus::Active),
            "done" => Ok(TaskStatus::Done),
            "stuck" => Ok(TaskStatus::Stuck),
            s => Err(FromSqlError::Other(format!("unknown task status: {s:?}").into())),
        }
    }
}

fn with_transaction<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match f() {
        Ok(v) => {
            conn.execute_batch("COMMIT")?;
            Ok(v)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

pub fn open() -> Result<Connection> {
    let dir = config::data_dir()?;
    std::fs::create_dir_all(&dir)?;
    let conn = Connection::open(dir.join("cairn.db"))?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS ideas (
            id          INTEGER PRIMARY KEY,
            title       TEXT NOT NULL,
            notes       TEXT,
            status      TEXT NOT NULL DEFAULT 'inbox',
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS tasks (
            id                  INTEGER PRIMARY KEY,
            idea_id             INTEGER NOT NULL REFERENCES ideas(id),
            description         TEXT NOT NULL,
            definition_of_done  TEXT NOT NULL DEFAULT '',
            kind                TEXT NOT NULL DEFAULT 'build',
            status              TEXT NOT NULL DEFAULT 'queued',
            session_minutes     INTEGER NOT NULL DEFAULT 45,
            scheduled_date      TEXT,
            sort_order          INTEGER NOT NULL DEFAULT 0,
            created_at          TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS sessions (
            id          INTEGER PRIMARY KEY,
            task_id     INTEGER NOT NULL REFERENCES tasks(id),
            started_at  TEXT NOT NULL DEFAULT (datetime('now')),
            ended_at    TEXT,
            outcome     TEXT
        );
        ",
    )?;
    // Guarded migration: add notes column if this is an existing database.
    if let Err(e) = conn.execute_batch("ALTER TABLE sessions ADD COLUMN notes TEXT;") {
        if !e.to_string().contains("duplicate column name") {
            return Err(e.into());
        }
    }
    Ok(conn)
}

pub fn capture_idea(conn: &Connection, title: &str, notes: Option<&str>) -> Result<i64> {
    conn.execute(
        "INSERT INTO ideas (title, notes) VALUES (?1, ?2)",
        params![title, notes],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn inbox_count(conn: &Connection) -> Result<i64> {
    let n = conn.query_row(
        "SELECT COUNT(*) FROM ideas WHERE status = 'inbox'",
        [],
        |r| r.get(0),
    )?;
    Ok(n)
}

pub fn list_ideas(conn: &Connection) -> Result<Vec<Idea>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, notes, status, created_at FROM ideas
         WHERE status != 'archived'
         ORDER BY CASE status WHEN 'inbox' THEN 0 WHEN 'planned' THEN 1 ELSE 2 END, id ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Idea {
            id: r.get(0)?,
            title: r.get(1)?,
            notes: r.get(2)?,
            status: r.get(3)?,
            created_at: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_idea(conn: &Connection, id: i64) -> Result<Idea> {
    conn.query_row(
        "SELECT id, title, notes, status, created_at FROM ideas WHERE id = ?1",
        params![id],
        |r| {
            Ok(Idea {
                id: r.get(0)?,
                title: r.get(1)?,
                notes: r.get(2)?,
                status: r.get(3)?,
                created_at: r.get(4)?,
            })
        },
    )
    .with_context(|| format!("no idea with id #{id}"))
}

pub fn set_idea_status(conn: &Connection, id: i64, status: IdeaStatus) -> Result<()> {
    conn.execute(
        "UPDATE ideas SET status = ?1 WHERE id = ?2",
        params![status.as_str(), id],
    )?;
    Ok(())
}

pub fn insert_drafts(
    conn: &Connection,
    idea_id: i64,
    drafts: &[TaskDraft],
    dates: &[chrono::NaiveDate],
) -> Result<Vec<i64>> {
    let base: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) FROM tasks",
        [],
        |r| r.get(0),
    )?;
    with_transaction(conn, || {
        drafts
            .iter()
            .enumerate()
            .map(|(i, d)| -> Result<i64> {
                let date = dates.get(i).map(|d| d.format("%Y-%m-%d").to_string());
                conn.execute(
                    "INSERT INTO tasks (idea_id, description, definition_of_done, kind, session_minutes, sort_order, scheduled_date)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        idea_id,
                        d.description,
                        d.definition_of_done,
                        d.kind.as_str(),
                        d.session_minutes,
                        base + 1 + i as i64,
                        date,
                    ],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .collect()
    })
}

/// Inserts a learning task *ahead* of a blocked task. Shifts every task
/// at or after the blocked sort_order up by one, then inserts at the
/// vacated slot — safe to call multiple times on the same blocked task.
pub fn insert_before(conn: &Connection, blocked: &Task, draft: &TaskDraft) -> Result<i64> {
    with_transaction(conn, || {
        conn.execute(
            "UPDATE tasks SET sort_order = sort_order + 1 WHERE sort_order >= ?1",
            params![blocked.sort_order],
        )?;
        conn.execute(
            "INSERT INTO tasks (idea_id, description, definition_of_done, kind, session_minutes, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                blocked.idea_id,
                draft.description,
                draft.definition_of_done,
                draft.kind.as_str(),
                draft.session_minutes,
                blocked.sort_order,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    })
}

pub fn get_task(conn: &Connection, id: i64) -> Result<Task> {
    conn.query_row(
        "SELECT id, idea_id, description, definition_of_done, kind, status,
                session_minutes, scheduled_date, sort_order
         FROM tasks WHERE id = ?1",
        params![id],
        row_to_task,
    )
    .with_context(|| format!("no task with id #{id}"))
}

pub fn open_tasks(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, idea_id, description, definition_of_done, kind, status,
                session_minutes, scheduled_date, sort_order
         FROM tasks
         WHERE status IN ('queued', 'active', 'stuck')
           AND (scheduled_date IS NULL OR scheduled_date <= date('now'))
         ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([], row_to_task)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn week_tasks(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, idea_id, description, definition_of_done, kind, status,
                session_minutes, scheduled_date, sort_order
         FROM tasks
         WHERE status IN ('queued', 'active', 'stuck')
           AND scheduled_date >= date('now')
           AND scheduled_date <= date('now', '+6 days')
         ORDER BY scheduled_date ASC, sort_order ASC",
    )?;
    let rows = stmt.query_map([], row_to_task)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn skip_today(conn: &Connection, next_date: &str) -> Result<usize> {
    let n = conn.execute(
        "UPDATE tasks SET scheduled_date = ?1
         WHERE status IN ('queued', 'active', 'stuck')
           AND (scheduled_date IS NULL OR scheduled_date <= date('now'))",
        params![next_date],
    )?;
    Ok(n)
}

pub fn record_session(conn: &Connection, task_id: i64, outcome: &str, notes: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (task_id, started_at, ended_at, outcome, notes)
         VALUES (?1, datetime('now'), datetime('now'), ?2, ?3)",
        params![task_id, outcome, notes],
    )?;
    Ok(())
}

pub fn latest_handoff(conn: &Connection) -> Result<Option<String>> {
    match conn.query_row(
        "SELECT notes FROM sessions
         WHERE notes IS NOT NULL AND notes != ''
         ORDER BY id DESC LIMIT 1",
        [],
        |r| r.get::<_, String>(0),
    ) {
        Ok(note) => Ok(Some(note)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set_task_status(conn: &Connection, id: i64, status: TaskStatus) -> Result<()> {
    let changed = conn.execute(
        "UPDATE tasks SET status = ?1 WHERE id = ?2",
        params![status.as_str(), id],
    )?;
    anyhow::ensure!(changed == 1, "no task with id #{id}");
    Ok(())
}

fn row_to_task(r: &rusqlite::Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: r.get(0)?,
        idea_id: r.get(1)?,
        description: r.get(2)?,
        definition_of_done: r.get(3)?,
        kind: r.get(4)?,
        status: r.get(5)?,
        session_minutes: r.get(6)?,
        scheduled_date: r.get(7)?,
        sort_order: r.get(8)?,
    })
}
