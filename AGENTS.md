# Working in this repository

Rules for anyone changing Almahrya, people and coding agents alike. Read this, then
[ARCHITECTURE.md](ARCHITECTURE.md) and [DESIGN.md](DESIGN.md). The design document is
`docs/superpowers/specs/2026-11-14-almahrya-design.md`.

## This repository is public

- No secrets, tokens, passwords, connection strings or `.env` contents. Not in code, tests, fixtures, docs or
  commit messages.
- No work data: no company, team, people, Lark or GitLab identifiers, URLs of private systems, or ticket codes.
- No names of any team's workflow or skills. Workflow-specific values are configuration in the user's
  `config.toml`. Examples and fixtures use neutral names such as `PROJ-123`, `~/work/tickets`, `/researcher`.
- Before every commit, search the diff for anything above. If in doubt, leave it out and ask.
- Design files with real data stay outside the repository.

## Architecture rules

- One backend (`almahryad`) owns all state and all side effects. Clients (desktop, web) only call the API.
- `core` and `api-types` are pure: no GPUI, no network, no filesystem, no async runtime specifics.
- Only `engine` touches git, files, SQLite, the sidecar, PTYs and the network. `api` stays thin.
- GPUI lives only in `ui-kit` and `app`. Pin GPUI to a commit.
- Every API type lives in `api-types`. Never hand-write TypeScript types for the API; generate them.
- The sidecar holds no policy. Every permission decision is made in Rust from rules in `core`.
- Never run an agent session in `bypassPermissions`. Never add a way to grant production writes or secret reads.
- Every external dependency is declared once in the workspace `Cargo.toml` with an exact version.

## Interface rules

- Follow DESIGN.md: tokens, type scale, sentence case, no all-caps labels, aligned label columns.
- `waiting` colour only where the user must act; `agent` colour for agent activity.
- Every button has a designed result state before it is built.
- User-facing text exists in Indonesian and English.

## Code

- Rust edition 2024. `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` must pass.
- Errors are typed per crate; no `unwrap` outside tests.
- Comments explain why, not what. Public items have doc comments.
- Integration tests live in each crate's `tests/` directory. New rules in `core` come with unit tests.

## Commits and merge requests

- Small commits, one logical change each, imperative subject lines.
- Never add co-author trailers to commits or merge request descriptions.
- Do not commit generated build output or the local design file.

## Local files you must not read or print

The user's `.env` credential file, `~/.ssh`, `~/.aws`, the Keychain, and anything under the data directory that
holds tokens. The daemon reads credentials; nothing else does.
