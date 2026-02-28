use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap},
};

use crate::app::{App, Screen};
use crate::models::*;

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());

    // Title bar
    let title = Paragraph::new(Line::from(vec![Span::styled(
        " Claude Session Viewer",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]));
    frame.render_widget(title, chunks[0]);

    // Help bar
    if app.search_active {
        // 検索バー表示
        let search_line = Line::from(vec![
            Span::styled(" /", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(&app.search_query, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Cyan)), // カーソル
        ]);
        let search_bar = Paragraph::new(search_line);
        frame.render_widget(search_bar, chunks[2]);
    } else {
        let help_text = match app.screen {
            Screen::ProjectList => "Enter: Open  s: Global Search  q: Quit  j/k: Navigate  /: Filter",
            Screen::SessionList => "Enter: Open  Esc: Back  j/k: Navigate  d/u: Half Page  Tab: Filter  /: Search",
            Screen::SessionDetail => "Esc: Back  j/k: Scroll  d/u: Half Page  g/G: Top/Bottom",
            Screen::GlobalSearch => "Enter: Detail  y: Copy resume cmd  Esc: Back  j/k: Navigate",
        };
        let help = Paragraph::new(Line::from(vec![Span::styled(
            help_text,
            Style::default().fg(Color::DarkGray),
        )]));
        frame.render_widget(help, chunks[2]);
    }

    // Screen content
    match app.screen {
        Screen::ProjectList => draw_project_list(frame, app, chunks[1]),
        Screen::SessionList => draw_session_list(frame, app, chunks[1]),
        Screen::SessionDetail => draw_session_detail(frame, app, chunks[1]),
        Screen::GlobalSearch => draw_global_search(frame, app, chunks[1]),
    }
}

