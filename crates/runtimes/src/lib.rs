//! Agent runtimes behind one `Runtime` trait: start a run, answer its requests,
//! interrupt it, and receive its events in one shared shape.
//!
//! Adapters own process lifecycle and validate every event they receive; they never
//! own permissions or workflow, which stay in `almahrya_core` and `almahrya_engine`.
//!
//! The first adapter is Claude, which runs `sidecars/claude` (the Claude Agent SDK)
//! over bounded JSON Lines. Codex (`codex app-server`) and ACP adapters are added when
//! they are needed.
