"""Claude Code Session Viewer - TUI Application.

textual ライブラリを使用した、Claude Code セッションログビューア。
"""

from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Vertical, VerticalScroll
from textual.screen import Screen
from textual.widgets import DataTable, Footer, Header, Static

from session_parser import (
    Message,
    ProjectInfo,
    SessionInfo,
    list_projects,
    list_sessions,
    load_session,
)


class ProjectListScreen(Screen):
    """プロジェクト一覧画面。"""

    BINDINGS = [
        Binding("enter", "select", "Open"),
        Binding("q", "quit_app", "Quit"),
        Binding("j", "cursor_down", "Down", show=False),
        Binding("k", "cursor_up", "Up", show=False),
    ]

    def __init__(self, projects: list[ProjectInfo]) -> None:
        super().__init__()
        self.projects = projects

    def compose(self) -> ComposeResult:
        yield Header()
        yield DataTable(id="project-table")
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#project-table", DataTable)
        table.cursor_type = "row"
        table.add_columns("Project Path", "Sessions")
        for project in self.projects:
            table.add_row(
                project.original_path,
                str(project.session_count),
                key=project.dir_name,
            )
        if self.projects:
            table.focus()

    def action_select(self) -> None:
        table = self.query_one("#project-table", DataTable)
        if table.row_count == 0:
            return
        row_key, _ = table.coordinate_to_cell_key(table.cursor_coordinate)
        project = next(
            (p for p in self.projects if p.dir_name == row_key.value), None
        )
        if project:
            self.app.push_screen(
                SessionListScreen(project.dir_name, project.original_path)
            )

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        project = next(
            (p for p in self.projects if p.dir_name == event.row_key.value), None
        )
        if project:
            self.app.push_screen(
                SessionListScreen(project.dir_name, project.original_path)
            )

    def action_cursor_down(self) -> None:
        table = self.query_one("#project-table", DataTable)
        table.action_cursor_down()

    def action_cursor_up(self) -> None:
        table = self.query_one("#project-table", DataTable)
        table.action_cursor_up()

    def action_quit_app(self) -> None:
        self.app.exit()


class SessionListScreen(Screen):
    """セッション一覧画面。"""

    BINDINGS = [
        Binding("enter", "select", "Open"),
        Binding("escape", "go_back", "Back"),
        Binding("q", "go_back", "Back"),
        Binding("j", "cursor_down", "Down", show=False),
        Binding("k", "cursor_up", "Up", show=False),
    ]

    def __init__(self, project_name: str, project_path: str) -> None:
        super().__init__()
        self.project_name = project_name
        self.project_path = project_path
        self.sessions: list[SessionInfo] = []

    def compose(self) -> ComposeResult:
        yield Header()
        yield Static(f" Project: {self.project_path}", id="breadcrumb")
        yield DataTable(id="session-table")
        yield Footer()

    def on_mount(self) -> None:
        self.sessions = list_sessions(self.project_name)
        table = self.query_one("#session-table", DataTable)
        table.cursor_type = "row"
        table.add_columns("Timestamp", "Messages", "Branch", "Preview")
        for session in self.sessions:
            preview = session.preview
            if len(preview) > 80:
                preview = preview[:80] + "..."
            preview = preview.replace("\n", " ")
            table.add_row(
                session.timestamp_str,
                str(session.message_count),
                session.git_branch,
                preview,
                key=session.session_id,
            )
        if self.sessions:
            table.focus()

    def action_select(self) -> None:
        table = self.query_one("#session-table", DataTable)
        if table.row_count == 0:
            return
        row_key, _ = table.coordinate_to_cell_key(table.cursor_coordinate)
        self._open_session(row_key.value)

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        self._open_session(event.row_key.value)

    def _open_session(self, session_id: str) -> None:
        session = next(
            (s for s in self.sessions if s.session_id == session_id), None
        )
        if session:
            self.app.push_screen(
                SessionDetailScreen(
                    self.project_name, session.session_id, session.preview
                )
            )

    def action_go_back(self) -> None:
        self.app.pop_screen()

    def action_cursor_down(self) -> None:
        table = self.query_one("#session-table", DataTable)
        table.action_cursor_down()

    def action_cursor_up(self) -> None:
        table = self.query_one("#session-table", DataTable)
        table.action_cursor_up()


