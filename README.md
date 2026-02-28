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

### Global Search

Press `s` from the project list to open **Global Search** — a cross-project full-text search over all session prompts.

初回起動時にSQLiteインデックスを自動構築し（`~/.claude/projects/` 配下を並列スキャン）、2回目以降は差分のみ更新するため高速に起動します。

```
 Claude Session Viewer
 Search: JWT認証█
┌ Global Search (2 results) ────────────────────────────────┐
│ Time     Project       Branch      Prompt                  │
│ 14:30    api-server    feat/auth   ...Add JWT認証 to the...│
│ Feb 10   web-app       main        ...JWT認証フローの実装...│
└────────────────────────────────────────────────────────────┘
Enter: Detail  y: Copy resume cmd  Esc: Back  j/k: Navigate
```

- **リアルタイム絞り込み** — 1文字入力するごとに結果が即座に更新されます
- **大文字小文字を無視** — `jwt` でも `JWT` でもマッチします
- **マッチハイライト** — 一致したテキストが黄色でハイライトされ、前後のコンテキストが `...` 付きで表示されます
- **セッション復帰** — 結果を選んで `y` を押すと `claude --resume <session-id>` コマンドがクリップボードにコピーされ、すぐにそのセッションを再開できます
- **詳細表示** — `Enter` でそのセッションの会話全文を閲覧できます

## Features

- Browse projects and sessions under `~/.claude/projects/`
- Three-screen navigation: Project List -> Session List -> Session Detail
- **Global Search** (`s` key) — substring search across all session prompts with match highlighting
- Fuzzy search with `/` key for project/session filtering (powered by [skim](https://github.com/lotabout/fuzzy-matcher))
- Time filter: Yesterday / Week / Month / All
- Color-coded messages by role (User, Assistant, Tool, Result, System)
- Vim-style keybindings
- Auto-scrolling tables — selected row always stays visible

## Installation

```bash
cargo install --path .
```

これにより `~/.cargo/bin/cc-sessions-viewer` にバイナリがインストールされ、どこからでも `cc-sessions-viewer` コマンドで起動できます（`~/.cargo/bin` がPATHに含まれている必要があります）。

ビルドのみ行う場合：

```bash
cargo build --release
# バイナリは target/release/cc-sessions-viewer に生成されます
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
| `s` | Global Search across all sessions (Project list) |
| `y` | Copy `claude --resume` command (Global Search) |
| `/` | Fuzzy search (Project / Session list) |
| `Tab` | Next time filter (Session list) |
| `Shift+Tab` | Previous time filter (Session list) |

## Dependencies

- [ratatui](https://github.com/ratatui/ratatui) - TUI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal backend
- [fuzzy-matcher](https://github.com/lotabout/fuzzy-matcher) - Fuzzy search
- [rusqlite](https://github.com/rusqlite/rusqlite) - SQLite session index
- [rayon](https://github.com/rayon-rs/rayon) - Parallel indexing
- [cli-clipboard](https://github.com/nicohman/rust-clipboard) - Clipboard support
- [serde](https://github.com/serde-rs/serde) / [serde_json](https://github.com/serde-rs/json) - JSON parsing
- [chrono](https://github.com/chronotope/chrono) - Date/time handling
- [dirs](https://github.com/dirs-dev/dirs-rs) - Home directory resolution

## License

MIT
