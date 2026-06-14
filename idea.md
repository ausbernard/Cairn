# cairn — feature cairns

## Philosophy

One cairn at a time, scoped small enough to finish. The tool isn't a backlog manager — it's a forcing function for completion. When an cairn feels too big, the right move is to scope it down at capture time, not to juggle multiple plans in parallel.

## Quick wins

- `cairn archive <id>` — the `archived` status is already in the model, just no command wired to it
- `cairn focus "new thing"` — update `current_focus` in config from the CLI instead of editing config.toml directly
- `cairn next` — single-line output of just the next task description; plug into tmux status pane or starship/fish prompt (`💡 write the "tell me about yourself" answer`)

## Medium lift

- `cairn start <id>` — countdown timer for `session_minutes`, macOS notification on finish, auto-sets task status to active; closes the loop between "what to do" and actually doing it
- `cairn log` — session history view: date, task, outcome, handoff note; good for weekly review and standup prep
- `cairn status` — one-screen dashboard: inbox count, active plan name, tasks done this week, next scheduled date
- `cairn edit <id>` — edit a task's description or definition-of-done interactively (inquire prompt, pre-filled with current value)

## Bigger swings

- `cairn digest` — feeds the week's session log + upcoming tasks to Claude, returns a markdown weekly review; pairs naturally with `cairn log`
- Shell prompt badge — `cairn prompt` prints a short string (`2 tasks · thu`) for fish/starship/p10k without blocking; cache result to a tempfile so the prompt doesn't hit SQLite on every keypress
- `cairn export` — dump planned cairns + tasks as `.md` files into an Obsidian vault folder; scheduled_date becomes a frontmatter field