class SessionDetailScreen(Screen):
    """セッション詳細画面（会話ビュー）。"""

    BINDINGS = [
        Binding("escape", "go_back", "Back"),
        Binding("q", "go_back", "Back"),
        Binding("j", "scroll_down", "Down", show=False),
        Binding("k", "scroll_up", "Up", show=False),
    ]

    DEFAULT_CSS = """
    SessionDetailScreen {
        layout: vertical;
    }
    #breadcrumb {
        height: 1;
        background: $surface;
        color: $text-muted;
        padding: 0 1;
    }
    #messages-scroll {
        height: 1fr;
    }
    .message-user {
        background: #1a3a5c;
        color: #e0e0e0;
        padding: 1 2;
        margin: 0 0 1 0;
    }
    .message-assistant {
        background: #2a2a3a;
        color: #e0e0e0;
        padding: 1 2;
        margin: 0 0 1 0;
    }
    .message-tool {
        background: #2a2a2a;
        color: #888888;
        padding: 0 2;
        margin: 0 0 0 4;
    }
    .message-system {
        background: #3a2a1a;
        color: #aa8855;
        padding: 0 2;
        margin: 0 0 1 0;
    }
    .message-timestamp {
        color: #666666;
        text-style: italic;
    }
    .message-role {
        text-style: bold;
        margin: 0 0 0 0;
    }
    """

    def __init__(
        self, project_name: str, session_id: str, preview: str
    ) -> None:
        super().__init__()
        self.project_name = project_name
        self.session_id = session_id
        self.preview = preview

    def compose(self) -> ComposeResult:
        yield Header()
        short_id = self.session_id[:8] + "..."
        yield Static(f" Session: {short_id}", id="breadcrumb")
        yield VerticalScroll(id="messages-scroll")
        yield Footer()

    def on_mount(self) -> None:
        messages = load_session(self.project_name, self.session_id)
        container = self.query_one("#messages-scroll", VerticalScroll)
        for msg in messages:
            widget = self._make_message_widget(msg)
            container.mount(widget)

    def _make_message_widget(self, msg: Message) -> Static:
        ts = f"  [{msg.timestamp_str}]" if msg.timestamp_str else ""
        if msg.role == "user":
            label = f"[bold cyan]USER[/bold cyan]{ts}\n{msg.text}"
            widget = Static(label, classes="message-user", markup=True)
        elif msg.role == "assistant":
            label = f"[bold green]ASSISTANT[/bold green]{ts}\n{msg.text}"
            widget = Static(label, classes="message-assistant", markup=True)
        elif msg.role in ("tool_use", "tool_result"):
            prefix = "TOOL" if msg.role == "tool_use" else "RESULT"
            text = msg.text
            if len(text) > 500:
                text = text[:500] + "..."
            label = f"[dim]{prefix}: {text}[/dim]"
            widget = Static(label, classes="message-tool", markup=True)
        else:
            label = f"[bold yellow]SYSTEM[/bold yellow]{ts}\n{msg.text}"
            widget = Static(label, classes="message-system", markup=True)
        return widget

    def action_go_back(self) -> None:
        self.app.pop_screen()

    def action_scroll_down(self) -> None:
        scroll = self.query_one("#messages-scroll", VerticalScroll)
        scroll.scroll_down()

    def action_scroll_up(self) -> None:
        scroll = self.query_one("#messages-scroll", VerticalScroll)
        scroll.scroll_up()


class SessionViewerApp(App):
    """Claude Code Session Viewer."""

    TITLE = "Claude Session Viewer"

    CSS = """
    Screen {
        layout: vertical;
    }
    #breadcrumb {
        height: 1;
        background: $surface;
        color: $text-muted;
        padding: 0 1;
    }
    #project-table, #session-table {
        height: 1fr;
    }
    """

    BINDINGS = [
        Binding("q", "quit", "Quit"),
    ]

    def on_mount(self) -> None:
        projects = list_projects()
        self.push_screen(ProjectListScreen(projects))


def main() -> None:
    app = SessionViewerApp()
    app.run()


if __name__ == "__main__":
    main()
