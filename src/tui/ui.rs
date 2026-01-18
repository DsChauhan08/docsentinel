//! TUI rendering

use super::app::{App, View};
use crate::drift::DriftSeverity;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Draw the UI
pub fn draw(f: &mut Frame, app: &App) {
    match app.state.view {
        View::Dashboard => draw_dashboard(f, app),
        View::Issues => draw_issues(f, app),
        View::IssueDetail => draw_detail(f, app),
        View::FixEditor => draw_editor(f, app),
        View::Docs => draw_docs(f, app),
        View::Help => draw_help(f, app),
    }

    // Draw status message if present
    if let Some(ref msg) = app.state.status_message {
        draw_status(f, msg);
    }

    // Draw confirmation dialog if present
    if let Some(ref dialog) = app.state.confirm_dialog {
        draw_confirm(f, &dialog.title, &dialog.message);
    }
}

/// Draw the dashboard view
fn draw_dashboard(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new("DocSentinel")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Stats
    let stats_text = vec![
        Line::from(vec![
            Span::raw("Repository: "),
            Span::styled(
                app.repo_path.display().to_string(),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Code chunks: "),
            Span::styled(
                app.stats.code_chunks.to_string(),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::raw("Doc chunks: "),
            Span::styled(
                app.stats.doc_chunks.to_string(),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::raw("Pending issues: "),
            Span::styled(
                app.stats.pending_events.to_string(),
                Style::default().fg(if app.stats.pending_events > 0 {
                    Color::Red
                } else {
                    Color::Green
                }),
            ),
        ]),
    ];

    let stats = Paragraph::new(stats_text)
        .block(Block::default().title("Statistics").borders(Borders::ALL));
    f.render_widget(stats, chunks[1]);

    // Issue summary
    let mut critical = 0;
    let mut high = 0;
    let mut medium = 0;
    let mut low = 0;

    for event in &app.events {
        match event.severity {
            DriftSeverity::Critical => critical += 1,
            DriftSeverity::High => high += 1,
            DriftSeverity::Medium => medium += 1,
            DriftSeverity::Low => low += 1,
        }
    }

    let summary_text = vec![
        Line::from(vec![
            Span::styled("🔴 Critical: ", Style::default().fg(Color::Red)),
            Span::raw(critical.to_string()),
        ]),
        Line::from(vec![
            Span::styled("🟠 High: ", Style::default().fg(Color::LightRed)),
            Span::raw(high.to_string()),
        ]),
        Line::from(vec![
            Span::styled("🟡 Medium: ", Style::default().fg(Color::Yellow)),
            Span::raw(medium.to_string()),
        ]),
        Line::from(vec![
            Span::styled("🟢 Low: ", Style::default().fg(Color::Green)),
            Span::raw(low.to_string()),
        ]),
    ];

    let summary = Paragraph::new(summary_text).block(
        Block::default()
            .title("Issues by Severity")
            .borders(Borders::ALL),
    );
    f.render_widget(summary, chunks[2]);

    // Help
    let help = Paragraph::new("[i] Issues  [d] Docs  [s] Scan  [?] Help  [q] Quit")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[3]);
}

/// Draw the issues list view
fn draw_issues(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new(format!("Drift Issues ({})", app.events.len()))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Issues list
    let items: Vec<ListItem> = app
        .events
        .iter()
        .enumerate()
        .map(|(i, event)| {
            let severity_color = App::severity_color(event.severity);
            let severity_icon = match event.severity {
                DriftSeverity::Critical => "🔴",
                DriftSeverity::High => "🟠",
                DriftSeverity::Medium => "🟡",
                DriftSeverity::Low => "🟢",
            };

            let content = Line::from(vec![
                Span::raw(severity_icon),
                Span::raw(" "),
                Span::styled(
                    format!("[{}]", event.severity),
                    Style::default().fg(severity_color),
                ),
                Span::raw(" "),
                Span::raw(&event.description),
            ]);

            let style = if i == app.state.selected_issue {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title("Issues").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(list, chunks[1]);

    // Help
    let help = Paragraph::new("[↑/↓] Navigate  [Enter] Details  [f] Fix  [x] Ignore  [Esc] Back")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
}

/// Draw the issue detail view
fn draw_detail(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    if let Some(event) = app.selected_event() {
        // Title
        let title = Paragraph::new(format!("Issue: {}", &event.id[..8]))
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, chunks[0]);

        // Details
        let severity_color = App::severity_color(event.severity);

        let detail_text = vec![
            Line::from(vec![
                Span::styled("Severity: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{}", event.severity),
                    Style::default().fg(severity_color),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Description: ",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from(event.description.clone()),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Evidence: ",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from(event.evidence.clone()),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Confidence: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("{:.0}%", event.confidence * 100.0)),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Related Code: ",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
        ];

        let mut lines = detail_text;
        for chunk_id in &event.related_code_chunks {
            lines.push(Line::from(format!("  • {}", chunk_id)));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Related Docs: ",
            Style::default().add_modifier(Modifier::BOLD),
        )]));
        for chunk_id in &event.related_doc_chunks {
            lines.push(Line::from(format!("  • {}", chunk_id)));
        }

        if let Some(ref fix) = event.suggested_fix {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Suggested Fix: ",
                Style::default().add_modifier(Modifier::BOLD),
            )]));
            for line in fix.lines() {
                lines.push(Line::from(format!("  {}", line)));
            }
        }

        let details = Paragraph::new(lines)
            .block(Block::default().title("Details").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        f.render_widget(details, chunks[1]);
    }

    // Help
    let help = Paragraph::new("[f] Fix  [x] Ignore  [↑/↓] Scroll  [Esc] Back")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
}

