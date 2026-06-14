<div align="center">
  <img src="assets/mark.png" width="300" alt="cairn" /><br/><br/>
  <strong>session-sized planning for people with no free time</strong><br/><br/>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="MIT License" /></a>
  <img src="https://img.shields.io/badge/built%20with-Rust-orange.svg" alt="Built with Rust" />
</div>

---

You have a full-time job, maybe a family, and forty-five minutes a night if you're lucky. `cairn` keeps an inbox of everything you want to build or learn, asks Claude to break an idea into tasks that fit a single session, and — when you get stuck — schedules a learning task *ahead* of the blocked work instead of letting you burn two hours going in circles.

One idea at a time. Scoped small enough to finish.

## The loop

```sh
cairn capture "build an MCP server for my homelab"  # inbox it — offline, instant
cairn plan 3                                         # Claude breaks cairn #3 into sessions
cairn today                                          # tonight's task + done-when criteria
cairn done 12                                        # mark it done, leave a handoff note
cairn stuck 12                                       # Claude schedules a learn task ahead of #12
cairn week                                           # see what the next 7 days look like
```

**capture → plan → today → done.** If you hit a wall: stuck → learn → done.

## Install

**Prerequisites**
- [Rust](https://rustup.rs) (stable)
- [Claude Code](https://claude.ai/code) installed and logged in

```sh
npm install -g @anthropic-ai/claude-code   # if you don't have it yet
git clone <repo> && cd Cairn
cargo install --path .
```

No API key required. `cairn` drives Claude through Claude Code's headless mode (`claude -p`), so planning runs on the Claude subscription you already have. As a guardrail, `ANTHROPIC_API_KEY` is stripped from the subprocess environment — it can never silently bill an API account even if a key is set in your shell.

Data lives in a single SQLite file. Config is TOML. Both land in your platform data/config dirs and are created automatically on first run.

## Commands

| Command | Description |
|---|---|
| `cairn capture "title"` | Add to inbox — no network, no Claude, instant |
| `cairn plan <id>` | Claude breaks the idea into session-sized tasks |
| `cairn today` | Tonight's task, done-when checklist, what's up next |
| `cairn done <id>` | Mark done — prompts for a handoff note |
| `cairn stuck <id>` | Mark stuck — Claude inserts a learn task ahead |
| `cairn skip` | Push today's tasks to the next available session |
| `cairn week` | What's scheduled across the next 7 days |
| `cairn list` | All ideas — inbox and planned |
| `cairn requeue <id>` | Reset a stuck task back to queued |

## Config

Written to your platform config dir on first run. Override the data location with `CAIRN_DATA_DIR`.

```toml
max_session_minutes = 45
available_days      = ["mon", "tue", "wed", "thu", "fri"]
tasks_per_day       = 1
model               = "sonnet"

# Passed into every planning prompt
current_focus = ["job searching", "system design interviews"]
strengths     = ["CI/CD", "Kubernetes", "Terraform", "GCP"]
gaps          = ["Rust lifetimes", "GraphQL", "system design"]
```

`tasks_per_day = 2` generates paired **learn → build** tasks so each session has both theory and practice. `model` accepts Claude Code aliases: `sonnet`, `opus`, `haiku`.

## What it enforces

**Every task fits one session.** Plans that come back with tasks over `max_session_minutes` are rejected and re-requested automatically — no manual trimming.

**Every task has a definition of done.** No "work on the frontend" tasks. You know exactly when to stop.

**Stuck means learn, not grind.** `cairn stuck` inserts a tightly-scoped learning task *before* the blocked one — with a resource link — so the next session unblocks you instead of repeating the same wall.

**One idea at a time.** This isn't a backlog manager. When an idea feels too big, scope it down at capture time. Finish before you plan the next one.

## License

MIT
