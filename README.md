# Almahrya

A personal workbench for an engineer's working day. It gathers notifications from Lark and GitLab, runs a Claude
agent on your own Mac against your own repositories, skills and tools, and answers teammates through a Lark bot.
You stay in control through a few clear gates: pushing code, sending messages in your name, and approving what
teammates ask for.

Status: the foundation is built. The daemon (`almahryad`) serves the `/v1` API on a Unix socket and on localhost
with token auth, keeps sessions and an event log in SQLite and replays missed events; the desktop app starts or
finds the daemon and lists chat sessions live; a development profile runs beside production. Nothing of the
workbench itself exists yet: no agent sessions, Inbox, Lark bot, terminal, read path or Settings.

## What it does

- **Dashboard**: what is waiting for you, your tickets on one track, the agents that are running.
- **Inbox**: Lark mentions and messages, GitLab merge requests and review comments, in one place.
- **Ticket sessions**: one session per ticket for its whole life, with live progress, decision cards instead of
  long questions, a single push gate per run, and notes the agent never forgets.
- **Source control and diff**: stage, discard and commit per file or per hunk, with actions on right-click.
- **Terminal**: tabs and split panes; send output to a session.
- **Lark bot**: answers questions from allowed people in allowed groups; teammates' action requests become
  approvals in your Inbox.
- **Settings**: Claude Code-like skills and connectors you can switch on and off, subagents, budgets, and a
  read-only data path so the agent never touches secret files.

## How it is built

One Rust backend (`almahryad`) with one HTTP + WebSocket API. The desktop app (Rust, GPUI) is a client of that
API, and a React web app will be another. Claude runs through the official Claude Agent SDK in a small
TypeScript sidecar; the backend makes every permission decision.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the system, [DESIGN.md](DESIGN.md) for the interface, and
[docs/superpowers/specs/2026-11-14-almahrya-design.md](docs/superpowers/specs/2026-11-14-almahrya-design.md) for
the full design.

## Layout

| Path | Responsibility |
|---|---|
| `crates/core` | Domain model and pure decisions; no IO |
| `crates/api-types` | API request, response and event types (TypeScript generated from here) |
| `crates/local` | Paths and token storage shared by the daemon and the desktop app |
| `crates/store` | SQLite |
| `crates/connectors` | Lark and GitLab |
| `crates/runtimes` | Agent runtimes behind one trait (Claude first) |
| `crates/readpath` | Read-only data service, exposed to the agent as MCP |
| `crates/engine` | The running system: sessions, runs, gates, bot, git, files, terminals |
| `crates/api` | HTTP and WebSocket server |
| `crates/daemon` | The `almahryad` binary |
| `crates/client` | Rust client for the API, used by the desktop app |
| `crates/ui-kit` | App components wrapped around GPUI |
| `crates/app` | Desktop binary |
| `sidecars/claude` | Claude Agent SDK sidecar (TypeScript) |

`crates/readpath` and `sidecars/claude` do not exist yet; `connectors` and `runtimes` are empty placeholders.

## Build and run

Requirements today: macOS 14 or later and Rust (edition 2024). `scripts/dev.sh` also needs
[watchexec](https://github.com/watchexec/watchexec) (`brew install watchexec`). Zig (for the terminal library),
Node.js (for the sidecar) and Claude, Lark and GitLab accounts are needed only by later plans.

| Command | What it does |
|---|---|
| `scripts/check.sh` | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace` |
| `scripts/dev.sh` | The development profile with rebuild on save: restarts `almahryad` and the desktop app when their crates change |
| `cargo run -p almahrya-daemon` | Runs `almahryad` in the foreground (production profile) |
| `cargo run -p almahrya-daemon -- install-agent` | Writes the launchd agent plist for `almahryad` and prints the command that loads it |
| `cargo run -p almahrya-app` | The desktop app; when no daemon answers it starts the `almahryad` next to its own executable, so run `cargo build -p almahrya-daemon` first |

## Your workflow stays yours

Ticket key formats, board columns, which skill runs at which moment, folder layouts and data sources are all
configuration in `~/.config/almahrya/config.toml`. The repository contains no workflow of any specific team.

## Contributing

Read [AGENTS.md](AGENTS.md) before changing anything; it applies to people and coding agents alike.