/// Draw the fix editor view
fn draw_editor(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(5),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new("Fix Editor")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Current documentation
    if let Some(event) = app.selected_event() {
        let current = Paragraph::new(event.evidence.clone())
            .block(Block::default().title("Current").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        f.render_widget(current, chunks[1]);

        // Fix content
        let fix_content = if !app.state.input_buffer.is_empty() {
            app.state.input_buffer.clone()
        } else {
            event
                .suggested_fix
                .clone()
                .unwrap_or_else(|| "No suggested fix available".to_string())
        };

        let fix_style = if app.state.input_mode {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let fix = Paragraph::new(fix_content)
            .style(fix_style)
            .block(
                Block::default()
                    .title(if app.state.input_mode {
                        "Fix (editing)"
                    } else {
                        "Fix"
                    })
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(fix, chunks[2]);
    }

    // Help
    let help = Paragraph::new("[e] Edit  [a] Apply  [Esc] Cancel")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[3]);
}

/// Draw the docs browser view
fn draw_docs(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new("Documentation Browser")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Search bar
    let search_style = if app.state.input_mode {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let search_text = if app.state.search_query.is_empty() {
        if app.state.input_mode {
            "Type to search...".to_string()
        } else {
            "Press / to search".to_string()
        }
    } else {
        app.state.search_query.clone()
    };
    let search = Paragraph::new(search_text)
        .style(search_style)
        .block(Block::default().title("Search").borders(Borders::ALL));
    f.render_widget(search, chunks[1]);

    // Filter chunks based on search query
    let filtered_chunks: Vec<_> = if app.state.search_query.is_empty() {
        app.code_chunks.iter().filter(|c| c.is_public).collect()
    } else {
        let query = app.state.search_query.to_lowercase();
        app.code_chunks
            .iter()
            .filter(|c| c.is_public)
            .filter(|c| {
                c.symbol_name.to_lowercase().contains(&query)
                    || c.file_path.to_lowercase().contains(&query)
            })
            .collect()
    };

    // Symbols list
    let items: Vec<ListItem> = filtered_chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let content = Line::from(vec![
                Span::styled(
                    format!("{} ", chunk.symbol_type),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    &chunk.symbol_name,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({})", chunk.file_path),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            let style = if i == app.state.selected_doc {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list_title = format!(
        "Symbols ({} of {})",
        filtered_chunks.len(),
        app.code_chunks.iter().filter(|c| c.is_public).count()
    );
    let list = List::new(items).block(Block::default().title(list_title).borders(Borders::ALL));
    f.render_widget(list, chunks[2]);

    // Help
    let help = Paragraph::new("[↑/↓] Navigate  [/] Search  [g/G] Top/Bottom  [Esc] Back")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[3]);
}

/// Draw the help view
fn draw_help(f: &mut Frame, _app: &App) {
    let area = centered_rect(60, 80, f.area());

    f.render_widget(Clear, area);

    let help_text = vec![
        Line::from(Span::styled(
            "DocSentinel Help",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Global",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        Line::from("  Ctrl+C, Ctrl+Q  Quit"),
        Line::from("  ?               Show help"),
        Line::from(""),
        Line::from(Span::styled(
            "Dashboard",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        Line::from("  i, Enter        View issues"),
        Line::from("  s               Run scan"),
        Line::from("  q               Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "Issues List",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        Line::from("  ↑/k, ↓/j        Navigate"),
        Line::from("  Enter           View details"),
        Line::from("  f               Open fix editor"),
        Line::from("  x               Ignore issue"),
        Line::from("  Esc             Back to dashboard"),
        Line::from(""),
        Line::from(Span::styled(
            "Fix Editor",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        Line::from("  e               Edit fix"),
        Line::from("  a               Apply fix"),
        Line::from("  Esc             Cancel"),
        Line::from(""),
        Line::from("Press any key to close"),
    ];

    let help = Paragraph::new(help_text)
        .block(Block::default().title("Help").borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    f.render_widget(help, area);
}

/// Draw status message
fn draw_status(f: &mut Frame, message: &str) {
    let area = Rect {
        x: 0,
        y: f.area().height - 1,
        width: f.area().width,
        height: 1,
    };

    let status =
        Paragraph::new(message).style(Style::default().fg(Color::Yellow).bg(Color::DarkGray));

    f.render_widget(status, area);
}

/// Draw confirmation dialog
fn draw_confirm(f: &mut Frame, title: &str, message: &str) {
    let area = centered_rect(50, 30, f.area());

    f.render_widget(Clear, area);

    let text = vec![
        Line::from(message),
        Line::from(""),
        Line::from("[y] Yes  [n] No"),
    ];

    let dialog = Paragraph::new(text)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    f.render_widget(dialog, area);
}

/// Create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
