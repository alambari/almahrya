# Almahrya design

Date: 2026-11-14. Status: approved direction, written from the brainstorm. Two areas are parked (see "Parked").

## 1. Intent

Almahrya is a personal workbench for one engineer's working day. It gathers the engineer's notifications
(Lark and GitLab), runs a Claude agent on the engineer's own Mac against their own repositories, skills and
tools, and answers teammates through a Lark bot. The engineer stays in control through a small number of
clear gates instead of babysitting a terminal.

Why it exists: today the owner glues the day together by hand. Mentions arrive in Lark, merge request threads
in GitLab, tickets move on a Lark Base board, and the real work happens in Claude Code sessions in a terminal.
Two real sessions were studied while designing this (a multi-repo implementation run and a review-feedback
rework across three related tickets). They showed the same pains:

- Status lives only in chat. The owner kept asking "what is left?" and could not see what the agent waited on.
- Context is lost when the conversation is compacted; scope decisions and architecture facts had to be re-taught.
- Questions from the agent arrive as paragraphs and are answered with one word.
- Subagents run for 15 to 35 minutes with no visible progress.
- Safety exceptions are granted in chat ("for this session I allow it") and nobody records or cleans them up.
- The agent reads secret files to reach databases because there is no proper read path.
- Opening the editor, the worktrees, diagrams and merge request links is manual glue.

Almahrya fixes these with persistent run state, visible progress, decision cards, a single push gate,
a read path for data, recorded temporary grants, and one place for the day's inputs.

The repository is public and the app must be usable by other people. Nothing in the code, docs or tests may
name the owner's company, team, workflow skills or ticket codes. Everything specific to the owner's workflow
is configuration that lives only on the owner's machine.

## 2. Goals, non-goals, anti-goals

Goals
- One app for the day: Dashboard, Inbox (Lark, GitLab), ticket sessions, source control with diff, documents,
  terminal, settings.
- Run the owner's own Claude Code skills and connectors through the Claude Agent SDK, with Claude Code-like
  behaviour: every enabled skill and connector is available; the agent suggests skills; `/name` forces one.
- Many runs at once (the owner runs 10 to 20 in parallel). No artificial concurrency cap.
- Gates the owner trusts: push and merge requests, messages sent in the owner's name, teammate requests,
  writes outside the ticket's working copy.
- A Lark bot that answers questions and turns teammates' action requests into Inbox approvals.
- One Rust backend with one API, used by the desktop app now and by a React web app later.

Non-goals (now)
- A general code editor ("Project" sidebar group was postponed).
- An always-on cloud relay for the bot (when the Mac sleeps, missed mentions are answered on wake).
- Multi-user hosting. The backend serves one person on one Mac.
- Runtimes other than Claude (Codex and ACP adapters come later behind the same runtime trait).
- Sources other than Lark and GitLab.

Anti-goals (would be a failure even if it works)
- The agent writes to production data. Never, and it cannot be enabled.
- The agent reads secret files or credentials.
- Rigid, invented restrictions on what the agent may load (an early "load set per session type" idea was
  rejected for this reason).
- A settings area so dense the owner cannot tell what matters.
- Anything workflow-specific hard-coded in the public repository.

## 3. Constraints

- macOS first. Desktop UI in Rust with GPUI (plus gpui-component), pinned to exact crate versions (GPUI is published on crates.io as `gpui-pre`) and wrapped in `ui-kit`, the only crate allowed to import it.
- Claude through the official Claude Agent SDK in a TypeScript sidecar, pinned. The Rust side validates every
  event and owns every permission decision; the sidecar owns no policy.
- Agent SDK tool search must stay on (first-party API base URL, no proxy). Connectors needed from the first turn
  are marked `alwaysLoad`.
- Never run the main agent session in `bypassPermissions`; subagents inherit permission modes.
- Files: config in `~/.config/almahrya/config.toml` (hand-editable), data in
  `~/Library/Application Support/Almahrya/` (SQLite, logs, cache), secrets in the macOS Keychain. The database
  credential file used by the read path is a user-chosen `.env` path that only the backend reads.
- The public repository holds no secrets, no work data, no Lark or GitLab identifiers, and no workflow names.
- Owner's coding rule for later: no co-author trailers in commits or merge requests.

## 4. Approaches considered for the backend

The owner wants one Rust backend and, later, a React web app that "just calls the Rust endpoints".

A. Backend as its own process (daemon) with one HTTP + WebSocket API; the desktop app is a client of that API.
B. One process: engine inside the GPUI app, plus an optional API module for the web.
C. Desktop UI in a webview with React from the start.

