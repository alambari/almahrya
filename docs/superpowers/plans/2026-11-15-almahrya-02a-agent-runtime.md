# Almahrya Agent Runtime and Permissions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A run of the Claude agent can be started through the daemon API, every tool call is classified by Rust rules, calls that need the owner wait as decision requests until answered over the API, and everything is recorded as domain events. No new desktop screens: the flow is exercised through the API, the Rust client and tests.

**Architecture:** A TypeScript sidecar wraps the Claude Agent SDK and forwards every tool call (through a `PreToolUse` hook) and every `AskUserQuestion` to the daemon over bounded JSON Lines; it holds no policy. `core` classifies each call (allow, ask, deny); the engine turns "ask" into a pending decision, waits for the answer, and replies to the sidecar. Events become domain events in `core`, persisted by `store` and mapped to wire DTOs in `api`.

**Tech Stack:** Rust 2024, tokio, serde, ts-rs, shlex (command tokenizing), TypeScript on Node 18.14+ with `@anthropic-ai/claude-agent-sdk` (exact pinned version), node:test.

**Spec:** `docs/superpowers/specs/2026-11-14-almahrya-design.md` (sections 6.4, 6.5, 7) and `ARCHITECTURE.md` ("Agent runtime (Claude)", "Permissions and grants"). Carry-over decisions: `docs/superpowers/plans/README.md` ("Carry into Plan 2").

## Global Constraints

- Public repository: no secrets, no work data, no company/team/people identifiers, no ticket codes, no workflow or skill names. NEVER write any word from the owner's local denylist (not even inside another word) or a ticket code. Fixtures use `PROJ-123`, `example-api`, `/researcher`. A local denylist drives `scripts/scrub.sh`; never write its words into the repo.
- Rust edition 2024; `scripts/check.sh` (fmt, clippy `-D warnings`, tests) passes after every task; `unsafe_code` is denied; no `unwrap()` outside tests; typed errors with `thiserror`.
- Every external Rust dependency is declared once in the root `Cargo.toml` `[workspace.dependencies]` with an exact version (`=x.y.z`). The sidecar pins every npm dependency to an exact version and commits its lockfile.
- Only `ui-kit` depends on GPUI. `core` and `api-types` stay free of IO, async runtimes and network crates. Only `engine` performs side effects; `api` stays thin.
- The sidecar holds no policy: every allow, ask and deny comes from Rust. The main session never uses `bypassPermissions`; `settingSources` is `[]`; `allowedTools` is empty so no setting can auto-approve a call before the hook.
- Never read or write the owner's real Keychain in tests; never call the real Claude API in tests (the sidecar's SDK is behind an interface with a fake).
- Sidecar lines are UTF-8 JSON objects, one per line, at most 1 MiB; anything longer or malformed is dropped with a `log` event, never fatal.
- Commits: small, imperative subjects, never a co-author trailer.

## Review Focus

1. A call classified "deny" never reaches the owner: no decision is created, the sidecar is answered "deny" at once, and no API route can turn it into "allow". Tests: Task 4 `denied_calls_never_ask`, Task 8 `denied_call_creates_no_decision`.
2. The sidecar dies mid-run: pending decisions resolve to deny, the run is marked failed with an event, nothing waits forever. Test: Task 8 `sidecar_exit_fails_run_and_resolves_pending`.
3. A garbage, truncated or oversized line from the sidecar does not crash the daemon or grow memory. Test: Task 7 `oversized_and_malformed_lines_are_dropped`.
4. A late, duplicate or unknown answer is rejected or ignored: unknown id gives 404, a second answer gives 409, an answer after the run ended is ignored. Test: Task 9 `answers_are_idempotent_and_validated`.
5. Shell commands that try to dodge the raw-client rule: `bash -c "psql …"`, `env X=1 kubectl …`, `sudo`, `$(…)` and backticks, `;` `&&` `||` `|` chains, quoted names, absolute paths such as `/usr/local/bin/psql`, `xargs`. Test: Task 4 `raw_clients_are_denied_in_every_shell_shape`. Reads of secret paths via `Read`, `Grep`, `Glob` and `cat`: `.env`, `../../.env`, `~/.ssh/…`, symlinks resolving there. Test: Task 4 `secret_paths_are_denied_for_every_reader`.

