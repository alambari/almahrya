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
