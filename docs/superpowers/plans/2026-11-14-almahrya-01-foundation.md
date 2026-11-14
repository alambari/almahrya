# Almahrya Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove the two risky UI dependencies, then build the daemon `almahryad` with its `/v1` HTTP + WebSocket API and a desktop window that lists sessions and updates live.

**Architecture:** One Rust daemon owns state (SQLite) and pushes typed events over a WebSocket with sequence numbers and replay. API types live in `api-types` and generate TypeScript. The desktop app talks to the daemon only through the `client` crate over a Unix socket.

**Tech Stack:** Rust 2024, tokio, axum (HTTP + WebSocket), rusqlite, serde, ts-rs, security-framework (Keychain), fd-lock, GPUI (git, pinned rev) with gpui-component, libghostty-vt, portable-pty.

**Spec:** `docs/superpowers/specs/2026-11-14-almahrya-design.md` and `ARCHITECTURE.md`.

## Global Constraints

- Public repository: no secrets, no work data, no company/team/people/Lark/GitLab identifiers, no ticket codes, no workflow or skill names. Fixtures use neutral names (`PROJ-123`).
- Rust edition 2024. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` pass after every task.
- Every external dependency is declared once in the root `Cargo.toml` `[workspace.dependencies]` with an exact version (`=x.y.z`), added with `cargo add` and then pinned to the resolved version. GPUI is a git dependency pinned to a `rev`.
- `core` and `api-types` never depend on GPUI, tokio IO, axum, rusqlite or network crates.
- Only `engine` performs side effects; `api` stays thin; GPUI lives only in `ui-kit` and `app`.
- Paths: config `~/.config/almahrya/config.toml`; data `~/Library/Application Support/Almahrya/`; secrets in the macOS Keychain. Tests override both with the env vars `ALMAHRYA_CONFIG` and `ALMAHRYA_DATA_DIR`.
- API: everything under `/v1`; one JSON error shape `{ "code": string, "message": string }`; Bearer token on every route except `GET /v1/health`.
- No `unwrap()` outside tests. Typed errors per crate (`thiserror`).
- Commits: small, imperative subjects, never a co-author trailer.

## Review Focus

1. Daemon restarts while the desktop is connected: the client reconnects and resumes from its last sequence number with no duplicate and no missed event. Test: Task 8 `reconnect_resumes_without_gap_or_duplicate`.
2. A second daemon is started while one runs: it exits with a clear error and leaves the first untouched. Test: Task 7 `second_instance_refuses`.
3. A stale socket file is left after a crash: the next start acquires the lock, removes the stale socket and serves. Test: Task 7 `stale_socket_is_replaced`.
4. Missing or wrong token: `401` with code `unauthorized`, and the token never appears in a response or log line. Test: Task 6 `wrong_token_is_rejected_without_echo`.
5. A client asks to replay from a sequence number older than the retained window: it gets a `replay_gap` event telling it to reload state instead of silently missing events. Test: Task 5 `subscribe_before_retention_reports_gap`.

---

### Task 1: Workspace shape and checks

**Files:**
- Modify: `Cargo.toml` (members, `[workspace.lints]`, shared deps)
- Create: `crates/api-types/Cargo.toml`, `crates/api-types/src/lib.rs`
- Create: `crates/api/Cargo.toml`, `crates/api/src/lib.rs`
- Create: `crates/daemon/Cargo.toml`, `crates/daemon/src/main.rs`
- Create: `crates/client/Cargo.toml`, `crates/client/src/lib.rs`
- Create: `scripts/check.sh`
- Modify: `.gitignore` (add `/spikes/*/target`)

**Interfaces:**
- Produces: workspace crates `almahrya-api-types`, `almahrya-api`, `almahrya-daemon` (binary `almahryad`), `almahrya-client`, each with `[lints] workspace = true`.

- [ ] **Step 1: Add the four crates as empty libraries (daemon as a bin named `almahryad` with `fn main() {}`), each with a one-line crate doc comment matching ARCHITECTURE.md's crate table.**
- [ ] **Step 2: Add `[workspace.lints.rust] unsafe_code = "deny"` and `[workspace.lints.clippy] unwrap_used = "deny"` (allowed in tests via `#[cfg_attr(test, allow(clippy::unwrap_used))]` at crate roots).**
- [ ] **Step 3: Write `scripts/check.sh` running `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace`, executable.**
- [ ] **Step 4: Run `scripts/check.sh`.** Expected: exits 0.
- [ ] **Step 5: Commit** `chore: add api-types, api, daemon and client crates`

### Task 2: Spike: GPUI window with ui-kit tokens (throwaway)

**Files:**
- Create: `spikes/gpui-window/` (its own `Cargo.toml` with `[workspace]` so it is outside the main workspace)
- Create: `docs/superpowers/spikes/gpui.md`

**Interfaces:**
- Produces: a pinned GPUI `rev` and gpui-component version known to build on this Mac, recorded in `docs/superpowers/spikes/gpui.md`, to be copied into the root `Cargo.toml` in Task 9.

- [ ] **Step 1: Build a window 1440×900 with a 244 px sidebar (`#1F1F22`) and main area (`#18181A`), one text in Plus Jakarta Sans 13 px `#ECEAE4`, one in JetBrains Mono 13 px, one gpui-component button.**
- [ ] **Step 2: Run `cargo run` in `spikes/gpui-window`.** Expected: window opens; both fonts render (fallback font visible means fonts not found; note it).
- [ ] **Step 3: Write `docs/superpowers/spikes/gpui.md`: rev, versions, build time, binary size, idle memory (Activity Monitor), font loading notes, anything that broke.**
- [ ] **Step 4: Commit** `spike: GPUI window with design tokens`

### Task 3: Spike: libghostty-vt cells drawn in GPUI (throwaway)

**Files:**
- Create: `spikes/terminal/` (own workspace)
- Create: `docs/superpowers/spikes/terminal.md`

**Interfaces:**
- Produces: a go/no-go for libghostty-vt `0.2.2` (requires Zig on PATH) versus the fallback `alacritty_terminal`, recorded with numbers in `docs/superpowers/spikes/terminal.md`.

- [ ] **Step 1: Spawn `$SHELL -l` with portable-pty (80×24), read bytes on a thread, feed them to a libghostty-vt terminal owned by that thread (its handles are `!Send`), send cell snapshots to the GPUI thread over a channel, draw the grid in JetBrains Mono with colours.**
- [ ] **Step 2: Run it; type `ls --color`, `vim`, then resize the window.** Expected: colours correct, vim full screen, resize reflows.
- [ ] **Step 3: Measure `cat` of a 50 MB file: time to finish and peak memory. Repeat with `alacritty_terminal` if libghostty-vt fails any of step 2.**
- [ ] **Step 4: Write `docs/superpowers/spikes/terminal.md` with the decision and numbers.**
- [ ] **Step 5: Commit** `spike: terminal rendering in GPUI`

### Task 4: Core ids, sessions and API types with TypeScript export

**Files:**
- Modify: `crates/core/src/lib.rs`; Create: `crates/core/src/session.rs`, `crates/core/tests/session.rs`
- Modify: `crates/api-types/src/lib.rs`; Create: `crates/api-types/src/{event.rs,session.rs,error.rs}`, `crates/api-types/tests/export.rs`, `crates/api-types/bindings/` (generated, committed)

**Interfaces:**
- Produces (core):
  - `pub struct SessionId(uuid::Uuid)` with `SessionId::new() -> Self`, `Display`, `FromStr`
  - `pub enum SessionKind { Chat, Ticket { key: String } }`
  - `pub struct Session { pub id: SessionId, pub title: String, pub kind: SessionKind, pub created_at: time::OffsetDateTime }`
- Produces (api-types, all `Serialize + Deserialize + ts_rs::TS`):
  - `SessionDto { id: String, title: String, kind: SessionKindDto, created_at: String /* RFC 3339 */ }`
  - `SessionKindDto` (tagged `{"type":"chat"}` / `{"type":"ticket","key":...}`)
  - `CreateSessionRequest { title: String, kind: SessionKindDto }`
  - `EventEnvelope { seq: u64, at: String, event: EventDto }`
  - `EventDto` (tagged by `"type"`): `SessionCreated { session: SessionDto }`, `ReplayGap { oldest_available: u64 }`
  - `ApiError { code: String, message: String }`
  - `impl From<&core::Session> for SessionDto`

- [ ] **Step 1: Write failing tests**

```rust
// crates/core/tests/session.rs
#[test]
fn session_id_round_trips_through_string() {
    let id = SessionId::new();
    assert_eq!(id.to_string().parse::<SessionId>().unwrap(), id);
}
#[test]
fn session_ids_are_unique() { assert_ne!(SessionId::new(), SessionId::new()); }

