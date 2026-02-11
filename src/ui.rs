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
            Screen::ProjectList => "Enter: Open  q: Quit  j/k: Navigate  d/u: Half Page  /: Search",
            Screen::SessionList => "Enter: Open  Esc: Back  j/k: Navigate  d/u: Half Page  Tab: Filter  /: Search",
            Screen::SessionDetail => "Esc: Back  j/k: Scroll  d/u: Half Page  g/G: Top/Bottom",
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

    let rows: Vec<Row> = app
        .displayed_projects
        .iter()
        .enumerate()
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

    let rows: Vec<Row> = app
        .filtered_sessions
        .iter()
        .enumerate()
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
