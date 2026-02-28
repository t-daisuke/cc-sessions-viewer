use crate::models::*;
use crate::parser;
use crate::ui;

use anyhow::Result;
use chrono::Utc;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    ProjectList,
    SessionList,
    SessionDetail,
    GlobalSearch,
}

pub struct App {
    pub screen: Screen,
    pub projects: Vec<ProjectInfo>,
    pub displayed_projects: Vec<ProjectInfo>,
    pub sessions: Vec<SessionInfo>,
    pub filtered_sessions: Vec<SessionInfo>,
    pub messages: Vec<Message>,
    pub selected_project: usize,
    pub selected_session: usize,
    pub scroll_offset: usize,
    pub time_filter: TimeFilter,
    pub current_project_name: String,
    pub should_quit: bool,
    pub terminal_height: usize,
    pub search_active: bool,
    pub search_query: String,
    pub global_search_results: Vec<SearchResult>,
    pub global_search_filtered: Vec<SearchResult>,
    pub global_search_query: String,
    pub global_search_selected: usize,
    pub project_scroll_offset: usize,
    pub session_scroll_offset: usize,
    pub global_search_scroll_offset: usize,
}

fn ensure_visible(selected: usize, scroll_offset: &mut usize, visible_height: usize) {
    if visible_height == 0 {
        return;
    }
    if selected < *scroll_offset {
        *scroll_offset = selected;
    } else if selected >= *scroll_offset + visible_height {
        *scroll_offset = selected - visible_height + 1;
    }
}

