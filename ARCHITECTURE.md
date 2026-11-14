# Architecture

Almahrya is one Rust backend (a daemon) with one API, and thin clients on top of it. The desktop app (GPUI) is
the first client; a React web app is the second, later. The design document is
`docs/superpowers/specs/2026-11-14-almahrya-design.md`.

```
                 almahryad  (Rust daemon, launchd user agent)
 ┌──────────────────────────────────────────────────────────────────────┐
 │ api        HTTP JSON + WebSocket, /v1, Unix socket + 127.0.0.1       │
 │ engine     sessions, runs, gates, Lark bot, inbox, git, files, PTYs  │
 │ core       domain types and decisions (pure, no IO)                  │
 │ store      SQLite                                                    │
 │ connectors Lark, GitLab                                              │
 │ runtimes   Claude sidecar (TypeScript, Agent SDK) over JSON Lines    │
 │ readpath   read-only data service, exposed to the agent as MCP       │
 └──────────────────────────────────────────────────────────────────────┘
          ▲ token auth                          ▲ token auth
   Desktop app (GPUI)                    Web app (React, later)
   app + ui-kit + client                 generated TypeScript types
```

## Why a daemon

- The Lark bot, running agents and terminals must keep going when the window is closed or the UI crashes.
- The web app must call exactly the endpoints the desktop already uses, so the desktop goes through the API
  too. There is one code path for every action.

## Processes

| Process | Language | Role |
|---|---|---|
| `almahryad` | Rust | The backend. Started by launchd at login. Owns all state and all side effects. |
| `Almahrya.app` | Rust (GPUI) | Desktop client and menu bar item. Starts the daemon if it is not running. |
| Claude sidecar | TypeScript | One per active agent session. Wraps the Claude Agent SDK. Started and supervised by the daemon. |
| Web app | React | Later. Same API. Served by the daemon or a dev server. |

## Crates

| Crate | Depends on | Responsibility |
|---|---|---|
| `core` | none | Domain model and pure decisions: permission classification, gate rules, ticket and run state machines, Lark access rule. No IO, no async runtime. Fully unit-tested. |
| `api-types` | `core` | Every request, response and event type of the API, with serde. TypeScript types are generated from this crate. |
| `store` | `core` | SQLite access and migrations. |
| `connectors` | `core` | Lark (long connection, messages, Base, contacts) and GitLab (merge requests, discussions) clients. |
| `runtimes` | `core` | Agent runtime trait; Claude implementation that starts and talks to the sidecar. Codex and ACP later. |
| `readpath` | `core` | Read-only query service for Postgres, ClickHouse, Elasticsearch, Redis. Reads credentials from the configured `.env`. Exposed to the agent as an MCP server. |
| `engine` | all of the above | The running system: sessions, runs, gates, inbox, bot, git, files, PTYs, temporary grants, audit. Own tokio runtime. |
| `api` | `engine`, `api-types` | HTTP and WebSocket server. Thin: validates, calls the engine, streams events. |
| `daemon` | `api`, `engine` | The `almahryad` binary: config, logging, launchd integration, single-instance lock. |
| `client` | `api-types` | Rust client for the API (HTTP + WebSocket), used by the desktop app. |
| `ui-kit` | GPUI | App components wrapped around GPUI and gpui-component. The only crate that depends on GPUI. Includes a component gallery used to compare before and after an upgrade. |
| `app` | `client`, `ui-kit` | The desktop binary. No business logic and no direct GPUI imports: it renders API state with `ui-kit` components and sends commands. |

Rules:
- `core` and `api-types` never depend on GPUI, tokio-specific IO, or network crates.
- Only `ui-kit` depends on GPUI. A CI check fails if any other crate lists `gpui`, `gpui-pre` or `gpui-component`.
- Only `engine` performs side effects. `api` and `app` never touch git, files, the database or the sidecar directly.
- Every external dependency is declared once in the workspace `Cargo.toml` with an exact version.

## API

- Transport: HTTP/1.1 JSON for commands and queries; one WebSocket per client for events and terminal bytes.
- Listening:
  - a Unix socket in the data directory, used by the desktop;
  - `127.0.0.1` on a configured port, used by the web app during development;
  - nothing on other interfaces. Remote access is out of scope for now.
- Auth: a random token created at first start and stored in the Keychain. Every request carries it. The web app
  gets it through a local login page served by the daemon.
- Versioning: everything under `/v1`. Breaking changes go to `/v2`; the daemon serves both for one release.
- Style:
  - resources are nouns (`/v1/sessions`, `/v1/sessions/{id}/runs`, `/v1/inbox`, `/v1/gates/{id}`);
  - actions are explicit sub-paths (`POST /v1/gates/{id}/approve`, `POST /v1/decisions/{id}/answer`);
  - lists are paginated with cursors;
  - errors are one JSON shape with a stable code.
- Events: the WebSocket sends typed events from `api-types`, each with a monotonically increasing sequence number.
  A client that reconnects sends its last sequence number and receives what it missed (events are kept in SQLite
  for a window). Example kinds:
  - `session.updated`, `run.stage_changed`, `decision.requested`, `gate.opened`
  - `subagent.progress`, `inbox.item_added`, `approval.requested`, `grant.changed`
  - `terminal.output`
- Type generation: TypeScript definitions are generated from `api-types` at build time into `web/src/api/`
  (when the web app exists). The desktop uses the Rust types directly through `client`.

## Agent runtime (Claude)

- The daemon starts one sidecar per active session and speaks bounded JSON Lines over stdin and stdout.
- The sidecar:
  - translates SDK messages into the app's event shape;
  - forwards every `canUseTool` request, question and permission request to the daemon;
  - waits for the answer.
