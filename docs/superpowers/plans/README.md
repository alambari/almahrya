# Implementation plans

The design (`docs/superpowers/specs/2026-11-14-almahrya-design.md`, section 10) fixes the build order. Each stage
gets its own plan, written when the previous stage is done, so later plans use what earlier stages taught.

| # | Plan | Delivers | Status |
|---|---|---|---|
| 1 | [Foundation](2026-11-14-almahrya-01-foundation.md) | GPUI and libghostty-vt spikes; `almahryad` with `/v1` HTTP + WebSocket, token, event replay; `api-types` with TypeScript generation; `client`; a desktop window listing sessions live | Ready |
| 2 | Ticket session | Claude sidecar, permission flow in `core`, run state file schema and reader, status bar, decision cards, subagent cards, ticket notes, push gate | After 1 |
| 3 | Inbox: GitLab | Merge requests and review comments by polling, reply drafts sent through a gate | After 2 |
| 4 | Inbox: Lark | Mentions, DMs, Base comments with user authorization; send to agent; mark done | After 3 |
| 5 | Lark bot | Long connection, access rule, acknowledgement, question vs action, teammate approvals, thread = session | After 4 |
| 6 | Terminal | PTYs in the daemon, `terminal.output` events, libghostty-vt rendering in the desktop | After 5 |
| 7 | Read path | MCP read-only service: Postgres and ClickHouse, then Redis and Elasticsearch; credentials editor | After 6 |
| 8 | Settings | General, Connections, Agent with subagents, Skills allowlist, Connectors | Alongside 2 to 7 as each needs it |

Parked in the design (Tickets settings, Security settings presentation) get plans only after the owner decides them.

## Carry into Plan 2 (Ticket session)

The final review of the foundation left these decisions for the next plan. They are requirements for Plan 2, not
built yet.

- Store epoch: give the event store an epoch (or a reset signal) so replay cannot silently skip events after the
  database is replaced. `/v1/health` and the WebSocket hello frame carry the epoch; a client accepts lower sequence
  numbers after the epoch changes.
- Event model: decide whether stored events are domain events in `core`, mapped to DTOs in `api`, or document that
  the event log is versioned together with the API (so `store` need not depend on `api-types`). Decide before adding
  more event kinds.
- WebSocket shutdown: give the event pump a shutdown signal that sends a Close frame (1001, going away), and correct
  the `serve` documentation to match.
- Data directory permissions: tighten an existing data directory to 0700 and create the database with mode 0600
  before any work data is stored.
- launchd and the app: set the agent's `KeepAlive` to `{ SuccessfulExit = false }` and exit 0 on "already running"
  when started by launchd, or do not spawn the daemon from the app when the agent plist exists.
- SQLite off the async threads: run store calls through `spawn_blocking` before terminal traffic arrives.
- Chunked replay: replay the event window in chunks instead of loading it whole.