fn draw_project_list(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let header = Row::new(vec![
        Cell::from("Project Path"),
        Cell::from("Sessions"),
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    // borders(2) + header(1) = 3
    let visible_height = (area.height as usize).saturating_sub(3);

    let rows: Vec<Row> = app
        .displayed_projects
        .iter()
        .enumerate()
        .skip(app.project_scroll_offset)
        .take(visible_height)
        .map(|(i, project)| {
            let style = if i == app.selected_project {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };
            Row::new(vec![
                Cell::from(project.original_path.clone()),
                Cell::from(project.session_count.to_string()),
            ])
            .style(style)
        })
        .collect();

    let title = if app.search_query.is_empty() {
        " Projects ".to_string()
    } else {
        format!(" Projects ({} matches) ", app.displayed_projects.len())
    };

    let table = Table::new(
        rows,
        [Constraint::Percentage(70), Constraint::Percentage(30)],
    )
    .header(header)
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(table, area);
}

fn draw_session_list(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let inner_chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(area);

    // Breadcrumb
    let breadcrumb = Paragraph::new(Line::from(vec![Span::styled(
        format!(" Project: {}", app.current_project_name),
        Style::default().fg(Color::DarkGray),
    )]));
    frame.render_widget(breadcrumb, inner_chunks[0]);

    // Filter tabs
    let filter_labels: Vec<String> = TimeFilter::all_filters()
        .iter()
        .map(|f| f.label().to_string())
        .collect();
    let selected_index = TimeFilter::all_filters()
        .iter()
        .position(|f| *f == app.time_filter)
        .unwrap_or(0);
    let tabs = Tabs::new(filter_labels)
        .select(selected_index)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, inner_chunks[1]);

    // Session table
    let header = Row::new(vec![
        Cell::from("Timestamp"),
        Cell::from("Msgs"),
        Cell::from("Branch"),
        Cell::from("Preview"),
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    // borders(2) + header(1) = 3
    let visible_height = (inner_chunks[2].height as usize).saturating_sub(3);

    let rows: Vec<Row> = app
        .filtered_sessions
        .iter()
        .enumerate()
        .skip(app.session_scroll_offset)
        .take(visible_height)
        .map(|(i, session)| {
            let style = if i == app.selected_session {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };
            let preview = if session.preview.chars().count() > 80 {
                let truncated: String = session.preview.chars().take(80).collect();
                format!("{}...", truncated)
            } else {
                session.preview.clone()
            }
            .replace('\n', " ");
            Row::new(vec![
                Cell::from(session.timestamp_str()),
                Cell::from(session.message_count.to_string()),
                Cell::from(session.git_branch.clone()),
                Cell::from(preview),
            ])
            .style(style)
        })
        .collect();

    let title = if app.search_query.is_empty() {
        " Sessions ".to_string()
    } else {
        format!(" Sessions ({} matches) ", app.filtered_sessions.len())
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(8),
            Constraint::Percentage(20),
            Constraint::Percentage(52),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(table, inner_chunks[2]);
}

fn draw_session_detail(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let inner_chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(area);

    // Breadcrumb
    let session_id_short = app
        .filtered_sessions
        .get(app.selected_session)
        .map(|s| &s.session_id[..s.session_id.len().min(8)])
        .unwrap_or("unknown");
    let breadcrumb = Paragraph::new(Line::from(vec![Span::styled(
        format!(" Session: {}", session_id_short),
        Style::default().fg(Color::DarkGray),
    )]));
    frame.render_widget(breadcrumb, inner_chunks[0]);

    // Messages
    let mut lines: Vec<Line> = Vec::new();

    for (i, msg) in app.messages.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }

        let role_color = match msg.role {
            MessageRole::User => Color::Cyan,
            MessageRole::Assistant => Color::Green,
            MessageRole::System => Color::Yellow,
            MessageRole::ToolUse => Color::Yellow,
            MessageRole::ToolResult => Color::Magenta,
            MessageRole::Progress => Color::DarkGray,
        };

        let ts = msg.timestamp_str();
        let mut header_spans = vec![Span::styled(
            msg.role_label(),
            Style::default()
                .fg(role_color)
                .add_modifier(Modifier::BOLD),
        )];
        if !ts.is_empty() {
            header_spans.push(Span::raw(" "));
            header_spans.push(Span::styled(ts, Style::default().fg(Color::DarkGray)));
        }
        lines.push(Line::from(header_spans));

        let text_color = match msg.role {
            MessageRole::ToolUse | MessageRole::ToolResult => Color::DarkGray,
            _ => Color::White,
        };

        for text_line in msg.text.lines() {
            lines.push(Line::from(Span::styled(
                text_line.to_string(),
                Style::default().fg(text_color),
            )));
        }
    }

    let paragraph = Paragraph::new(lines)
        .scroll((app.scroll_offset as u16, 0))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    frame.render_widget(paragraph, inner_chunks[1]);
}

fn draw_global_search(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let inner_chunks = Layout::vertical([
        Constraint::Length(1), // search input
        Constraint::Min(0),   // results
    ])
    .split(area);

    // Search input
    let search_line = Line::from(vec![
        Span::styled(
            " Search: ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&app.global_search_query, Style::default().fg(Color::White)),
        Span::styled("█", Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(Paragraph::new(search_line), inner_chunks[0]);

    // Results table
    let header = Row::new(vec![
        Cell::from("Time"),
        Cell::from("Project"),
        Cell::from("Branch"),
        Cell::from("Prompt"),
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    // borders(2) + header(1) = 3
    let visible_height = (inner_chunks[1].height as usize).saturating_sub(3);

    let rows: Vec<Row> = app
        .global_search_filtered
        .iter()
        .enumerate()
        .skip(app.global_search_scroll_offset)
        .take(visible_height)
        .map(|(i, result)| {
            let style = if i == app.global_search_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };

            let time_str = format_relative_time(&result.created_at);

            let project_short = result
                .project_path
                .rsplit('/')
                .next()
                .unwrap_or(&result.project_path);

            let prompt = if result.best_match_prompt.is_empty() {
                result.prompts.first().cloned().unwrap_or_default()
            } else {
                result.best_match_prompt.clone()
            };
            let prompt = prompt.replace('\n', " ");
            let prompt_line = build_match_snippet(&prompt, &result.best_match_indices, 60);

            Row::new(vec![
                Cell::from(time_str),
                Cell::from(project_short.to_string()),
                Cell::from(result.git_branch.clone()),
                Cell::from(prompt_line),
            ])
            .style(style)
        })
        .collect();

    let title = format!(
        " Global Search ({} results) ",
        app.global_search_filtered.len()
    );
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(12),
            Constraint::Percentage(20),
            Constraint::Percentage(18),
            Constraint::Percentage(50),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(table, inner_chunks[1]);
}

fn build_match_snippet<'a>(prompt: &str, indices: &[usize], max_width: usize) -> Line<'a> {
    let chars: Vec<char> = prompt.chars().collect();
    let prompt_len = chars.len();

    if indices.is_empty() || prompt_len == 0 {
        let display: String = chars.iter().take(max_width).collect();
        if prompt_len > max_width {
            return Line::from(format!("{}...", display));
        }
        return Line::from(display);
    }

    let first_match = *indices.first().unwrap();
    let last_match = *indices.last().unwrap();
    let match_center = (first_match + last_match) / 2;

    let half_width = max_width / 2;
    let start = if match_center > half_width {
        (match_center - half_width).min(prompt_len.saturating_sub(max_width))
    } else {
        0
    };
    let end = (start + max_width).min(prompt_len);

    let match_set: std::collections::HashSet<usize> = indices.iter().copied().collect();

    let mut spans: Vec<Span> = Vec::new();
    if start > 0 {
        spans.push(Span::styled("...", Style::default().fg(Color::DarkGray)));
    }

    let mut current_text = String::new();
    let mut current_is_match = false;

    for i in start..end {
        let is_match = match_set.contains(&i);
        if is_match != current_is_match && !current_text.is_empty() {
            let style = if current_is_match {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            spans.push(Span::styled(std::mem::take(&mut current_text), style));
        }
        current_text.push(chars[i]);
        current_is_match = is_match;
    }

    if !current_text.is_empty() {
        let style = if current_is_match {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(Span::styled(current_text, style));
    }

    if end < prompt_len {
        spans.push(Span::styled("...", Style::default().fg(Color::DarkGray)));
    }

    Line::from(spans)
}

pub fn format_relative_time(iso: &str) -> String {
    use chrono::{DateTime, Utc};
    let dt: DateTime<Utc> = match iso.parse() {
        Ok(d) => d,
        Err(_) => return iso.to_string(),
    };
    let now = Utc::now();
    let dur = now.signed_duration_since(dt);
    if dur.num_hours() < 24 {
        dt.format("%H:%M").to_string()
    } else if dur.num_days() < 7 {
        format!("{} days ago", dur.num_days())
    } else {
        dt.format("%b %d").to_string()
    }
}
