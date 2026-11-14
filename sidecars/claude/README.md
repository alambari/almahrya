# Claude sidecar

A thin TypeScript process around the Claude Agent SDK (pinned). The daemon (`almahryad`) starts one per active
agent session and speaks bounded JSON Lines over stdin and stdout.

It translates SDK messages into the app's event shape and relays every permission request, `canUseTool` call and
question to the daemon, then waits for the answer. It owns no policy: the Rust side validates every event and
decides every permission. See ARCHITECTURE.md, "Agent runtime (Claude)".

Not implemented yet.