Picked: A. Both of the owner's needs (keep running when the window is closed; web that calls the same
endpoints) are met by the same shape. B gives two code paths that drift and stops the bot when the app quits.
C throws away the decided GPUI and native terminal.

## 5. Architecture (summary; full detail in ARCHITECTURE.md)

```
almahryad (Rust daemon, launchd agent, always running)
 ├─ engine     sources (Lark, GitLab), Lark bot, runs, gates, permissions, read path, git, files, PTYs
 ├─ store      SQLite (sessions, runs, events, inbox, grants, audit)
 ├─ runtimes   Claude sidecar (TypeScript, Agent SDK) over JSON Lines
 └─ api        HTTP JSON (commands, queries) + WebSocket (events, terminal bytes), versioned /v1
        ▲ Unix socket + 127.0.0.1, token from Keychain
   ┌────┴─────────────┐
 Desktop (GPUI)     Web (React, later)
```

- Types for the API are Rust structs in one crate; TypeScript types are generated from them for the web.
- The desktop app uses the same API through a small Rust client crate. It holds no business logic.
- Terminals: the daemon owns PTYs (portable-pty) and streams raw bytes; each client parses and draws
  (desktop: libghostty-vt drawn in GPUI, fallback alacritty_terminal; web: xterm.js).
- The read path is an MCP server provided by the daemon to the agent: read-only query tools for Postgres,
  ClickHouse, Elasticsearch and Redis, keyed by source and environment. The daemon reads credentials; the
  agent never sees them.

## 6. Product design

### 6.1 Navigation

Sidebar (collapsible): Dashboard; Inbox (Lark n, GitLab n, + connection); Session (search); Source control
(only for the selected session, only when it changed files); Terminal; Settings at the bottom. The menu bar item
keeps the daemon visible when the window is closed.

### 6.2 Dashboard

"Waiting for you" first (gates, teammate requests, reopened tickets), then the personal ticket track
(first column to last column of the board, with gate notches), running agents, and today's numbers.
One load animation (the track draws, tickets land); everything else moves only in answer to an action.

### 6.3 Inbox