---

### Task 1: Domain events in core, persisted by store, mapped in api

**Files:**
- Create: `crates/core/src/event.rs`, `crates/core/tests/event.rs`
- Modify: `crates/core/src/lib.rs`, `crates/store/src/events.rs`, `crates/store/src/lib.rs`, `crates/store/Cargo.toml` (drop the `almahrya-api-types` dependency), `crates/engine/src/events.rs`, `crates/api/src/ws.rs`, `crates/api-types/src/event.rs`
- Create: `crates/store/migrations/0002_domain_events.sql`

**Interfaces:**
- Produces (core): `pub enum DomainEvent { SessionCreated { session: Session } }` (`#[non_exhaustive]`), `pub struct EventRecord { pub seq: u64, pub at: OffsetDateTime, pub event: DomainEvent }`, `DomainEvent::to_json(&self) -> String` and `DomainEvent::from_json(&str) -> Result<DomainEvent, EventDecodeError>`; the JSON carries `"v": 1` and a `"type"` tag; an unknown `v` or type gives `EventDecodeError::Unknown { v, kind }` rather than a panic.
- Produces (store): `append_event(&self, e: &DomainEvent) -> Result<EventRecord, StoreError>`, `events_after(&self, seq: u64, limit: usize) -> Result<Vec<EventRecord>, StoreError>` (rows that fail to decode are skipped and counted in `Store::undecodable_events(&self) -> u64`, never silently ending a replay).
- Produces (api): `impl From<&EventRecord> for EventEnvelope`; `EventDto` stays the wire type.
- Ruling for the builder: this is pre-release data. Migration 0002 deletes existing `events` rows and records `v` in the payload; no row conversion.

- [ ] **Step 1: Write failing tests**

```rust
// crates/core/tests/event.rs
#[test] fn session_created_round_trips_with_version() { /* to_json contains "v":1; from_json equals original */ }
#[test] fn unknown_version_is_an_error_not_a_panic() { /* from_json(r#"{"v":99,"type":"x"}"#) is Err(Unknown{v:99,..}) */ }
// crates/store/tests/store.rs
#[test] fn undecodable_rows_are_skipped_and_counted() { /* insert a bad payload row; events_after skips it; undecodable_events() == 1 */ }
```

- [ ] **Step 2: Run `cargo test -p almahrya-core -p almahrya-store`.** Expected: FAIL (types not defined).
- [ ] **Step 3: Implement `DomainEvent`, the store changes and migration 0002; engine publishes `EventRecord`; the WebSocket pump maps to `EventEnvelope`; existing engine, api and client tests keep passing unchanged.**
- [ ] **Step 4: Run `scripts/check.sh`.** Expected: PASS.
- [ ] **Step 5: Commit** `refactor: persist domain events and map them to wire types in api`

### Task 2: Store epoch so replay cannot silently skip events

**Files:**
- Modify: `crates/store/src/{lib.rs,migrate.rs}`, `crates/store/migrations/0003_epoch.sql` (create), `crates/engine/src/events.rs`, `crates/api/src/routes.rs`, `crates/api/src/ws.rs`, `crates/api-types/src/{health.rs,event.rs}`, `crates/client/src/events.rs`
- Test: `crates/store/tests/store.rs`, `crates/engine/tests/events.rs`, `crates/client/tests/client.rs`

**Interfaces:**
- Produces: `Store::epoch(&self) -> &str` (a random 128-bit hex id stored in a `meta` table at first open and never changed for that database file); `HealthDto { status, pid, epoch: String }`; a first WebSocket frame `EventDto::Hello { epoch: String, newest_seq: u64 }` (envelope `seq` = `newest_seq`); `EventsQuery` accepts `epoch=<id>` next to `after`. Server rule: if the client's `epoch` is present and differs, or `after > newest_seq`, send `EventDto::ReplayGap { oldest_available: 1 }` after `Hello` and replay from the start. Client rule: remember the epoch; when `Hello.epoch` differs from the remembered one, reset its `last_seq` to 0 before consuming further events (so lower seqs are accepted) and pass the Hello through.