impl App {
    pub fn new() -> App {
        let projects = parser::list_projects().unwrap_or_default();
        let displayed_projects = projects.clone();
        App {
            screen: Screen::ProjectList,
            projects,
            displayed_projects,
            sessions: Vec::new(),
            filtered_sessions: Vec::new(),
            messages: Vec::new(),
            selected_project: 0,
            selected_session: 0,
            scroll_offset: 0,
            time_filter: TimeFilter::All,
            current_project_name: String::new(),
            should_quit: false,
            terminal_height: 24,
            search_active: false,
            search_query: String::new(),
            global_search_results: Vec::new(),
            global_search_filtered: Vec::new(),
            global_search_query: String::new(),
            global_search_selected: 0,
            project_scroll_offset: 0,
            session_scroll_offset: 0,
            global_search_scroll_offset: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_projects(projects: Vec<ProjectInfo>) -> App {
        let displayed_projects = projects.clone();
        App {
            screen: Screen::ProjectList,
            projects,
            displayed_projects,
            sessions: Vec::new(),
            filtered_sessions: Vec::new(),
            messages: Vec::new(),
            selected_project: 0,
            selected_session: 0,
            scroll_offset: 0,
            time_filter: TimeFilter::All,
            current_project_name: String::new(),
            should_quit: false,
            terminal_height: 24,
            search_active: false,
            search_query: String::new(),
            global_search_results: Vec::new(),
            global_search_filtered: Vec::new(),
            global_search_query: String::new(),
            global_search_selected: 0,
            project_scroll_offset: 0,
            session_scroll_offset: 0,
            global_search_scroll_offset: 0,
        }
    }

    pub fn apply_filter(&mut self) {
        let now = Utc::now();
        let time_filtered: Vec<SessionInfo> = self
            .sessions
            .iter()
            .filter(|s| match self.time_filter {
                TimeFilter::All => true,
                TimeFilter::Yesterday => s
                    .timestamp
                    .map(|t| now.signed_duration_since(t).num_hours() < 24)
                    .unwrap_or(false),
                TimeFilter::Week => s
                    .timestamp
                    .map(|t| now.signed_duration_since(t).num_days() < 7)
                    .unwrap_or(false),
                TimeFilter::Month => s
                    .timestamp
                    .map(|t| now.signed_duration_since(t).num_days() < 30)
                    .unwrap_or(false),
            })
            .cloned()
            .collect();

        if self.search_query.is_empty() {
            self.filtered_sessions = time_filtered;
        } else {
            let matcher = SkimMatcherV2::default();
            self.filtered_sessions = time_filtered
                .into_iter()
                .filter(|s| {
                    matcher
                        .fuzzy_match(&s.preview, &self.search_query)
                        .is_some()
                        || matcher
                            .fuzzy_match(&s.summary, &self.search_query)
                            .is_some()
                        || matcher
                            .fuzzy_match(&s.git_branch, &self.search_query)
                            .is_some()
                })
                .collect();
        }
    }

    fn ensure_table_scroll(&mut self) {
        let th = self.terminal_height;
        match self.screen {
            Screen::ProjectList => {
                let vh = th.saturating_sub(5);
                ensure_visible(self.selected_project, &mut self.project_scroll_offset, vh);
            }
            Screen::SessionList => {
                let vh = th.saturating_sub(7);
                ensure_visible(self.selected_session, &mut self.session_scroll_offset, vh);
            }
            Screen::GlobalSearch => {
                let vh = th.saturating_sub(6);
                ensure_visible(self.global_search_selected, &mut self.global_search_scroll_offset, vh);
            }
            Screen::SessionDetail => {}
        }
    }

    pub fn enter_session_list(&mut self) {
        if self.displayed_projects.is_empty() {
            return;
        }
        let project = &self.displayed_projects[self.selected_project];
        self.current_project_name = project.dir_name.clone();
        self.search_query.clear();
        self.sessions = parser::list_sessions(&project.dir_name).unwrap_or_default();
        self.apply_filter();
        self.selected_session = 0;
        self.session_scroll_offset = 0;
        self.scroll_offset = 0;
        self.screen = Screen::SessionList;
    }

    pub fn enter_session_detail(&mut self) {
        if self.filtered_sessions.is_empty() {
            return;
        }
        let session = &self.filtered_sessions[self.selected_session];
        self.messages =
            parser::load_session(&self.current_project_name, &session.session_id)
                .unwrap_or_default();
        self.scroll_offset = 0;
        self.screen = Screen::SessionDetail;
    }

    pub fn go_back(&mut self) {
        // 検索中なら検索をキャンセル
        self.search_active = false;
        self.search_query.clear();
        match self.screen {
            Screen::ProjectList => {
                self.should_quit = true;
            }
            Screen::SessionList => {
                self.screen = Screen::ProjectList;
                self.selected_session = 0;
                self.session_scroll_offset = 0;
                self.scroll_offset = 0;
                self.displayed_projects = self.projects.clone(); // リセット
            }
            Screen::SessionDetail => {
                self.screen = Screen::SessionList;
                self.scroll_offset = 0;
            }
            Screen::GlobalSearch => {
                self.screen = Screen::ProjectList;
                self.global_search_query.clear();
                self.global_search_selected = 0;
                self.global_search_scroll_offset = 0;
            }
        }
    }

    pub fn navigate_up(&mut self) {
        match self.screen {
            Screen::ProjectList => {
                if self.selected_project > 0 {
                    self.selected_project -= 1;
                }
            }
            Screen::SessionList => {
                if self.selected_session > 0 {
                    self.selected_session -= 1;
                }
            }
            Screen::SessionDetail => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            Screen::GlobalSearch => {
                if self.global_search_selected > 0 {
                    self.global_search_selected -= 1;
                }
            }
        }
        self.ensure_table_scroll();
    }

    pub fn navigate_down(&mut self) {
        match self.screen {
            Screen::ProjectList => {
                if !self.displayed_projects.is_empty() && self.selected_project < self.displayed_projects.len() - 1 {
                    self.selected_project += 1;
                }
            }
            Screen::SessionList => {
                if !self.filtered_sessions.is_empty()
                    && self.selected_session < self.filtered_sessions.len() - 1
                {
                    self.selected_session += 1;
                }
            }
            Screen::SessionDetail => {
                self.scroll_offset += 1;
            }
            Screen::GlobalSearch => {
                if !self.global_search_filtered.is_empty()
                    && self.global_search_selected < self.global_search_filtered.len() - 1
                {
                    self.global_search_selected += 1;
                }
            }
        }
        self.ensure_table_scroll();
    }

    pub fn half_page_down(&mut self) {
        let half = self.terminal_height / 2;
        match self.screen {
            Screen::ProjectList => {
                if !self.displayed_projects.is_empty() {
                    self.selected_project =
                        (self.selected_project + half).min(self.displayed_projects.len() - 1);
                }
            }
            Screen::SessionList => {
                if !self.filtered_sessions.is_empty() {
                    self.selected_session =
                        (self.selected_session + half).min(self.filtered_sessions.len() - 1);
                }
            }
            Screen::SessionDetail => {
                self.scroll_offset += half;
            }
            Screen::GlobalSearch => {
                if !self.global_search_filtered.is_empty() {
                    self.global_search_selected = (self.global_search_selected + half)
                        .min(self.global_search_filtered.len() - 1);
                }
            }
        }
        self.ensure_table_scroll();
    }

    pub fn half_page_up(&mut self) {
        let half = self.terminal_height / 2;
        match self.screen {
            Screen::ProjectList => {
                self.selected_project = self.selected_project.saturating_sub(half);
            }
            Screen::SessionList => {
                self.selected_session = self.selected_session.saturating_sub(half);
            }
            Screen::SessionDetail => {
                self.scroll_offset = self.scroll_offset.saturating_sub(half);
            }
            Screen::GlobalSearch => {
                self.global_search_selected = self.global_search_selected.saturating_sub(half);
            }
        }
        self.ensure_table_scroll();
    }

    pub fn cycle_filter_next(&mut self) {
        self.time_filter = self.time_filter.next();
        self.apply_filter();
        self.selected_session = 0;
        self.session_scroll_offset = 0;
    }

    pub fn cycle_filter_prev(&mut self) {
        self.time_filter = self.time_filter.prev();
        self.apply_filter();
        self.selected_session = 0;
        self.session_scroll_offset = 0;
    }

    pub fn go_to_top(&mut self) {
        match self.screen {
            Screen::ProjectList => {
                self.selected_project = 0;
                self.project_scroll_offset = 0;
            }
            Screen::SessionList => {
                self.selected_session = 0;
                self.session_scroll_offset = 0;
            }
            Screen::SessionDetail => {
                self.scroll_offset = 0;
            }
            Screen::GlobalSearch => {
                self.global_search_selected = 0;
                self.global_search_scroll_offset = 0;
            }
        }
    }

    pub fn set_sessions(&mut self, sessions: Vec<SessionInfo>) {
        self.sessions = sessions;
        self.apply_filter();
        self.selected_session = 0;
        self.session_scroll_offset = 0;
        self.scroll_offset = 0;
        self.screen = Screen::SessionList;
    }

    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
        self.scroll_offset = 0;
        self.screen = Screen::SessionDetail;
    }

    pub fn go_to_bottom(&mut self) {
        match self.screen {
            Screen::ProjectList => {
                if !self.displayed_projects.is_empty() {
                    self.selected_project = self.displayed_projects.len() - 1;
                }
            }
            Screen::SessionList => {
                if !self.filtered_sessions.is_empty() {
                    self.selected_session = self.filtered_sessions.len() - 1;
                }
            }
            Screen::SessionDetail => {
                // Scroll to a large value; the UI will clamp it
                self.scroll_offset = usize::MAX / 2;
            }
            Screen::GlobalSearch => {
                if !self.global_search_filtered.is_empty() {
                    self.global_search_selected = self.global_search_filtered.len() - 1;
                }
            }
        }
        self.ensure_table_scroll();
    }

    /// 検索モードを開始（ProjectList/SessionListのみ）
    pub fn start_search(&mut self) {
        if self.screen == Screen::SessionDetail {
            return;
        }
        self.search_active = true;
        self.search_query.clear();
    }

    /// 検索をキャンセルし全リストを復元
    pub fn cancel_search(&mut self) {
        self.search_active = false;
        self.search_query.clear();
        self.apply_search();
    }

    /// 検索を確定（フィルタ結果を保持して検索モード終了）
    pub fn confirm_search(&mut self) {
        self.search_active = false;
    }

    /// 検索クエリに文字を追加
    pub fn search_push(&mut self, ch: char) {
        self.search_query.push(ch);
        self.apply_search();
    }

    /// 検索クエリから最後の文字を削除
    pub fn search_pop(&mut self) {
        self.search_query.pop();
        self.apply_search();
    }

    /// 検索フィルタを適用
    pub fn apply_search(&mut self) {
        if self.search_query.is_empty() {
            // 検索クエリが空なら全項目を表示
            self.displayed_projects = self.projects.clone();
        } else {
            let matcher = SkimMatcherV2::default();
            self.displayed_projects = self
                .projects
                .iter()
                .filter(|p| {
                    matcher
                        .fuzzy_match(&p.original_path, &self.search_query)
                        .is_some()
                })
                .cloned()
                .collect();
        }
        self.selected_project = 0;
        self.project_scroll_offset = 0;

        // SessionListの場合はfiltered_sessionsも再フィルタ
        if self.screen == Screen::SessionList {
            self.apply_filter();
            self.session_scroll_offset = 0;
        }
    }

    pub fn enter_global_search(&mut self, results: Vec<SearchResult>) {
        self.global_search_results = results.clone();
        self.global_search_filtered = results;
        self.global_search_query.clear();
        self.global_search_selected = 0;
        self.global_search_scroll_offset = 0;
        self.screen = Screen::GlobalSearch;
    }

    pub fn global_search_push(&mut self, ch: char) {
        self.global_search_query.push(ch);
        self.apply_global_search();
    }

    pub fn global_search_pop(&mut self) {
        self.global_search_query.pop();
        self.apply_global_search();
    }

    fn apply_global_search(&mut self) {
        if self.global_search_query.is_empty() {
            self.global_search_filtered = self.global_search_results.clone();
        } else {
            let query = self.global_search_query.to_lowercase();
            self.global_search_filtered = self
                .global_search_results
                .iter()
                .filter_map(|r| {
                    let mut best_prompt = String::new();
                    let mut best_indices: Vec<usize> = Vec::new();
                    let mut found = false;
                    for prompt in &r.prompts {
                        let lower = prompt.to_lowercase();
                        if let Some(byte_pos) = lower.find(&query) {
                            // byte position -> char index
                            let char_start = lower[..byte_pos].chars().count();
                            let char_len = query.chars().count();
                            best_prompt = prompt.clone();
                            best_indices = (char_start..char_start + char_len).collect();
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        // プロジェクト名・ブランチ名でもマッチを試す
                        if r.project_path.to_lowercase().contains(&query)
                            || r.git_branch.to_lowercase().contains(&query)
                        {
                            best_prompt = r.prompts.first().cloned().unwrap_or_default();
                            found = true;
                        }
                    }
                    if found {
                        let mut result = r.clone();
                        result.best_match_prompt = best_prompt;
                        result.best_match_indices = best_indices;
                        Some(result)
                    } else {
                        None
                    }
                })
                .collect();
        }
        self.global_search_selected = 0;
        self.global_search_scroll_offset = 0;
    }

    pub fn get_resume_command(&self) -> Option<String> {
        self.global_search_filtered
            .get(self.global_search_selected)
            .map(|r| format!("claude --resume {}", r.session_id))
    }
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();
}

pub fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Restore terminal on panic
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        default_panic(info);
    }));

    let mut app = App::new();

    let result = run_loop(&mut terminal, &mut app);

    restore_terminal(&mut terminal);

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|frame| {
            app.terminal_height = frame.area().height as usize;
            ui::draw(frame, app);
        })?;

        if let Event::Key(key) = event::read()? {
            if app.screen == Screen::GlobalSearch {
                match key.code {
                    KeyCode::Esc => app.go_back(),
                    KeyCode::Enter => {
                        if let Some(result) =
                            app.global_search_filtered.get(app.global_search_selected)
                        {
                            let dir_name = result.dir_name.clone();
                            let session_id = result.session_id.clone();
                            app.current_project_name = dir_name;
                            if let Ok(msgs) =
                                parser::load_session(&app.current_project_name, &session_id)
                            {
                                app.messages = msgs;
                                app.scroll_offset = 0;
                                app.screen = Screen::SessionDetail;
                            }
                        }
                    }
                    KeyCode::Char('y') => {
                        if let Some(cmd) = app.get_resume_command() {
                            let _ = cli_clipboard::set_contents(cmd);
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => app.navigate_down(),
                    KeyCode::Char('k') | KeyCode::Up => app.navigate_up(),
                    KeyCode::Char('d') => app.half_page_down(),
                    KeyCode::Char('u') => app.half_page_up(),
                    KeyCode::Char('g') => app.go_to_top(),
                    KeyCode::Char('G') => app.go_to_bottom(),
                    KeyCode::Backspace => app.global_search_pop(),
                    KeyCode::Char(c) => app.global_search_push(c),
                    _ => {}
                }
            } else if app.search_active {
                match key.code {
                    KeyCode::Esc => app.cancel_search(),
                    KeyCode::Enter => app.confirm_search(),
                    KeyCode::Backspace => app.search_pop(),
                    KeyCode::Down => app.navigate_down(),
                    KeyCode::Up => app.navigate_up(),
                    KeyCode::Char(c) => app.search_push(c),
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.go_back();
                    }
                    KeyCode::Char('/') => {
                        app.start_search();
                    }
                    KeyCode::Char('s') => {
                        if app.screen == Screen::ProjectList {
                            if let Ok(db_path) = crate::indexer::build_default_index() {
                                if let Ok(index) =
                                    crate::index::SessionIndex::open(&db_path)
                                {
                                    if let Ok(sessions) = index.search_all() {
                                        let results: Vec<SearchResult> = sessions
                                            .into_iter()
                                            .map(|s| SearchResult {
                                                session_id: s.session_id,
                                                project_path: s.project_path,
                                                dir_name: s.dir_name,
                                                git_branch: s.git_branch,
                                                created_at: s.created_at,
                                                prompts: s.prompts,
                                                best_match_prompt: String::new(),
                                                best_match_indices: Vec::new(),
                                            })
                                            .collect();
                                        app.enter_global_search(results);
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Enter => match app.screen {
                        Screen::ProjectList => app.enter_session_list(),
                        Screen::SessionList => app.enter_session_detail(),
                        Screen::SessionDetail => {}
                        Screen::GlobalSearch => {}
                    },
                    KeyCode::Char('j') | KeyCode::Down => {
                        app.navigate_down();
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        app.navigate_up();
                    }
                    KeyCode::Tab => {
                        if app.screen == Screen::SessionList {
                            app.cycle_filter_next();
                        }
                    }
                    KeyCode::BackTab => {
                        if app.screen == Screen::SessionList {
                            app.cycle_filter_prev();
                        }
                    }
                    KeyCode::Char('d') => {
                        app.half_page_down();
                    }
                    KeyCode::Char('u') => {
                        app.half_page_up();
                    }
                    KeyCode::Char('g') => {
                        app.go_to_top();
                    }
                    KeyCode::Char('G') => {
                        app.go_to_bottom();
                    }
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_project(name: &str) -> ProjectInfo {
        ProjectInfo {
            dir_name: name.to_string(),
            original_path: format!("/path/{}", name),
            session_count: 0,
        }
    }

    fn make_session(id: &str) -> SessionInfo {
        SessionInfo {
            session_id: id.to_string(),
            project_name: "test".to_string(),
            preview: format!("Preview {}", id),
            timestamp: Some(chrono::Utc::now()),
            message_count: 0,
            git_branch: String::new(),
            summary: String::new(),
        }
    }

    fn make_message(role: MessageRole, text: &str) -> Message {
        Message {
            role,
            text: text.to_string(),
            timestamp: None,
            tool_name: None,
        }
    }

    // ===== ナビゲーションテスト =====

    #[test]
    fn navigate_down_project_list() {
        let mut app = App::with_projects(vec![
            make_project("a"),
            make_project("b"),
            make_project("c"),
        ]);
        assert_eq!(app.selected_project, 0);
        app.navigate_down();
        assert_eq!(app.selected_project, 1);
        app.navigate_down();
        assert_eq!(app.selected_project, 2);
    }

    #[test]
    fn navigate_up_project_list() {
        let mut app = App::with_projects(vec![
            make_project("a"),
            make_project("b"),
            make_project("c"),
        ]);
        app.selected_project = 2;
        app.navigate_up();
        assert_eq!(app.selected_project, 1);
        app.navigate_up();
        assert_eq!(app.selected_project, 0);
    }

    #[test]
    fn navigate_down_session_list() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![
            make_session("s1"),
            make_session("s2"),
            make_session("s3"),
        ]);
        assert_eq!(app.selected_session, 0);
        app.navigate_down();
        assert_eq!(app.selected_session, 1);
        app.navigate_down();
        assert_eq!(app.selected_session, 2);
    }

    #[test]
    fn navigate_up_session_list() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![
            make_session("s1"),
            make_session("s2"),
            make_session("s3"),
        ]);
        app.selected_session = 2;
        app.navigate_up();
        assert_eq!(app.selected_session, 1);
        app.navigate_up();
        assert_eq!(app.selected_session, 0);
    }

    #[test]
    fn navigate_down_session_detail() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_messages(vec![
            make_message(MessageRole::User, "hello"),
            make_message(MessageRole::Assistant, "hi"),
        ]);
        assert_eq!(app.scroll_offset, 0);
        app.navigate_down();
        assert_eq!(app.scroll_offset, 1);
        app.navigate_down();
        assert_eq!(app.scroll_offset, 2);
    }

    #[test]
    fn navigate_up_session_detail() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_messages(vec![
            make_message(MessageRole::User, "hello"),
            make_message(MessageRole::Assistant, "hi"),
        ]);
        app.scroll_offset = 5;
        app.navigate_up();
        assert_eq!(app.scroll_offset, 4);
        app.navigate_up();
        assert_eq!(app.scroll_offset, 3);
    }

    #[test]
    fn navigate_down_empty_project_list_no_panic() {
        let mut app = App::with_projects(vec![]);
        app.navigate_down(); // should not panic
        assert_eq!(app.selected_project, 0);
    }

    #[test]
    fn navigate_down_empty_session_list_no_panic() {
        let mut app = App::with_projects(vec![]);
        app.set_sessions(vec![]);
        app.navigate_down(); // should not panic
        assert_eq!(app.selected_session, 0);
    }

    #[test]
    fn navigate_up_at_top_stays_zero() {
        let mut app = App::with_projects(vec![
            make_project("a"),
            make_project("b"),
        ]);
        assert_eq!(app.selected_project, 0);
        app.navigate_up();
        assert_eq!(app.selected_project, 0);
    }

    #[test]
    fn navigate_down_at_bottom_stays_max() {
        let mut app = App::with_projects(vec![
            make_project("a"),
            make_project("b"),
            make_project("c"),
        ]);
        app.selected_project = 2;
        app.navigate_down();
        assert_eq!(app.selected_project, 2);
    }

    #[test]
    fn navigate_up_session_list_at_top_stays_zero() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![make_session("s1"), make_session("s2")]);
        assert_eq!(app.selected_session, 0);
        app.navigate_up();
        assert_eq!(app.selected_session, 0);
    }

    #[test]
    fn navigate_down_session_list_at_bottom_stays_max() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![make_session("s1"), make_session("s2")]);
        app.selected_session = 1;
        app.navigate_down();
        assert_eq!(app.selected_session, 1);
    }

    #[test]
    fn navigate_up_session_detail_at_zero_stays_zero() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_messages(vec![make_message(MessageRole::User, "hi")]);
        assert_eq!(app.scroll_offset, 0);
        app.navigate_up();
        assert_eq!(app.scroll_offset, 0);
    }

    // ===== ハーフページテスト =====

    #[test]
    fn half_page_down_project_list() {
        let projects: Vec<_> = (0..20).map(|i| make_project(&format!("p{}", i))).collect();
        let mut app = App::with_projects(projects);
        app.terminal_height = 24;
        assert_eq!(app.selected_project, 0);
        app.half_page_down();
        assert_eq!(app.selected_project, 12); // 24/2 = 12
    }

    #[test]
    fn half_page_up_project_list() {
        let projects: Vec<_> = (0..20).map(|i| make_project(&format!("p{}", i))).collect();
        let mut app = App::with_projects(projects);
        app.terminal_height = 24;
        app.selected_project = 15;
        app.half_page_up();
        assert_eq!(app.selected_project, 3); // 15 - 12 = 3
    }

    #[test]
    fn half_page_down_session_list() {
        let mut app = App::with_projects(vec![make_project("a")]);
        let sessions: Vec<_> = (0..20).map(|i| make_session(&format!("s{}", i))).collect();
        app.set_sessions(sessions);
        app.terminal_height = 24;
        assert_eq!(app.selected_session, 0);
        app.half_page_down();
        assert_eq!(app.selected_session, 12);
    }

    #[test]
    fn half_page_up_session_list() {
        let mut app = App::with_projects(vec![make_project("a")]);
        let sessions: Vec<_> = (0..20).map(|i| make_session(&format!("s{}", i))).collect();
        app.set_sessions(sessions);
        app.terminal_height = 24;
        app.selected_session = 15;
        app.half_page_up();
        assert_eq!(app.selected_session, 3);
    }

    #[test]
    fn half_page_down_session_detail() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_messages(vec![make_message(MessageRole::User, "hi")]);
        app.terminal_height = 24;
        assert_eq!(app.scroll_offset, 0);
        app.half_page_down();
        assert_eq!(app.scroll_offset, 12);
    }

    #[test]
    fn half_page_up_session_detail() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_messages(vec![make_message(MessageRole::User, "hi")]);
        app.terminal_height = 24;
        app.scroll_offset = 20;
        app.half_page_up();
        assert_eq!(app.scroll_offset, 8); // 20 - 12 = 8
    }

    #[test]
    fn half_page_down_clamps_project_list() {
        let mut app = App::with_projects(vec![
            make_project("a"),
            make_project("b"),
            make_project("c"),
        ]);
        app.terminal_height = 24; // half = 12, but only 3 items
        app.half_page_down();
        assert_eq!(app.selected_project, 2); // clamped to max index
    }

    #[test]
    fn half_page_down_clamps_session_list() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![make_session("s1"), make_session("s2"), make_session("s3")]);
        app.terminal_height = 24;
        app.half_page_down();
        assert_eq!(app.selected_session, 2); // clamped to max index
    }

    #[test]
    fn half_page_up_clamps_at_zero() {
        let mut app = App::with_projects(vec![
            make_project("a"),
            make_project("b"),
        ]);
        app.terminal_height = 24;
        app.selected_project = 3; // even if beyond, saturating_sub handles it
        app.half_page_up();
        assert_eq!(app.selected_project, 0);
    }

    // ===== go_to_top / go_to_bottom テスト =====

    #[test]
    fn go_to_top_project_list() {
        let mut app = App::with_projects(vec![
            make_project("a"),
            make_project("b"),
            make_project("c"),
        ]);
        app.selected_project = 2;
        app.go_to_top();
        assert_eq!(app.selected_project, 0);
    }

    #[test]
    fn go_to_top_session_list() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![make_session("s1"), make_session("s2"), make_session("s3")]);
        app.selected_session = 2;
        app.go_to_top();
        assert_eq!(app.selected_session, 0);
    }

    #[test]
    fn go_to_top_session_detail() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_messages(vec![make_message(MessageRole::User, "hi")]);
        app.scroll_offset = 100;
        app.go_to_top();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn go_to_bottom_project_list() {
        let mut app = App::with_projects(vec![
            make_project("a"),
            make_project("b"),
            make_project("c"),
        ]);
        app.go_to_bottom();
        assert_eq!(app.selected_project, 2);
    }

    #[test]
    fn go_to_bottom_session_list() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![make_session("s1"), make_session("s2"), make_session("s3")]);
        app.go_to_bottom();
        assert_eq!(app.selected_session, 2);
    }

    #[test]
    fn go_to_bottom_session_detail() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_messages(vec![make_message(MessageRole::User, "hi")]);
        app.go_to_bottom();
        assert!(app.scroll_offset > 0);
    }

    #[test]
    fn go_to_top_empty_project_list_no_panic() {
        let mut app = App::with_projects(vec![]);
        app.go_to_top(); // should not panic
        assert_eq!(app.selected_project, 0);
    }

    #[test]
    fn go_to_bottom_empty_project_list_no_panic() {
        let mut app = App::with_projects(vec![]);
        app.go_to_bottom(); // should not panic
        assert_eq!(app.selected_project, 0);
    }

    #[test]
    fn go_to_top_empty_session_list_no_panic() {
        let mut app = App::with_projects(vec![]);
        app.set_sessions(vec![]);
        app.go_to_top();
        assert_eq!(app.selected_session, 0);
    }

    #[test]
    fn go_to_bottom_empty_session_list_no_panic() {
        let mut app = App::with_projects(vec![]);
        app.set_sessions(vec![]);
        app.go_to_bottom();
        assert_eq!(app.selected_session, 0);
    }

    // ===== go_back テスト =====

    #[test]
    fn go_back_from_project_list_sets_should_quit() {
        let mut app = App::with_projects(vec![make_project("a")]);
        assert_eq!(app.screen, Screen::ProjectList);
        app.go_back();
        assert!(app.should_quit);
    }

    #[test]
    fn go_back_from_session_list_to_project_list() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![make_session("s1")]);
        assert_eq!(app.screen, Screen::SessionList);
        app.selected_session = 1; // some value
        app.go_back();
        assert_eq!(app.screen, Screen::ProjectList);
        assert_eq!(app.selected_session, 0);
    }

    #[test]
    fn go_back_from_session_detail_to_session_list() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![make_session("s1")]);
        app.set_messages(vec![make_message(MessageRole::User, "hi")]);
        assert_eq!(app.screen, Screen::SessionDetail);
        app.go_back();
        assert_eq!(app.screen, Screen::SessionList);
        assert_eq!(app.scroll_offset, 0);
    }

    // ===== フィルタテスト =====

    #[test]
    fn cycle_filter_next_order() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![make_session("s1")]);
        assert_eq!(app.time_filter, TimeFilter::All);
        app.cycle_filter_next();
        assert_eq!(app.time_filter, TimeFilter::Yesterday);
        app.cycle_filter_next();
        assert_eq!(app.time_filter, TimeFilter::Week);
        app.cycle_filter_next();
        assert_eq!(app.time_filter, TimeFilter::Month);
        app.cycle_filter_next();
        assert_eq!(app.time_filter, TimeFilter::All);
    }

    #[test]
    fn cycle_filter_prev_order() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![make_session("s1")]);
        assert_eq!(app.time_filter, TimeFilter::All);
        app.cycle_filter_prev();
        assert_eq!(app.time_filter, TimeFilter::Month);
        app.cycle_filter_prev();
        assert_eq!(app.time_filter, TimeFilter::Week);
        app.cycle_filter_prev();
        assert_eq!(app.time_filter, TimeFilter::Yesterday);
        app.cycle_filter_prev();
        assert_eq!(app.time_filter, TimeFilter::All);
    }

    #[test]
    fn cycle_filter_resets_selected_session() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![make_session("s1"), make_session("s2"), make_session("s3")]);
        app.selected_session = 2;
        app.cycle_filter_next();
        assert_eq!(app.selected_session, 0);
    }

    #[test]
    fn cycle_filter_prev_resets_selected_session() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![make_session("s1"), make_session("s2"), make_session("s3")]);
        app.selected_session = 2;
        app.cycle_filter_prev();
        assert_eq!(app.selected_session, 0);
    }

    // ===== set_sessions / set_messages テスト =====

    #[test]
    fn set_sessions_updates_state() {
        let mut app = App::with_projects(vec![make_project("a")]);
        assert_eq!(app.screen, Screen::ProjectList);
        let sessions = vec![make_session("s1"), make_session("s2")];
        app.set_sessions(sessions);
        assert_eq!(app.screen, Screen::SessionList);
        assert_eq!(app.sessions.len(), 2);
        assert_eq!(app.filtered_sessions.len(), 2);
        assert_eq!(app.selected_session, 0);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn set_sessions_applies_filter() {
        let mut app = App::with_projects(vec![make_project("a")]);
        // Set filter to Yesterday; sessions with old timestamps should be filtered out
        app.time_filter = TimeFilter::Yesterday;
        let mut old_session = make_session("old");
        old_session.timestamp = Some(chrono::Utc::now() - chrono::Duration::days(10));
        let recent_session = make_session("recent");
        app.set_sessions(vec![old_session, recent_session]);
        assert_eq!(app.sessions.len(), 2);
        assert_eq!(app.filtered_sessions.len(), 1);
        assert_eq!(app.filtered_sessions[0].session_id, "recent");
    }

    #[test]
    fn set_messages_updates_state() {
        let mut app = App::with_projects(vec![make_project("a")]);
        assert_eq!(app.screen, Screen::ProjectList);
        app.scroll_offset = 10; // set some offset
        let messages = vec![
            make_message(MessageRole::User, "hello"),
            make_message(MessageRole::Assistant, "world"),
        ];
        app.set_messages(messages);
        assert_eq!(app.screen, Screen::SessionDetail);
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.scroll_offset, 0); // reset to 0
    }

    // ===== 空リスト安全性テスト =====

    #[test]
    fn empty_projects_all_operations_safe() {
        let mut app = App::with_projects(vec![]);
        // navigate
        app.navigate_down();
        app.navigate_up();
        // half page
        app.half_page_down();
        app.half_page_up();
        // go_to
        app.go_to_top();
        app.go_to_bottom();
        // go_back
        app.go_back();
        assert!(app.should_quit);
    }

    #[test]
    fn empty_sessions_all_operations_safe() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_sessions(vec![]);
        // navigate
        app.navigate_down();
        app.navigate_up();
        // half page
        app.half_page_down();
        app.half_page_up();
        // go_to
        app.go_to_top();
        app.go_to_bottom();
        // filter
        app.cycle_filter_next();
        app.cycle_filter_prev();
        // go_back
        app.go_back();
        assert_eq!(app.screen, Screen::ProjectList);
    }

    #[test]
    fn empty_messages_all_operations_safe() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_messages(vec![]);
        // navigate
        app.navigate_down();
        app.navigate_up();
        // half page
        app.half_page_down();
        app.half_page_up();
        // go_to
        app.go_to_top();
        app.go_to_bottom();
        // go_back
        app.go_back();
        assert_eq!(app.screen, Screen::SessionList);
    }

    // ===== 検索テスト =====

    #[test]
    fn start_search_activates() {
        let mut app = App::with_projects(vec![make_project("a")]);
        assert!(!app.search_active);
        app.start_search();
        assert!(app.search_active);
        assert!(app.search_query.is_empty());
    }

    #[test]
    fn cancel_search_restores_all() {
        let mut app = App::with_projects(vec![
            make_project("alpha"),
            make_project("beta"),
            make_project("gamma"),
        ]);
        app.start_search();
        app.search_push('z'); // フィルタで全て消える可能性あり
        app.cancel_search();
        assert!(!app.search_active);
        assert!(app.search_query.is_empty());
        assert_eq!(app.displayed_projects.len(), 3);
    }

    #[test]
    fn confirm_search_keeps_filter() {
        let mut app = App::with_projects(vec![
            make_project("alpha"),
            make_project("beta"),
            make_project("gamma"),
        ]);
        app.start_search();
        app.search_push('a'); // "alpha" と "gamma" にマッチ
        let filtered_count = app.displayed_projects.len();
        app.confirm_search();
        assert!(!app.search_active);
        assert_eq!(app.displayed_projects.len(), filtered_count);
    }

    #[test]
    fn search_push_filters_projects() {
        let mut app = App::with_projects(vec![
            make_project("alpha"),
            make_project("beta"),
            make_project("gamma"),
        ]);
        app.start_search();
        app.search_push('b');
        app.search_push('e');
        app.search_push('t');
        app.search_push('a');
        // "beta" にマッチするはず
        assert!(app.displayed_projects.len() <= 3);
        let has_beta = app
            .displayed_projects
            .iter()
            .any(|p| p.dir_name == "beta");
        assert!(has_beta);
    }

    #[test]
    fn search_pop_expands_results() {
        let mut app = App::with_projects(vec![
            make_project("alpha"),
            make_project("beta"),
            make_project("gamma"),
        ]);
        app.start_search();
        app.search_push('b');
        app.search_push('e');
        app.search_push('t');
        app.search_push('a');
        let narrow_count = app.displayed_projects.len();
        app.search_pop(); // "bet" に緩和
        let wider_count = app.displayed_projects.len();
        assert!(wider_count >= narrow_count);
    }

    #[test]
    fn search_on_session_detail_does_nothing() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.set_messages(vec![make_message(MessageRole::User, "hi")]);
        assert_eq!(app.screen, Screen::SessionDetail);
        app.start_search();
        assert!(!app.search_active);
    }

    #[test]
    fn search_resets_selected_project() {
        let mut app = App::with_projects(vec![
            make_project("alpha"),
            make_project("beta"),
            make_project("gamma"),
        ]);
        app.selected_project = 2;
        app.start_search();
        app.search_push('a');
        assert_eq!(app.selected_project, 0);
    }

    #[test]
    fn navigate_with_search_uses_displayed_projects() {
        let mut app = App::with_projects(vec![
            make_project("alpha"),
            make_project("beta"),
            make_project("gamma"),
        ]);
        app.start_search();
        app.search_push('a'); // "alpha" と "gamma" にマッチ (original_path: /path/alpha, /path/gamma)
        let count = app.displayed_projects.len();
        // 最下端までナビゲート
        for _ in 0..count + 5 {
            app.navigate_down();
        }
        // displayed_projects のサイズを超えないこと
        assert!(app.selected_project < count);
    }

    // ===== GlobalSearch テスト =====

    fn make_search_result(id: &str, prompts: Vec<&str>) -> SearchResult {
        SearchResult {
            session_id: id.to_string(),
            project_path: format!("/path/{}", id),
            dir_name: format!("dir-{}", id),
            git_branch: "main".to_string(),
            created_at: "2026-01-15T10:00:00Z".to_string(),
            prompts: prompts.into_iter().map(String::from).collect(),
            best_match_prompt: String::new(),
            best_match_indices: Vec::new(),
        }
    }

    #[test]
    fn enter_global_search_from_project_list() {
        let mut app = App::with_projects(vec![make_project("a")]);
        assert_eq!(app.screen, Screen::ProjectList);
        app.enter_global_search(vec![]);
        assert_eq!(app.screen, Screen::GlobalSearch);
    }

    #[test]
    fn global_search_go_back_returns_to_project_list() {
        let mut app = App::with_projects(vec![make_project("a")]);
        app.enter_global_search(vec![]);
        assert_eq!(app.screen, Screen::GlobalSearch);
        app.go_back();
        assert_eq!(app.screen, Screen::ProjectList);
    }

    #[test]
    fn global_search_fuzzy_filter() {
        let mut app = App::with_projects(vec![make_project("a")]);
        let searchable = vec![
            make_search_result("s1", vec!["JWT認証の実装", "テスト書いて"]),
            make_search_result("s2", vec!["デプロイの設定"]),
        ];
        app.enter_global_search(searchable);
        app.global_search_push('認');
        app.global_search_push('証');
        assert!(app.global_search_filtered.iter().any(|r| r.session_id == "s1"));
    }

    #[test]
    fn global_search_navigate() {
        let mut app = App::with_projects(vec![make_project("a")]);
        let searchable = vec![
            make_search_result("s1", vec!["a"]),
            make_search_result("s2", vec!["b"]),
        ];
        app.enter_global_search(searchable);
        assert_eq!(app.global_search_selected, 0);
        app.navigate_down();
        assert_eq!(app.global_search_selected, 1);
        app.navigate_up();
        assert_eq!(app.global_search_selected, 0);
    }

    #[test]
    fn global_search_copy_resume_cmd() {
        let mut app = App::with_projects(vec![make_project("a")]);
        let searchable = vec![
            make_search_result("abc-123-def", vec!["hello"]),
        ];
        app.enter_global_search(searchable);
        let cmd = app.get_resume_command();
        assert_eq!(cmd, Some("claude --resume abc-123-def".to_string()));
    }

    #[test]
    fn search_filters_sessions_by_preview() {
        let mut app = App::with_projects(vec![make_project("a")]);
        let mut s1 = make_session("s1");
        s1.preview = "Fix authentication bug".to_string();
        let mut s2 = make_session("s2");
        s2.preview = "Add new feature".to_string();
        let mut s3 = make_session("s3");
        s3.preview = "Update documentation".to_string();
        app.set_sessions(vec![s1, s2, s3]);

        app.start_search();
        app.search_push('a');
        app.search_push('u');
        app.search_push('t');
        app.search_push('h');

        // "authentication" を含む s1 がマッチするはず
        let has_auth = app
            .filtered_sessions
            .iter()
            .any(|s| s.session_id == "s1");
        assert!(has_auth);
    }
}
