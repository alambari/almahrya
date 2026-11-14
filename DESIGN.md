# Design guide

How Almahrya looks, moves and speaks. Every client (desktop now, web later) follows this guide. Visual
reference: the design file kept outside the repository; this document is the source of truth when they differ.

## Character

A quiet workbench with gates. Calm surfaces, one colour that means "the agent", one colour that means
"waiting for you", and nothing else asking for attention.

## Colour tokens

| Token | Dark | Light | Use |
|---|---|---|---|
| `bg` (Arang / Kertas) | `#18181A` | `#F4F5F2` | Window background |
| `surface` (Batu) | `#232326` | `#E8E9E4` | Cards, popups, inputs |
| `sidebar` | `#1F1F22` | `#ECEDE8` | Sidebar, secondary panels |
| `hairline` | `#2C2C30` | `#DADBD5` | Dividers, borders |
| `text` (Tinta) | `#ECEAE4` | `#1D1D1F` | Primary text |
| `text-muted` (Kabut) | `#97958E` | `#6E6C66` | Secondary text |
| `text-faint` | `#6A6863` | `#8E8C86` | Hints, captions |
| `agent` (Nila) | `#6F7CF2` | `#4A57D6` | Agent activity, primary buttons, links (`#8E98F5` for link text on dark) |
| `waiting` (Kunyit) | `#E8A93C` | `#B97A12` | Only for "waiting for you": gates, decisions, approvals |
| `success` | `#7DBB8E` | `#3E8A57` | Passed, connected |
| `danger` | `#D98A7A` | `#B2503D` | Never-allowed rules, production badge, failures |

Rules:
- `waiting` appears only where the owner must act. Waiting items get a 3 px `waiting` left edge.
- `agent` marks what the agent is doing. The status bar uses it when nothing waits on the owner.
- `danger` is for rules that cannot be overridden and for production. Never decorative.

## Typography

- UI: Plus Jakarta Sans. Code, diffs, terminal, paths, keys, skill names: JetBrains Mono. Nothing else.
- Scale: 12 (captions), 13 (body, controls), 15 (reading text in sessions), 20 (drawer titles), 28 (page titles).
- Sentence case everywhere. No all-caps labels. No detail chains joined with middle dots.

## Layout

- Window: sidebar 244 px (collapsible to icons), main area, optional right panel 300 px.
- Settings: navigation column plus content column 640 to 720 px wide, left-aligned.
- Spacing on a 4 px grid. Radii: 6 for controls, 8 to 10 for cards, 12 for popups and drawers.
- Label and value pairs align in columns: build rows with a fixed-width label column. Never align with spaces.
- Drawers slide from the right over a dimmed scrim (60% black). Modals centre with a scrim (70% black).

## Components

| Component | Notes |
|---|---|
| Status bar | Under the session tabs. "Waiting for you" (`waiting`) or "Agent is" (`agent`) with one action on the right. Always present in sessions. |
| Decision card | Question title, options as rows with consequences, one "Recommended" tag, an own-answer row, a note on the default and its timeout, "Answer" button in `waiting`. Collapses into a summary after answering. |
| Gate card | `waiting` edge. Repo table, "What you should know", manual checks, merge order, primary action disabled until manual checks are ticked. |
| Subagent card | Dot (`agent` running, `waiting` paused), title, last command in mono, elapsed time and tool count, View, Stop. |
| Stage list | Done (check in `success`), current (highlighted row with result line and a link to reasons), upcoming (faint). |
| Toggle | 38 × 22, `agent` when on, `#3A3A40` when off. |
| Chip | Small rounded label for related items, merge requests, tool and skill names (mono). |
| List card | Rounded container, rows 44 px or taller, hairline dividers, checkbox or toggle at a fixed column. |
| Autocomplete picker | Selected items as removable chips in the input, matching text in bold, arrow keys to move, Enter to add, Backspace to remove the last chip. |
| Masked value | Mono dots with "Show" per row; revealed values re-hide after a timeout. |

## Motion

Motion explains change; it never decorates.
- One load moment: when the app opens, the dashboard's ticket track draws and tickets land.
- Everything else answers an action: sidebar collapse (spring), expand and collapse, diff slides in, context menu
  grows from the cursor, popups rise, drawers slide, tab underline slides, an approved gate moves the ticket.
- Durations: 140 to 160 ms for small state changes, 200 to 260 ms for drawers and navigation.
- A breathing dot only while something runs. Respect reduced motion: replace movement with opacity changes.

## Writing

- Indonesian or English, chosen in Settings; labels follow the language (for example "Berdampingan" /
  "Side by side").
- Name things by what the owner understands, not by how the system works.
- Buttons say what happens ("Push and open 5 merge requests", "Save to .env"); the result uses the same verb.
- Errors say what happened and what to do, without apologising. Empty states invite the next action.
- Every statement of a rule that cannot be changed is written as a statement, not as a disabled control.

## Every button has a result

Each button, link and menu item has a designed result state before it is built. Keep the button map (button,
what happens, resulting screen) in the design file up to date; add the state there before adding the button in code.

## Screens

Dashboard; Inbox (Lark, GitLab); ticket session (conversation, documents, activity; decision card, subagent card,
push gate; drawers for automatic decisions and test results per case); source control and diff (right-click
actions, commit popup, discard confirmation); terminal (tabs, split); composer skill picker and suggestions;
context meter; settings (General, Connections with Lark bot and access, Agent with subagents, Skills, Connectors).
Parked: Tickets settings, Security settings.
