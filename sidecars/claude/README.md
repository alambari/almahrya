# Claude sidecar

A thin TypeScript process around the Claude Agent SDK (pinned). The Rust
`almahrya-runtimes` crate starts it and speaks bounded JSON Lines over stdin/stdout.

It translates SDK messages into the app's event shape and relays permission and
question requests to the app. It owns no policy: the Rust side validates every event
and decides every permission.

Adapted from Aurora's `agent-worker`. Not implemented yet.
