# Claude Code in this repository

Follow [AGENTS.md](AGENTS.md); it holds every rule for this repository.

@AGENTS.md

Claude-specific notes:
- This repository is public. Before proposing a commit, check the diff for secrets, work data, identifiers and
  workflow names, as AGENTS.md describes.
- When a change touches the interface, check DESIGN.md first and keep the button map in the design file current.
- When a change touches agent behaviour or permissions, the source of truth is the design document in
  `docs/superpowers/specs/`. Do not weaken the never-allowed rules.
- Use the Claude Agent SDK documentation (code.claude.com/docs/en/agent-sdk) for sidecar work; do not guess SDK
  options.