- Lark: mentions, DMs, groups, Base comments, teammate requests. Actions: send to agent (new session or the
  ticket's session), mark done (with undo), open in Lark.
- GitLab: merge requests to review, review comments on the owner's merge requests. Review comments arrive
  automatically (no pasting). The agent drafts a reply per thread; the owner sends it through a gate.
- A ticket assigned to the owner appears with "Research now" and "Dismiss".

### 6.4 Ticket session

One ticket is one session for its whole life (research, implementation, debug, review replies). Each stage is
a fresh run fed by the stage files on disk, not by old chat.

Layout (designed, "session v3"):
- Header: ticket key and title, related tickets, merge request chips per repo, "Open in editor" (all worktrees
  of the ticket), mini stage track.
- Status bar under the tabs, always present:
  - "Waiting for you" (attention colour) with a jump to the card, or
  - "Agent is" (agent colour) with what runs now. Nothing waits on the owner in this state.
- Tabs: Conversation, Documents, Activity.
- Conversation:
  - Decision card instead of paragraph questions. Clickable options with consequences, one marked
    recommended, an own-answer option, and a default used automatically after 30 minutes. Answered cards
    collapse into a summary. If the chosen option needs a temporary grant, the summary shows it with "View
    journal" and "Revoke".
  - Subagent card: elapsed time, tool count, last command, View, Stop. Paused subagents say so.
  - Push gate (see 6.5).
- Right panel:
  - Stages of the run with results (attempt number, passed, failed, not run, a link to the reasons).
  - "Automatic decisions" count, opening a drawer (6.6).
  - "Ticket notes": facts the agent must not forget (scope, product rules, architecture). Stored in the ticket
    folder, editable, re-read by the agent after every compaction.
- Documents tab: the ticket folder grouped by stage (inputs, research, implementation, QA and debug), with
  Read / Edit / Side by side, find (Cmd+F) and replace (Cmd+H).
- Activity tab: timeline of tool calls, permission decisions, data reads (with environment badge), grants.

Run state lives in the ticket folder (run state file plus a human-readable run log), so a run survives
compaction, app restarts and multi-day gaps. Small choices the agent makes on its own are recorded there as
automatic decisions.

### 6.5 Push gate

One card per run when everything is ready:
- Repo table: commits, tests, coverage, review result per repo. Weak results are marked, for example
  "syntax check only".
- "What you should know": tests not run and why, skipped hooks, unreviewed changes, new dependencies.
- Manual checks. The Push button stays disabled until every manual check is ticked. Groups of tests that were
  not run never block the push (owner's decision).
- Merge order. Buttons: Push and open merge requests, View changes (diff), Open in editor, Hold.
- After push: merge request links are commented on the ticket and the ticket moves on the board.
- Ticket documents (the sprint folder) are committed and pushed automatically when a stage finishes, without
  a gate. Code always goes through this gate.

### 6.6 Drawers

- Automatic decisions: filter by stage and "Deviates" (choices that differ from notes or defaults). Each entry
  has the reason and "Disagree", which puts the objection in the composer; the agent reruns that stage.
- Test results per case: not-run cases grouped by reason. Per group:
  - "Create test data", which asks for a temporary grant.
  - "Make it a manual check", which adds it to the push gate.
  - "Accept untested", which is written in the merge request body.
  Case codes open details.

### 6.7 Source control and diff

VS Code-like. Changed files grouped by repo (repos first, loose files after). Diff inline or side by side;
stage and discard per hunk and per file. No commit box in the sidebar: actions live in a right-click menu that
opens a popup per action (Commit, Discard confirmation). Commit can be per file. "Write with agent" fills the
commit message.

### 6.8 Terminal

Tabs and split panes. "Send to session" passes output to a session (the agent then asks before editing files
outside a worktree). Suggestions can be dismissed.

### 6.9 Composer, skills, context

- "/" opens a skill picker: skills already used in this session on top, all enabled skills below.
- Local skill suggestions appear as chips above the composer, matched on the Mac with no tokens spent.
- A context meter chip in the session header opens a breakdown (base, skill list, connectors used, loaded skills,
  conversation, run cost against the budget).

### 6.10 Lark bot

- Long connection to Lark (no public webhook). First response to a mention is an emoji reaction.
- Access rule: the sender must be on the allowed-people list AND the message must be in an allowed group.
  A DM from an allowed person is answered.
- Questions are answered read-only from context.
  - From the owner: actions run.
  - From a teammate: actions become an Inbox approval ("<name> asks …, Approve / Reject"). Nothing runs on
    the owner's Mac without the owner's approval.
- Thread = session. A top-level mention starts a new session and the bot replies in a thread.
- Context comes in layers:
  1. the mention itself;
  2. the thread, or a window of recent messages;
  3. resolved references (tickets, merge requests, images, docs);
  4. further read-only searching, which is logged.
- Context follows where it was asked (no private material in group answers). Ambiguous references are asked
  back. Edited messages use the latest text; deleted ones stop the work. When the Mac was asleep, the bot
  answers missed mentions on wake with a note.

### 6.11 Settings (v3)

Pages: General, Connections, Tickets (parked), Agent, Skills, Connectors, Security (parked).
- General: theme (Light, Dark, System), one language setting for UI and agent replies (English, Indonesian),
  open at login, keep running when the window closes, notifications, terminal (shell, font, Option as Meta),
  data (config path, session history retention, monthly cost with CSV export).
- Connections: accounts only (GitLab, Lark). Lark expands into "Bot and access": bot on/off, allowed groups and
  allowed people (autocomplete pickers from Lark), behaviour (seen reaction, context window, rate limit, outside
  the lists, when the Mac sleeps).
- Agent: model, budget per run and per day, shared memory folder (read and write), and subagents:
  - Global limits: depth 3 below the main agent, 20 concurrent per session.
  - List: built-in ones plus those imported from Claude Code, each with an on/off toggle and Edit.
  - Edit drawer, mapped to the SDK agent definition:
    - when to use (description), instructions (prompt), model, effort
    - allowed and denied tools, skills preloaded at start, connectors
    - turn limit, background, read project instructions
    - permission mode: may be stricter than Security, never looser, never bypass
  - Edits are an Almahrya copy; the Claude Code file is untouched.
- Skills: the user's skills folder, searchable, with a checkbox per skill. The agent may load only checked skills.
  The SDK lets `/name` force a skill outside the allowlist, so the backend also gives the SDK a generated skills
  folder containing only checked skills and denies the Skill tool for others.
- Connectors: plugins and MCP servers, from two sources (imported from Claude Code, or installed in Almahrya).
  Each has an on/off toggle that applies only to Almahrya. A connector newly found in Claude Code arrives switched
  off with a "New from Claude Code" badge.

No concurrency limit setting. No model per session type. No skill or connector "load sets".

## 7. Security model (decided parts)

Three groups. The page that presents them is parked, but these rules are decided:
- Always allowed, logged: reading code, logs and ticket documents; reading staging and production databases
  through the read path; local tests and read-only commands.
- Needs the owner:
  - push and merge requests (push gate);
  - `--force`, skipping hooks (`-n`), asked separately;
  - writes to staging through a temporary grant scoped to one ticket, journaled and cleaned up;
  - messages, comments and review replies in the owner's name;
  - teammate requests from the bot;
  - file edits outside the ticket's working copy and the ticket documents folder.
- Never, cannot be granted, not even from chat:
  - writing, changing or deleting production data;
  - reading `.env` files, credentials, `~/.ssh`, `~/.aws`, the Keychain;
  - running raw `psql`, `redis-cli`, `kubectl`, `podman`;
  - bypass permission mode.
- Read path: the backend's own read-only query service for each database and environment. Credentials come from
  the user's `.env` file, read only by the backend. Queries are logged; writes are refused; sensitive columns are
  masked. Settings has a credentials editor (keys listed, values hidden, reveal per value; my proposal: Touch ID to
  reveal, re-hidden after 30 seconds).
- Temporary grants are granted from decision cards and listed with "Clean up now" and "Revoke".

## 8. Parked (do last)

- Tickets settings page. Where the work tracker lives, the ticket documents folder, and the "moments"
  (assigned, notes written after the owner's talk with the tech lead, research final, review comments, reopened)
  with their buttons, skills, and wait-for-click versus run-automatically. Open: how the app knows the owner
  finished writing notes.
- Security settings page presentation and the file-edit rule. Leading proposal: inside a worktree the agent edits
  freely; outside it always asks; a plain chat session that needs to edit a repo offers once to create a worktree.
  Teammate requests always need approval first.

## 9. Calls made after the builder check (proposals the owner can correct)

- Stages: in v1 Almahrya does not orchestrate stages itself. The user's skills do, as they already do in Claude
  Code, and write a run state file in the ticket folder. Almahrya reads that file through a documented generic
  schema (stages with name, status, attempt, result counts, links to reports, automatic decisions, gate request).
  A per-user mapping in `config.toml` adapts an existing file format. The UI (stage list, gate, drawers) is built
  from that file plus the event stream.
- Push gate facts:
  - The daemon computes git facts itself: commits, ahead/behind, changed files, and whether hooks were skipped,
    detected from the commit command.
  - Tests, coverage, review results, manual checks and "what you should know" come from the run state file the
    skill writes.
  - The daemon never guesses a repository's test command.
- Enforcement of the never-list is layered:
  1. Rust policy on every `canUseTool` call, with command parsing (not substring matching) for Bash.
  2. The Claude Code sandbox for Bash, which blocks reading the secret paths and network access to database hosts.
     Only the read path, outside the sandbox, can reach them.
  3. The read path itself only runs statements it has parsed as read-only.
- Messages in the owner's name: Lark and GitLab writes go through daemon-provided tools that open a gate.
  Connectors imported from Claude Code that can write to Lark or GitLab have their write tools classified as
  "needs the owner".
- Accounts:
  - The Lark bot is a self-built Lark app on the long-connection event API.
  - The owner's own mentions, DMs and Base comments are read with the owner's user authorization.
  - GitLab uses a personal access token against a configurable (self-hosted) URL, polled, so no public endpoint
    is needed.
- Read path in v1: Postgres and ClickHouse first, then Redis and Elasticsearch. Masking rules come from config,
  with defaults by column name (secret, token, password, key).
- Session truth: SQLite in the daemon is the source of truth for what the UI shows. Each ticket stage starts a
  fresh SDK session fed by the stage files. A plain chat or bot thread resumes its own SDK session.

## 10. Build order (agreed with the owner)

0. Spikes first: GPUI window with ui-kit wrappers, and libghostty-vt rendering cells in GPUI. Both are pre-1.0.
1. Daemon and API: `almahryad`, `/v1` HTTP + WebSocket, token, event replay, `api-types` with TypeScript
   generation, `client`.
2. One ticket session end to end: sidecar, permission flow, status bar, decision cards, subagent cards, stage list
   from the run state file, ticket notes, push gate.
3. Inbox: GitLab (review comments, reply drafts through a gate), then Lark.
4. Lark bot.
5. Terminal.
6. Read path.

Open question for the owner: are the production database users in the credential file read-only at the database
level? If yes, production writes are impossible even if every software layer failed.

## 11. Left to the builder

- Exact API routes and event names (follow the conventions in ARCHITECTURE.md).
- Crate-internal module layout, error types, logging format.
- Choice of HTTP framework, schema export tool and git library, within ARCHITECTURE.md's guidance.
- Light theme values beyond the token table in DESIGN.md.
- Feature ideas offered but not chosen: daily report, learning from reopened tickets, bot answering ticket
  status, cost and time per ticket.