- It holds no policy.
- The daemon:
  - validates every event;
  - classifies each tool call with `core` (always allowed, needs the owner, never);
  - asks the owner through a decision card or gate when needed;
  - answers the sidecar.
- Subagent tool calls go through the same flow. Subagent messages are identified by `parent_tool_use_id`
  (and `agent_id` in TypeScript); spawns are `Agent` tool calls (`Task` in older versions).
- The main session never runs in `bypassPermissions`. Subagent permission modes may be stricter, never looser.
- Skills: the daemon builds a per-session skills folder containing only the skills the owner checked, passes it to
  the SDK, and denies the Skill tool for anything else.
- Connectors: enabled MCP servers and plugins are passed per session. Tool search stays on (first-party API base
  URL). Servers needed from turn one are marked `alwaysLoad`.
- Run state for ticket work lives in the ticket folder as a run state file plus a readable run log. A run survives
  compaction, restarts and multi-day gaps, and the session view is rebuilt from it.
- After compaction, the ticket notes file is re-injected so scope and facts are not lost.

## Permissions and grants

- `core` holds the rules as data and pure functions. The engine enforces them before any tool call is answered.
- Hard denies (never grantable):
  - writes to production data;
  - reading `.env` files, credentials, `~/.ssh`, `~/.aws`, the Keychain;
  - raw `psql`, `redis-cli`, `kubectl`, `podman`;
  - bypass mode.
- Temporary grants:
  - scoped to one session and one capability (for example "write to staging ClickHouse");
  - expire when the stage ends;
  - every write is journaled with its undo;
  - "Clean up now" replays the journal;
  - grants are stored in SQLite and shown in Security and in the session.
- Every decision (allowed, asked, denied, granted) is written to the audit log and appears in the Activity tab.

## Read path

- An MCP server provided by the daemon to each agent session, with read-only tools per source and environment
  (for example `query(source, environment, sql)`).
- Credentials are read from the user-configured `.env` file by the daemon only. The agent never sees them and
  cannot read the file.
- Statements are parsed and checked as read-only before they run; results are size-limited; configured sensitive
  columns are masked; every query is logged with its environment.
- The owner's own read-only wrapper scripts can also be registered as sources.

## Lark bot

- Long connection (WebSocket event subscription), so no public endpoint is needed.
- Flow:
  1. A mention arrives.
  2. The access rule in `core` runs before any model call: allowed person AND allowed group, or a DM from an
     allowed person.
  3. The bot sends the emoji acknowledgement.
  4. It classifies the request as a question or an action.
  5. A question is answered read-only. An action runs if the owner asked; if a teammate asked, it becomes an
     Inbox approval.
- Thread maps to session. Context is built in layers and treated as data, never as instructions.
- When the Mac was asleep, missed mentions are fetched on wake and answered with a note.

## Git, files, documents

- The engine runs git for status, diff, stage, discard, commit, push, and worktree management. Worktrees live under
  a configured root. Each ticket session knows its worktrees.
- File access through the API is limited to the session's worktrees and its ticket documents folder.
- Ticket documents are committed and pushed automatically when a stage finishes; code pushes go through the gate.

## Terminal

- The daemon owns PTYs (portable-pty), one per terminal tab, and streams raw output as `terminal.output` events.
  Input goes back over the same WebSocket.
- Clients parse and render:
  - desktop: libghostty-vt on its own thread, cells drawn in GPUI (fallback alacritty_terminal);
  - web: xterm.js.
- Because the daemon owns the PTY, a terminal keeps running when the window closes and can be reopened from any
  client.

## Storage and files

| What | Where |
|---|---|
| Config (hand-editable) | `~/.config/almahrya/config.toml` |
| Database, logs, cache, Unix socket | `~/Library/Application Support/Almahrya/` |
| API token, Lark and GitLab tokens | macOS Keychain |
| Database credentials for the read path | the user-chosen `.env` path, read only by the daemon |
| Ticket run state, notes, documents | the ticket folder in the user's documents repository |

Workflow-specific settings (ticket key prefix, board columns, which skill each moment runs, folder layout) are
configuration in `config.toml`. They never appear in code or tests.

## Testing

- `core`: unit tests for every rule (permission classes, gate conditions, access rule, state machines).
- `engine`, `api`, `readpath`: integration tests in each crate's `tests/` with a fake sidecar and a fake Lark/GitLab.
- `api`: contract tests that the generated TypeScript types match the Rust types.
- The sidecar: tests against a recorded SDK message stream.

## Upgrading GPUI

GPUI is pre-1.0 and changes without notice, so upgrades are planned, small and checked.

- Where it comes from: gpui-component takes GPUI from crates.io as `gpui-pre`, so both are pinned as exact crate
  versions in the root `Cargo.toml` (the versions that built in `docs/superpowers/spikes/gpui.md`). Do not add a git
  dependency on the Zed repository next to them; the types would not match.
- Blast radius: only `ui-kit` imports GPUI. An upgrade edits `ui-kit` and, if a component signature changes,
  the call sites in `app`.
- Procedure:
  1. Create a branch `chore/gpui-<version>`.
  2. Bump the pinned versions together (`gpui-pre`, `gpui-pre-platform`, `gpui-component`).
  3. Fix compile errors inside `ui-kit` only.
  4. Run `scripts/check.sh`, then the component gallery (`cargo run -p almahrya-ui-kit --example gallery`) and
     save its screenshots next to the previous ones in a pull request for comparison.
  5. Merge only when tests pass and the gallery looks the same or better.
- Fonts are embedded in the app, not read from the user's font folder.
- The window surface is tagged Display P3; saturated colours are checked in the gallery.
