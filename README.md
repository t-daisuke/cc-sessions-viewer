# Claude Session Viewer

A terminal UI application for browsing Claude Code session history.

## Demo

```
 Claude Session Viewer
┌ Projects ──────────────────────────────────────────────────────┐
│ Project Path                              Sessions             │
│ /Users/you/src/github.com/org/api-server        12             │
│ /Users/you/src/github.com/org/web-app             8             │
│ /Users/you/src/github.com/org/cli-tool            3             │
└────────────────────────────────────────────────────────────────┘
Enter: Open  q: Quit  j/k: Navigate  d/u: Half Page  /: Search
```

Select a project with `Enter`, then browse its sessions:

```
 Claude Session Viewer
 Project: -Users-you-src-github-com-org-api-server
 Yesterday │ Week │ Month │ All
┌ Sessions ──────────────────────────────────────────────────────┐
│ Timestamp            Msgs  Branch          Preview             │
│ 2026-02-11 14:30:00    24  feat/auth       Add JWT auth...     │
│ 2026-02-10 09:15:00    18  fix/db-conn     Fix connection...   │
│ 2026-02-09 16:45:00     6  main            Refactor tests...   │
└────────────────────────────────────────────────────────────────┘
Enter: Open  Esc: Back  j/k: Navigate  d/u: Half Page  Tab: Filter  /: Search
```

Open a session to see the full conversation:

```
 Claude Session Viewer
 Session: a1b2c3d4

USER 2026-02-11 14:30:00
Add JWT authentication to the /api/login endpoint

ASSISTANT 2026-02-11 14:30:05
I'll implement JWT authentication. Let me first check the existing code.

TOOL 2026-02-11 14:30:06
[Read] src/routes/auth.rs

RESULT
pub fn login(req: LoginRequest) -> Result<Response> { ... }

ASSISTANT 2026-02-11 14:30:12
I see the current login handler. I'll add JWT token generation...
```

Use `/` to fuzzy-search across project paths or session previews:

```
/auth█
┌ Sessions (2 matches) ─────────────────────────────────────────┐
│ 2026-02-11 14:30:00    24  feat/auth       Add JWT auth...     │
│ 2026-01-28 11:00:00    15  fix/auth        Fix OAuth callback. │
└────────────────────────────────────────────────────────────────┘
```

## Features

- Browse projects and sessions under `~/.claude/projects/`
- Three-screen navigation: Project List -> Session List -> Session Detail
- Fuzzy search with `/` key (powered by [skim](https://github.com/lotabout/fuzzy-matcher))
- Time filter: Yesterday / Week / Month / All
- Color-coded messages by role (User, Assistant, Tool, Result, System)
- Vim-style keybindings

## Installation

### Homebrew

```bash
brew tap t-daisuke/tap
brew install cc-sessions-viewer
```

### Build from source

```bash
cargo install --path .
```

## Usage

インストール後はどこからでも実行できます：

```bash
cc-sessions-viewer
```

または、リポジトリ内で直接実行：

```bash
cargo run --release
```

## Keybindings

| Key | Action |
|-----|--------|
| `Enter` | Select / Open |
| `Esc` / `q` | Go back / Quit |
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `d` | Half page down |
| `u` | Half page up |
| `g` | Go to top |
| `G` | Go to bottom |
| `/` | Fuzzy search (Project / Session list) |
| `Tab` | Next time filter (Session list) |
| `Shift+Tab` | Previous time filter (Session list) |

## Dependencies

- [ratatui](https://github.com/ratatui/ratatui) - TUI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal backend
- [fuzzy-matcher](https://github.com/lotabout/fuzzy-matcher) - Fuzzy search
- [serde](https://github.com/serde-rs/serde) / [serde_json](https://github.com/serde-rs/json) - JSON parsing
- [chrono](https://github.com/chronotope/chrono) - Date/time handling
- [dirs](https://github.com/dirs-dev/dirs-rs) - Home directory resolution

## License

MIT
