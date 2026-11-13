# Almahrya

A personal desktop app for the working day: it collects notifications from Lark and
GitLab, forwards chosen ones to a Claude agent on this machine, and runs a Lark bot.

Status: workspace skeleton only. Nothing runs yet.

## Layout

| Path | Responsibility |
|---|---|
| `crates/core` | Domain model and decisions; pure, no IO |
| `crates/store` | Operational state in SQLite |
| `crates/connectors` | Notification sources (Lark, GitLab) |
| `crates/runtimes` | Agent runtimes behind one trait (Claude first) |
| `crates/engine` | The running system: sources, rules, agent runs, Lark bot |
| `crates/ui-kit` | App components wrapped around GPUI |
| `crates/app` | Desktop binary |
| `sidecars/claude` | Claude Agent SDK sidecar (TypeScript) |

Integration tests live in each crate's `tests/` directory.