// crates/api-types/tests/export.rs
#[test]
fn ticket_kind_serializes_tagged() {
    let k = SessionKindDto::Ticket { key: "PROJ-123".into() };
    assert_eq!(serde_json::to_string(&k).unwrap(), r#"{"type":"ticket","key":"PROJ-123"}"#);
}
#[test]
fn typescript_bindings_are_current() {
    // regenerates into a temp dir and compares with crates/api-types/bindings/
    assert_eq!(generate_bindings_to_temp(), read_committed_bindings());
}
```

- [ ] **Step 2: Run `cargo test -p almahrya-core -p almahrya-api-types`.** Expected: FAIL, types not defined.
- [ ] **Step 3: Implement the types above. Generate bindings with ts-rs into `crates/api-types/bindings/` via `cargo test -p almahrya-api-types export_bindings -- --ignored` (an ignored test that writes files); `typescript_bindings_are_current` compares.**
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(core,api-types): sessions, events and generated TypeScript types`

### Task 5: Store and engine with event replay

**Files:**
- Modify: `crates/store/src/lib.rs`; Create: `crates/store/src/{migrate.rs,sessions.rs,events.rs}`, `crates/store/migrations/0001_init.sql`, `crates/store/tests/store.rs`
- Modify: `crates/engine/src/lib.rs`; Create: `crates/engine/src/events.rs`, `crates/engine/tests/events.rs`

**Interfaces:**
- Consumes: `core::Session`, `api_types::{EventDto, EventEnvelope}`
- Produces (store):
  - `Store::open(path: &Path) -> Result<Store, StoreError>` (runs migrations; WAL mode)
  - `Store::insert_session(&self, s: &Session) -> Result<(), StoreError>`
  - `Store::list_sessions(&self) -> Result<Vec<Session>, StoreError>` (newest first)
  - `Store::append_event(&self, e: &EventDto) -> Result<EventEnvelope, StoreError>` (assigns `seq`)
  - `Store::events_after(&self, seq: u64, limit: usize) -> Result<Vec<EventEnvelope>, StoreError>`
  - `Store::oldest_seq(&self) -> Result<Option<u64>, StoreError>`
  - `Store::prune_events(&self, keep_last: u64) -> Result<(), StoreError>`
- Produces (engine):
  - `Engine::new(store: Store, retention: u64) -> Engine` (`retention` default `10_000` events)
  - `Engine::create_session(&self, title: String, kind: SessionKind) -> Result<Session, EngineError>` (persists, appends `SessionCreated`, broadcasts)
  - `Engine::list_sessions(&self) -> Result<Vec<Session>, EngineError>`
  - `Engine::subscribe(&self, after: u64) -> Result<impl Stream<Item = EventEnvelope>, EngineError>`: replays stored events after `after`, then live; if `after < oldest_seq - 1`, first yields `ReplayGap { oldest_available }`.

- [ ] **Step 1: Write failing tests**

```rust
// crates/store/tests/store.rs
#[test]
fn events_get_increasing_seq_and_replay_after() {
    let s = Store::open(&tmp().join("a.db")).unwrap();
    let a = s.append_event(&created("one")).unwrap();
    let b = s.append_event(&created("two")).unwrap();
    assert_eq!(b.seq, a.seq + 1);
    assert_eq!(s.events_after(a.seq, 10).unwrap().len(), 1);
}

// crates/engine/tests/events.rs
#[tokio::test]
async fn subscriber_gets_replay_then_live() {
    let e = engine();
    e.create_session("one".into(), SessionKind::Chat).await.unwrap();
    let mut sub = e.subscribe(0).await.unwrap();
    assert!(matches!(sub.next().await.unwrap().event, EventDto::SessionCreated { .. }));
    e.create_session("two".into(), SessionKind::Chat).await.unwrap();
    assert!(matches!(sub.next().await.unwrap().event, EventDto::SessionCreated { .. }));
}
#[tokio::test]
async fn subscribe_before_retention_reports_gap() {
    let e = engine_with_retention(2);
    for i in 0..5 { e.create_session(format!("s{i}"), SessionKind::Chat).await.unwrap(); }
    let mut sub = e.subscribe(0).await.unwrap();
    assert!(matches!(sub.next().await.unwrap().event, EventDto::ReplayGap { oldest_available: 4 }));
}
```

- [ ] **Step 2: Run `cargo test -p almahrya-store -p almahrya-engine`.** Expected: FAIL.
- [ ] **Step 3: Implement. Engine holds `tokio::sync::broadcast` (capacity 1024); subscription subscribes to broadcast before reading the replay and drops live events with `seq <= last replayed` to avoid duplicates. Prune after each append when count exceeds `retention`.**
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(store,engine): sessions and event log with replay`

### Task 6: HTTP and WebSocket API with token auth

**Files:**
- Modify: `crates/api/src/lib.rs`; Create: `crates/api/src/{routes.rs,auth.rs,ws.rs,error.rs}`, `crates/api/tests/http.rs`, `crates/api/tests/ws.rs`

**Interfaces:**
- Consumes: `Engine` (Task 5), api-types (Task 4)
- Produces:
  - `pub fn router(engine: Arc<Engine>, token: Token) -> axum::Router`
  - `pub struct Token(String)` with constant-time compare; `Debug` prints `Token(***)`
  - Routes: `GET /v1/health` → `{"status":"ok"}` (no auth); `GET /v1/sessions` → `Vec<SessionDto>`; `POST /v1/sessions` (`CreateSessionRequest`) → `201` + `SessionDto`; `GET /v1/events?after=<u64>` → WebSocket upgrade, sends `EventEnvelope` JSON text frames.
  - Auth: `Authorization: Bearer <token>`; the WebSocket also accepts `?token=` (browsers cannot set headers on upgrade).
  - Errors: `401 {"code":"unauthorized","message":"Missing or invalid token"}`; `422 {"code":"invalid_request",...}`.

- [ ] **Step 1: Write failing tests**

```rust
#[tokio::test]
async fn health_needs_no_token() { assert_eq!(get(app(), "/v1/health", None).await.status(), 200); }
#[tokio::test]
async fn wrong_token_is_rejected_without_echo() {
    let res = get(app(), "/v1/sessions", Some("nope-secret")).await;
    assert_eq!(res.status(), 401);
    let body = body_string(res).await;
    assert!(body.contains(r#""code":"unauthorized""#));
    assert!(!body.contains("nope-secret"));
}
#[tokio::test]
async fn create_then_list_session() { /* POST then GET returns one SessionDto with the title */ }
#[tokio::test]
async fn websocket_streams_replay_then_live() { /* bind 127.0.0.1:0, connect with ?token=, receive SessionCreated for an existing session, create another, receive it */ }
```

- [ ] **Step 2: Run `cargo test -p almahrya-api`.** Expected: FAIL.
- [ ] **Step 3: Implement with axum; auth as a middleware layer on everything except `/v1/health`; never log header values.**
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(api): v1 sessions, events WebSocket and token auth`

### Task 7: Daemon binary: config, token, sockets, single instance, launchd

**Files:**
- Modify: `crates/daemon/src/main.rs`; Create: `crates/daemon/src/{config.rs,paths.rs,token_store.rs,lock.rs,launchd.rs,serve.rs}`, `crates/daemon/tests/daemon.rs`

**Interfaces:**
- Consumes: `api::router`, `Engine`, `Store`
- Produces:
  - `Config { api_port: u16 /* default 47821 */, data_dir: PathBuf, event_retention: u64 /* 10_000 */ }` from `config.toml` `[api] port`, overridable by `ALMAHRYA_CONFIG` / `ALMAHRYA_DATA_DIR`
  - `trait TokenStore { fn get_or_create(&self) -> Result<Token, DaemonError>; }` with `KeychainTokenStore` (service `almahrya`, account `api-token`) and `FileTokenStore` (tests only, behind `ALMAHRYA_TOKEN_FILE`)
  - Listens on `<data_dir>/almahryad.sock` (mode 0600) and `127.0.0.1:<api_port>`
  - Lock file `<data_dir>/almahryad.lock` via fd-lock; if held → exit code 3 with message `almahryad is already running`
  - Stale socket: when the lock is acquired and the socket file exists, remove it before binding
  - `almahryad install-agent` writes `~/Library/LaunchAgents/dev.almahrya.daemon.plist` (RunAtLoad, KeepAlive) and prints the `launchctl bootstrap` command; `almahryad` with no args serves
  - SIGTERM/SIGINT: stop accepting, close WebSockets, exit 0

- [ ] **Step 1: Write failing tests (spawn the built binary with temp `ALMAHRYA_DATA_DIR`, `ALMAHRYA_TOKEN_FILE`, free port)**

```rust
#[test] fn serves_health_on_tcp_and_unix_socket() { /* GET /v1/health over both → 200 */ }
#[test] fn second_instance_refuses() { /* start A, start B → B exits 3, stderr contains "already running", A still healthy */ }
#[test] fn stale_socket_is_replaced() { /* create a plain file at almahryad.sock, start → healthy over the socket */ }
#[test] fn sigterm_exits_cleanly() { /* send SIGTERM → exit status 0 within 2 s, socket file removed */ }
```

- [ ] **Step 2: Run `cargo test -p almahrya-daemon`.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(daemon): almahryad with sockets, single instance and launchd agent`

### Task 8: Rust API client with resume

**Files:**
- Modify: `crates/client/src/lib.rs`; Create: `crates/client/src/{http.rs,events.rs}`, `crates/client/tests/client.rs`

**Interfaces:**
- Consumes: api-types; for tests `api::router` + `Engine`
- Produces:
  - `Client::unix(socket: PathBuf, token: String) -> Client`, `Client::tcp(base: Url, token: String) -> Client`
  - `async fn list_sessions(&self) -> Result<Vec<SessionDto>, ClientError>`
  - `async fn create_session(&self, req: CreateSessionRequest) -> Result<SessionDto, ClientError>`
  - `fn events(&self, after: u64) -> impl Stream<Item = Result<EventEnvelope, ClientError>>`: reconnects with backoff 0.25 s doubling to 5 s, resumes from the last delivered `seq`, never yields a `seq` twice.

- [ ] **Step 1: Write failing tests**

```rust
#[tokio::test]
async fn reconnect_resumes_without_gap_or_duplicate() {
    // server 1 on a socket; create s1; consume its event; stop server 1;
    // create s2 and s3 directly in the same store; start server 2 on the same socket;
    // the stream yields exactly s2 then s3, with seq = previous + 1 each.
}
#[tokio::test]
async fn create_and_list_over_unix_socket() { /* round trip */ }
```

- [ ] **Step 2: Run `cargo test -p almahrya-client`.** Expected: FAIL.
- [ ] **Step 3: Implement (hyper client over `tokio::net::UnixStream` for the socket; tokio-tungstenite for WebSocket over both transports).**
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Commit** `feat(client): API client with event resume`

### Task 9: Desktop window: start daemon, list sessions live

**Files:**
- Modify: `Cargo.toml` (GPUI rev and gpui-component version from Task 2's notes)
- Modify: `crates/ui-kit/src/lib.rs`; Create: `crates/ui-kit/src/{theme.rs,sidebar.rs}`
- Modify: `crates/app/src/main.rs`; Create: `crates/app/src/{daemon_link.rs,sessions_view.rs}`, `crates/app/tests/daemon_link.rs`

**Interfaces:**
- Consumes: `client::Client` (Task 8); daemon binary (Task 7)
- Produces:
  - `ui_kit::Theme` with the dark tokens from DESIGN.md (`bg #18181A`, `surface #232326`, `sidebar #1F1F22`, `hairline #2C2C30`, `text #ECEAE4`, `text_muted #97958E`, `text_faint #6A6863`, `agent #6F7CF2`, `waiting #E8A93C`)
  - `daemon_link::ensure_daemon(paths: &Paths) -> Result<DaemonLink, LinkError>`: connects to the socket; if absent or refused, spawns `almahryad` next to the app binary, waits up to 5 s for `/v1/health`, then connects. Token read from the Keychain (same service/account as Task 7).
  - Window: sidebar with a "Session" group listing sessions newest first, updated from the event stream; a "New chat session" button creating `{title:"New chat", kind:chat}`.

- [ ] **Step 1: Write failing test**

```rust
#[tokio::test]
async fn ensure_daemon_starts_one_when_none_runs_and_reuses_it() {
    // temp data dir; ensure_daemon → health ok; ensure_daemon again → same pid (from /v1/health "pid" field)
}
```

Add `"pid": u32` to the health response in Task 6's handler as part of this task (and to api-types).

- [ ] **Step 2: Run `cargo test -p almahrya-app`.** Expected: FAIL.
- [ ] **Step 3: Implement `ensure_daemon`; build the window with ui-kit components; subscribe with `Client::events(0)` and apply `SessionCreated` to the list; on `ReplayGap`, reload with `list_sessions`.**
- [ ] **Step 4: Run tests, then `cargo run -p almahrya-app`: click "New chat session" twice; kill `almahryad`; confirm the app restarts it and the list still shows both sessions.**
- [ ] **Step 5: Commit** `feat(app): desktop window with live session list`

### Task 10: CI for the public repository

**Files:**
- Create: `.github/workflows/check.yml`
- Create: `scripts/scrub.sh`

**Interfaces:**
- Produces: CI on macOS running `scripts/check.sh` and `scripts/scrub.sh` on every push and pull request.

- [ ] **Step 1: Write `scripts/scrub.sh`: fails if tracked files match patterns listed in a local, untracked `~/.config/almahrya/scrub-patterns` (one regex per line) plus built-in generic patterns (`-----BEGIN .* PRIVATE KEY-----`, `ghp_[A-Za-z0-9]{20,}`, `xox[bp]-`, `postgres://[^ ]+:[^ ]+@`). In CI only the built-in patterns run.**
- [ ] **Step 2: Run `scripts/scrub.sh` locally.** Expected: exits 0 on the current tree; exits 1 when a test file containing `ghp_aaaaaaaaaaaaaaaaaaaaaa` is added (remove it after).
- [ ] **Step 3: Add the workflow (macos-latest, stable Rust, cache cargo).**
- [ ] **Step 4: Commit** `ci: checks and secret scrub for the public repository`