- [ ] **Step 1: Write failing tests**
  - store: `epoch_is_stable_across_reopen_and_differs_between_databases`
  - engine: `subscribe_with_a_cursor_beyond_newest_reports_gap`
  - client: `epoch_change_resets_the_cursor_and_accepts_lower_seqs` (server 1 with events up to seq 5; client consumes them; server 2 on a fresh database with seq 1 and 2; the stream yields Hello, ReplayGap, then seq 1 and 2).
- [ ] **Step 2: Run the three crates' tests.** Expected: FAIL.
- [ ] **Step 3: Implement; regenerate TypeScript bindings (`cargo test -p almahrya-api-types export_bindings -- --ignored`) and commit them.**
- [ ] **Step 4: Run `scripts/check.sh`.** Expected: PASS.
- [ ] **Step 5: Commit** `feat: store epoch and hello frame for safe event resume`

### Task 3: WebSocket shutdown, data-dir permissions

**Files:**
- Modify: `crates/api/src/{routes.rs,ws.rs,serve.rs}`, `crates/local/src/paths.rs`, `crates/store/src/lib.rs`
- Test: `crates/api/tests/ws.rs`, `crates/local/tests/paths.rs`, `crates/store/tests/store.rs`

**Interfaces:**
- Produces: `router_with_shutdown(engine: Arc<Engine>, token: Token, shutdown: tokio::sync::watch::Receiver<bool>) -> Router` (existing `router` stays and never shuts down); on shutdown every pump sends `Message::Close(Some(CloseFrame { code: 1001, reason: "going away" }))` and ends; `serve_unix`/`serve_tcp` document and honour it; `Paths::ensure_data_dir(&self) -> Result<(), LocalError>` creates the directory 0700 and **tightens an existing directory to 0700**; `Store::open` creates the database file 0600 (and the `-wal`/`-shm` files follow the main file's mode).

- [ ] **Step 1: Write failing tests**
  - api: `shutdown_sends_close_1001_to_open_websockets`
  - local: `existing_loose_data_dir_is_tightened_to_0700`
  - store: `database_file_is_created_0600`
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement; the daemon creates the watch channel and flips it on SIGTERM/SIGINT before the 1.5 s drain.**
- [ ] **Step 4: Run `scripts/check.sh`.** Expected: PASS.
- [ ] **Step 5: Commit** `feat: close websockets with 1001 on shutdown and tighten data permissions`

### Task 4: Permission rules in core

**Files:**
- Create: `crates/core/src/permission.rs`, `crates/core/src/shell.rs`, `crates/core/tests/permission.rs`, `crates/core/tests/shell.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/Cargo.toml` (shlex)

**Interfaces:**
- Produces:
  - `pub struct ToolCall { pub name: String, pub input: serde_json::Value, pub agent_id: Option<String> }`
  - `pub enum Decision { Allow, Ask { reason: String }, Deny { reason: String } }`
  - `pub struct PermissionContext { pub worktrees: Vec<PathBuf>, pub documents_dirs: Vec<PathBuf>, pub home: PathBuf, pub file_edit: FileEditPolicy, pub enabled_connectors: BTreeSet<String> }` and `pub enum FileEditPolicy { ReadOnly, AskEveryEdit, FreeInWorktree }`
  - `pub fn classify(call: &ToolCall, ctx: &PermissionContext) -> Decision`
  - `pub fn shell_words(command: &str) -> Vec<Vec<String>>`: splits a shell command into simple commands, looking through `;` `&&` `||` `|` `&`, `$(…)`, backticks, `bash -c '…'`/`sh -c`/`zsh -c`, `env VAR=… cmd`, `sudo`, `xargs`, `nohup`, `time`, quotes and backslashes; returns every simple command's argv with the program name reduced to its base name.
- Rules (from the spec, section 7), in this order: (1) never: argv[0] is `psql`, `redis-cli`, `kubectl`, `podman` (any absolute path or alias form); a path argument or redirect that resolves under `.env*`, `~/.ssh`, `~/.aws`, `~/Library/Keychains`, or a `credentials` file; the tool `bypass`-style requests; (2) ask: `git push`, any git command with `--force`/`-f` on push, `-n`/`--no-verify` on commit, `gh pr create`-like publishing commands left as ask, file writes outside every worktree and documents dir, Lark/GitLab write-capable tools from connectors; (3) file edits inside a worktree follow `file_edit` (`ReadOnly` → deny, `AskEveryEdit` → ask, `FreeInWorktree` → allow); (4) everything else allow (reads, searches, local test commands). The `Skill` tool and connector tools are allowed only when named in `enabled_connectors`/an allowlist passed in the context (Plan 2b supplies it; until then the field defaults to "all enabled").
  - Paths are canonicalized (symlinks resolved; non-existent files resolved through their parent) before the secret-path check.

- [ ] **Step 1: Write failing tests** (names and assertions; each lists real inputs)
  - `raw_clients_are_denied_in_every_shell_shape`: `psql -h x`, `/usr/local/bin/psql`, `bash -c "psql -c 'select 1'"`, `env PGPASSWORD=x psql`, `sudo kubectl get pods`, `echo $(redis-cli get a)`, `` `podman ps` ``, `ls; kubectl get ns`, `cat f | xargs psql`, `FOO=1 psql` → all `Decision::Deny`; `echo psql` and `git log --grep psql` → `Allow`.
  - `secret_paths_are_denied_for_every_reader`: tools `Read`, `Grep`, `Glob`, `Edit`, `Write` and Bash `cat`, `less`, `head`, `source` on `.env`, `./sub/../.env`, `../../.env`, `~/.ssh/id_example`, `~/.aws/credentials`, and a symlink under a worktree pointing to `.env` (built in a temp dir) → `Deny`; `README.md` → `Allow`.
  - `denied_calls_never_ask`: no input that yields `Deny` can be turned into `Ask` by `file_edit` or `enabled_connectors` settings (property test over a fixed list of settings).
  - `push_and_force_and_no_verify_ask`: `git push`, `git push --force-with-lease`, `git commit -n`, `git commit --no-verify -m x` → `Ask`; `git status`, `git diff` → `Allow`.
  - `edits_follow_the_file_edit_policy`: inside a worktree: `ReadOnly` → Deny, `AskEveryEdit` → Ask, `FreeInWorktree` → Allow; outside every worktree: Ask for all policies except `ReadOnly` (Deny); under a documents dir: Allow for `FreeInWorktree`, Ask otherwise.
  - `shell_words_looks_through_wrappers`: unit cases for `shell_words` with expected argv lists.
- [ ] **Step 2: Run `cargo test -p almahrya-core`.** Expected: FAIL.
- [ ] **Step 3: Implement `shell_words` (use `shlex` for quoting; handle `-c` strings and `$(…)` by recursion with a depth limit of 8) and `classify`. An unparseable command (unbalanced quote, depth exceeded) is `Ask` with the reason "command could not be analysed".**
- [ ] **Step 4: Run `scripts/check.sh`.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): permission classification and shell command analysis`

### Task 5: Sidecar protocol types with generated TypeScript

**Files:**
- Create: `crates/runtimes/src/protocol.rs`, `crates/runtimes/tests/protocol.rs`, `crates/runtimes/bindings/` (generated, committed)
- Modify: `crates/runtimes/src/lib.rs`, `crates/runtimes/Cargo.toml`

**Interfaces:**
- Produces (all `Serialize + Deserialize + ts_rs::TS`, `#[serde(tag = "type", rename_all = "snake_case")]`):
  - `pub const PROTOCOL_VERSION: u32 = 1;` and `pub const MAX_LINE_BYTES: usize = 1_048_576;`
  - `Command`: `Start(StartRun)`, `UserMessage { run_id, text }`, `Interrupt { run_id }`, `AnswerPermission { request_id, decision: PermissionAnswer }`, `AnswerQuestion { request_id, answers: BTreeMap<String, Vec<String>> }`, `Stop { run_id }`, `Shutdown`.
  - `StartRun { run_id: String, cwd: String, prompt: String, model: Option<String>, resume_session_id: Option<String>, max_turns: Option<u32>, max_budget_usd: Option<f64>, allowed_tool_names: Vec<String>, env: BTreeMap<String, String> }` (`allowed_tool_names` is only a hint list for the SDK's `tools` option; it never grants permission).
  - `PermissionAnswer`: `Allow { updated_input: Option<serde_json::Value> }` / `Deny { message: String }`.
  - `Event`: `Ready { protocol_version, sdk_version }`, `Session { run_id, sdk_session_id, model, tools: Vec<String>, skills: Vec<String> }`, `Text { run_id, agent_id: Option<String>, text }`, `ToolUse { run_id, agent_id, tool_use_id, name, input }`, `ToolResult { run_id, agent_id, tool_use_id, is_error: bool, summary: String }`, `PermissionRequest { request_id, run_id, agent_id, tool_use_id, tool_name, input }`, `Question { request_id, run_id, questions: Vec<QuestionDto> }`, `Subagent { run_id, task_id, status, description, tool_uses: Option<u32>, duration_ms: Option<u64> }`, `Compacted { run_id, pre_tokens: u64 }`, `Result { run_id, subtype, is_error, total_cost_usd: f64, num_turns: u32, stop_reason: Option<String>, input_tokens: u64, output_tokens: u64 }`, `Failure { run_id: Option<String>, message }`, `Log { level, message }`.
  - `fn encode_line<T: Serialize>(v: &T) -> Result<String, ProtocolError>` (single line, trailing `\n`, error if the encoding exceeds `MAX_LINE_BYTES`) and `fn decode_line<T: DeserializeOwned>(line: &str) -> Result<T, ProtocolError>`.

- [ ] **Step 1: Write failing tests**: `commands_and_events_round_trip` (one value per variant), `encode_refuses_oversized_lines`, `decode_rejects_unknown_type_without_panicking`, `typescript_bindings_are_current` (same pattern as api-types).
- [ ] **Step 2: Run `cargo test -p almahrya-runtimes`.** Expected: FAIL.
- [ ] **Step 3: Implement and generate bindings into `crates/runtimes/bindings/`.**
- [ ] **Step 4: Run `scripts/check.sh`.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(runtimes): sidecar protocol types and generated TypeScript`

### Task 6: The Claude sidecar

**Files:**
- Create: `sidecars/claude/{package.json,package-lock.json,tsconfig.json,.nvmrc}`, `sidecars/claude/src/{main.ts,protocol.ts,sdk.ts,gate.ts,lines.ts}`, `sidecars/claude/test/{gate.test.ts,lines.test.ts,run.test.ts}`
- Modify: `sidecars/claude/README.md`, `.gitignore` (`sidecars/claude/node_modules/`, `sidecars/claude/dist/`)

**Interfaces:**
- Consumes: the generated bindings from Task 5 (copied by `npm run bindings` from `crates/runtimes/bindings/` into `src/generated/`, which is committed).
- Produces:
  - `src/sdk.ts`: `interface SdkAdapter { run(start: StartRun, hooks: GateHooks): AsyncIterable<SdkMessageLike> ; interrupt(runId: string): Promise<void>; }` with `RealSdk` (calls `query()` with `prompt` as an `AsyncIterable` for streaming input, `options.settingSources: []`, `options.allowedTools: []`, `options.permissionMode: 'default'`, `options.includePartialMessages: false`, `options.hooks.PreToolUse = [{ matcher: undefined, timeout: 86400, hooks: [gate] }]`, `options.canUseTool` handling only `AskUserQuestion`, `options.env` merged over `process.env` with `start.env`, `options.cwd`, `maxTurns`, `maxBudgetUsd`, `resume`) and `FakeSdk` (scripted messages, used only by tests).
  - `src/gate.ts`: `makeGate(send: (e: Event) => void, pending: PendingRequests)` returns a `PreToolUse` callback that sends `permission_request`, awaits `answer_permission`, and returns `{ hookSpecificOutput: { hookEventName: 'PreToolUse', permissionDecision: 'allow' | 'deny', permissionDecisionReason, updatedInput? } }`; if the daemon connection closes, every pending request resolves to deny.
  - `src/lines.ts`: `readLines(stream, maxBytes)`: async iterator yielding complete lines; a line over `maxBytes` is discarded up to the next newline and reported once via a callback (never buffered whole).
  - `src/main.ts`: reads commands on stdin, writes events on stdout (stdout is protocol only; diagnostics go to `log` events, never to stdout directly), sends `ready` first, exits 0 on `shutdown` or stdin end.
  - The SDK package is pinned to an exact version in `package.json`; `npm ci` must succeed offline from the lockfile cache.
- Mapping rules the tests pin: an `assistant` message's text blocks → `text`; `tool_use` blocks → `tool_use`; `user` messages' `tool_result` blocks → `tool_result` (summary truncated to 500 chars, never the full output); `system` `init` → `session`; `task_started`/`task_progress`/`task_notification` → `subagent`; compact boundary → `compacted`; `result` → `result`; anything unknown → ignored with one `log` at `debug`.

- [ ] **Step 1: Write failing tests** (node:test with `tsx` or compiled JS)
  - `gate.test.ts`: `gate_waits_for_the_daemon_answer` (allow → hook returns allow; deny with message → deny with reason), `closing_the_connection_denies_pending_requests`, `gate_never_decides_by_itself` (no code path returns allow without an `answer_permission`).
  - `lines.test.ts`: `oversized_line_is_discarded_and_reported_once`, `partial_lines_are_joined_across_chunks`.
  - `run.test.ts`: `fake_run_maps_messages_to_events` (a scripted FakeSdk conversation with text, tool use, a subagent task, a result produces exactly the expected event list in order), `ask_user_question_round_trip` (question event out, answer in, the adapter receives `updatedInput.answers`).
- [ ] **Step 2: Run `npm test` in `sidecars/claude`.** Expected: FAIL.
- [ ] **Step 3: Implement; verify the real SDK option names against the installed package's type definitions (compile-time check with `tsc --noEmit`).**
- [ ] **Step 4: Run `npm run build && npm test` and `scripts/check.sh`.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(sidecar): Claude Agent SDK sidecar with a Rust-owned permission gate`

### Task 7: Sidecar process management in runtimes

**Files:**
- Create: `crates/runtimes/src/process.rs`, `crates/runtimes/src/runtime.rs`, `crates/runtimes/tests/fake_sidecar.rs`, `crates/runtimes/tests/process.rs`
- Modify: `crates/runtimes/src/lib.rs`, `crates/runtimes/Cargo.toml`

**Interfaces:**
- Consumes: `protocol::{Command, Event}` (Task 5).
- Produces:
  - `pub trait AgentRuntime { async fn start(&self, config: SidecarConfig) -> Result<SidecarHandle, RuntimeError>; }`
  - `pub struct SidecarConfig { pub program: PathBuf, pub args: Vec<String>, pub env: BTreeMap<String, String>, pub cwd: PathBuf }`
  - `pub struct SidecarHandle` with `async fn send(&self, cmd: Command) -> Result<(), RuntimeError>`, `fn events(&mut self) -> &mut tokio::sync::mpsc::Receiver<SidecarEvent>`, `async fn shutdown(self, grace: Duration)`.
  - `pub enum SidecarEvent { Event(Event), Dropped { reason: String }, Exited { code: Option<i32> } }` — `Exited` is always the last item.
  - `ClaudeRuntime` implements `AgentRuntime` by spawning `node <sidecar entry>` (path from config) and waiting for `ready` (5 s) with the matching `PROTOCOL_VERSION`, else `RuntimeError::BadHandshake`.
  - Lines over `MAX_LINE_BYTES` or undecodable produce `Dropped`, never an error that ends the stream; stdout read is bounded; stderr is captured to a ring of the last 64 KiB for `RuntimeError` messages; the child is killed on drop; no token or environment value is ever printed.
- Tests use a fake sidecar: a small Rust test binary (`tests/fake_sidecar.rs` built as an example) that speaks the protocol and can be told to emit garbage, an oversized line, or exit.

- [ ] **Step 1: Write failing tests**
  - `handshake_requires_matching_protocol_version`
  - `commands_and_events_flow_both_ways`
  - `oversized_and_malformed_lines_are_dropped` (the stream continues with the next good event; memory stays bounded: assert the reader never holds more than `MAX_LINE_BYTES + 1` bytes)
  - `exit_is_reported_last_with_the_code`
  - `dropping_the_handle_kills_the_child` (the pid no longer exists after drop)
- [ ] **Step 2: Run `cargo test -p almahrya-runtimes`.** Expected: FAIL.
- [ ] **Step 3: Implement with `tokio::process`; reuse `encode_line`/`decode_line`.**
- [ ] **Step 4: Run `scripts/check.sh`.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(runtimes): supervised sidecar process with bounded framing`

### Task 8: Runs, decisions and permission flow in the engine

**Files:**
- Create: `crates/engine/src/runs.rs`, `crates/engine/src/decisions.rs`, `crates/engine/tests/runs.rs`
- Modify: `crates/core/src/event.rs` (new variants), `crates/engine/src/{lib.rs,events.rs}`, `crates/engine/Cargo.toml`, store migration `0004_runs.sql`, `crates/store/src/runs.rs`

**Interfaces:**
- Consumes: `AgentRuntime`/`SidecarHandle` (Task 7), `classify` (Task 4), `DomainEvent` (Task 1).
- Produces (core): new `DomainEvent` variants `RunStarted { run_id, session_id, prompt }`, `AgentText { run_id, agent_id, text }`, `ToolRequested { run_id, tool_use_id, name, input_summary, decision: DecisionKind }` (`DecisionKind::{Allowed, Asked, Denied}`), `ToolFinished { run_id, tool_use_id, is_error }`, `DecisionRequested { decision_id, run_id, kind: DecisionRequestKind, title, detail, options }`, `DecisionAnswered { decision_id, answer }`, `SubagentChanged { run_id, task_id, status, description, tool_uses, duration_ms }`, `RunFinished { run_id, outcome: RunOutcome, cost_usd, turns }` with `RunOutcome::{Completed, Failed { reason }, Interrupted, BudgetExceeded}`. `input_summary` is a truncated string (at most 300 chars) and never the full input.
- Produces (engine):
  - `Engine::with_runtime(self, runtime: Arc<dyn AgentRuntimeDyn>, sidecar: SidecarConfig, ctx: PermissionContext) -> Engine`
  - `async fn start_run(&self, session_id: SessionId, req: StartRunRequest) -> Result<RunId, EngineError>` where `StartRunRequest { prompt: String, cwd: PathBuf, model: Option<String>, max_budget_usd: Option<f64> }`
  - `async fn answer_decision(&self, decision_id: DecisionId, answer: DecisionAnswer) -> Result<(), EngineError>` returning `EngineError::UnknownDecision`, `EngineError::AlreadyAnswered`, `EngineError::RunEnded`
  - `async fn interrupt_run(&self, run_id: RunId) -> Result<(), EngineError>`; `fn pending_decisions(&self, session_id: SessionId) -> Vec<PendingDecision>`
  - Flow: each sidecar `PermissionRequest` → `classify`: Allow → answer allow immediately and record `ToolRequested{Allowed}`; Deny → answer deny immediately with the reason, record `ToolRequested{Denied}`, create no decision; Ask → record `DecisionRequested`, keep the sidecar request pending until `answer_decision`. `Question` events become `DecisionRequested` of kind question with the options. When the sidecar exits or the run is interrupted, every pending decision for the run resolves to deny/ignored and `RunFinished` is recorded. The budget is enforced by the sidecar's `maxBudgetUsd` and by the engine summing `Result` costs against `max_budget_usd` (a `Result` with `error_max_budget_usd` → `BudgetExceeded`).

- [ ] **Step 1: Write failing tests** (a `FakeRuntime` in the test file scripts sidecar events and records the commands the engine sends)
  - `allowed_call_is_answered_immediately_and_recorded`
  - `denied_call_creates_no_decision` (Review Focus 1: the sidecar receives a deny and `pending_decisions` is empty)
  - `asked_call_waits_for_the_owner_and_then_continues` (allow answer → the sidecar receives allow; the events show Requested then Answered)
  - `sidecar_exit_fails_run_and_resolves_pending` (Review Focus 2)
  - `second_answer_is_rejected_and_late_answer_after_end_is_ignored`
  - `question_round_trip_returns_the_selected_labels`
  - `budget_exceeded_ends_the_run_with_that_outcome`
  - `events_are_persisted_in_order_and_survive_a_restart` (new Engine over the same database sees the same run history)
- [ ] **Step 2: Run `cargo test -p almahrya-engine`.** Expected: FAIL.
- [ ] **Step 3: Implement; one task per run reads the sidecar stream and owns that run's pending table; never hold a std lock across an await.**
- [ ] **Step 4: Run `scripts/check.sh`.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(engine): agent runs with classified tool calls and pending decisions`

### Task 9: Run and decision endpoints

**Files:**
- Modify: `crates/api-types/src/{event.rs,run.rs (create),lib.rs}`, `crates/api/src/{routes.rs,error.rs}`, `crates/api/tests/http.rs`, `crates/api/tests/runs.rs (create)`, bindings regenerated

**Interfaces:**
- Produces (api-types): `StartRunRequestDto { prompt, cwd, model: Option<String>, max_budget_usd: Option<f64> }`, `RunDto { id, session_id, state }`, `DecisionDto { id, run_id, kind, title, detail, options: Vec<DecisionOptionDto> }`, `AnswerDecisionRequest { answer: DecisionAnswerDto }` where `DecisionAnswerDto` is `Allow` | `Deny { message }` | `Choice { labels: Vec<String> }` | `Text { text }`; new `EventDto` variants mirroring the Task 8 domain events (same names, snake_case).
- Produces (routes, all behind the token): `POST /v1/sessions/{id}/runs` → 201 `RunDto`; `GET /v1/sessions/{id}/decisions` → `Vec<DecisionDto>`; `POST /v1/decisions/{id}/answer` → 204; `POST /v1/runs/{id}/interrupt` → 202. Errors: unknown id → 404 `not_found`; already answered → 409 `already_answered`; run ended → 409 `run_ended`; invalid body → 422.

- [ ] **Step 1: Write failing tests** (engine with the FakeRuntime from Task 8 exposed through a `test-support` feature or a shared dev module)
  - `start_run_then_decision_then_answer_flow`
  - `answers_are_idempotent_and_validated` (Review Focus 4)
  - `denied_calls_have_no_answer_route` (there is no decision id to answer for a denied call: answering a made-up id is 404)
  - `events_for_a_run_arrive_over_the_websocket_in_order`
  - `typescript_bindings_are_current`
- [ ] **Step 2: Run `cargo test -p almahrya-api -p almahrya-api-types`.** Expected: FAIL.
- [ ] **Step 3: Implement the DTOs, mappings from domain events, and the four routes; regenerate and commit the bindings.**
- [ ] **Step 4: Run `scripts/check.sh`.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(api): run and decision endpoints with domain event mapping`

### Task 10: Client methods, a command-line harness and a manual smoke test

**Files:**
- Modify: `crates/client/src/lib.rs`, `crates/client/tests/client.rs`
- Create: `crates/client/examples/agent_run.rs`, `docs/superpowers/spikes/agent-smoke.md`
- Modify: `crates/daemon/src/main.rs` (wire `ClaudeRuntime` with the sidecar path from config `[agent] sidecar` or the default `sidecars/claude/dist/main.js` next to the executable; the `PermissionContext` comes from config with `file_edit = "ask_every_edit"` by default)

**Interfaces:**
- Produces: `Client::start_run(&self, session: &str, req: StartRunRequestDto) -> Result<RunDto, ClientError>`, `Client::decisions(&self, session: &str) -> Result<Vec<DecisionDto>, ClientError>`, `Client::answer_decision(&self, id: &str, a: DecisionAnswerDto) -> Result<(), ClientError>`, `Client::interrupt_run(&self, run: &str) -> Result<(), ClientError>`.
- The example `agent_run` takes `--socket`, `--token-file`, `--cwd` and a prompt, creates a session, starts a run, prints events, and asks the human on stdin for each decision. It never prints tokens.

- [ ] **Step 1: Write failing tests**: `client_start_run_and_answer_against_a_fake_runtime_server` (reuse the Task 9 server fixture).
- [ ] **Step 2: Run `cargo test -p almahrya-client`.** Expected: FAIL.
- [ ] **Step 3: Implement the client methods, the daemon wiring and the example.**
- [ ] **Step 4: Write `docs/superpowers/spikes/agent-smoke.md`: the exact commands for a MANUAL smoke run against the real SDK in a scratch repository (build the sidecar, start the daemon on the dev profile, run `agent_run` with a prompt that reads a file, one that tries `git push` (must ask) and one that tries `psql` (must be denied), what to expect, and how to stop). Agents do not run it; the owner does.**
- [ ] **Step 5: Run `scripts/check.sh`, then commit** `feat: client methods, run harness and manual smoke guide for the agent runtime`
