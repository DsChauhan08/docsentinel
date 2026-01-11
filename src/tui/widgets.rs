//! Custom TUI widgets

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// A diff view widget showing side-by-side comparison
pub struct DiffView<'a> {
    left_title: &'a str,
    left_content: &'a str,
    right_title: &'a str,
    right_content: &'a str,
}

impl<'a> DiffView<'a> {
    pub fn new(
        left_title: &'a str,
        left_content: &'a str,
        right_title: &'a str,
        right_content: &'a str,
    ) -> Self {
        Self {
            left_title,
            left_content,
            right_title,
            right_content,
        }
    }
}

impl<'a> Widget for DiffView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Split area in half
        let mid = area.width / 2;

        let left_area = Rect {
            x: area.x,
            y: area.y,
            width: mid,
            height: area.height,
        };

        let right_area = Rect {
            x: area.x + mid,
            y: area.y,
            width: area.width - mid,
            height: area.height,
        };

        // Render left side
        let left = Paragraph::new(self.left_content)
            .block(
                Block::default()
                    .title(self.left_title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            )
            .wrap(Wrap { trim: false });
        left.render(left_area, buf);

        // Render right side
        let right = Paragraph::new(self.right_content)
            .block(
                Block::default()
                    .title(self.right_title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            )
            .wrap(Wrap { trim: false });
        right.render(right_area, buf);
    }
}

/// A severity badge widget
pub struct SeverityBadge {
    severity: crate::drift::DriftSeverity,
}

impl SeverityBadge {
    pub fn new(severity: crate::drift::DriftSeverity) -> Self {
        Self { severity }
    }

    pub fn to_span(&self) -> Span<'static> {
        use crate::drift::DriftSeverity;

        let (text, color) = match self.severity {
            DriftSeverity::Critical => ("CRITICAL", Color::Red),
            DriftSeverity::High => ("HIGH", Color::LightRed),
            DriftSeverity::Medium => ("MEDIUM", Color::Yellow),
            DriftSeverity::Low => ("LOW", Color::Green),
        };

        Span::styled(
            format!(" {} ", text),
            Style::default()
                .fg(Color::White)
                .bg(color)
                .add_modifier(Modifier::BOLD),
        )
    }
}

/// A progress bar widget
pub struct ProgressBar {
    progress: f64,
    label: Option<String>,
}

impl ProgressBar {
    pub fn new(progress: f64) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            label: None,
        }
    }

    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }
}

impl Widget for ProgressBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 3 || area.height < 1 {
            return;
        }

        let filled = ((area.width as f64 - 2.0) * self.progress) as u16;

        // Draw border
        buf.set_string(area.x, area.y, "[", Style::default());
        buf.set_string(area.x + area.width - 1, area.y, "]", Style::default());

        // Draw filled portion
        for x in 0..filled {
            buf.set_string(
                area.x + 1 + x,
                area.y,
                "█",
                Style::default().fg(Color::Green),
            );
        }

        // Draw empty portion
        for x in filled..(area.width - 2) {
            buf.set_string(
                area.x + 1 + x,
                area.y,
                "░",
                Style::default().fg(Color::DarkGray),
            );
        }

        // Draw label if present
        if let Some(label) = self.label {
            let label_x = area.x + (area.width - label.len() as u16) / 2;
            buf.set_string(label_x, area.y, &label, Style::default().fg(Color::White));
        }
    }
}

/// A code block widget with syntax highlighting hints
pub struct CodeBlock<'a> {
    content: &'a str,
    language: Option<&'a str>,
    line_numbers: bool,
}

impl<'a> CodeBlock<'a> {
    pub fn new(content: &'a str) -> Self {
        Self {
            content,
            language: None,
            line_numbers: false,
        }
    }

    pub fn language(mut self, lang: &'a str) -> Self {
        self.language = Some(lang);
        self
    }

    pub fn line_numbers(mut self, show: bool) -> Self {
        self.line_numbers = show;
        self
    }
}

impl<'a> Widget for CodeBlock<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = self
            .content
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if self.line_numbers {
                    Line::from(vec![
                        Span::styled(
                            format!("{:4} │ ", i + 1),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::raw(line),
                    ])
                } else {
                    Line::from(line)
                }
            })
            .collect();

        let title = self
            .language
            .map(|l| format!(" {} ", l))
            .unwrap_or_default();

        let block = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::White));

        block.render(area, buf);
    }
}

/// A key hint widget for showing keyboard shortcuts
pub struct KeyHints<'a> {
    hints: Vec<(&'a str, &'a str)>,
}

impl<'a> KeyHints<'a> {
    pub fn new(hints: Vec<(&'a str, &'a str)>) -> Self {
        Self { hints }
    }
}

impl<'a> Widget for KeyHints<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let spans: Vec<Span> = self
            .hints
            .iter()
            .flat_map(|(key, desc)| {
                vec![
                    Span::styled(
                        format!("[{}]", key),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(" {} ", desc)),
                ]
            })
            .collect();

        let line = Line::from(spans);
        let paragraph = Paragraph::new(line).style(Style::default().fg(Color::DarkGray));

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar() {
        let bar = ProgressBar::new(0.5).with_label("50%");
        // Just ensure it doesn't panic
        assert!(bar.progress >= 0.0 && bar.progress <= 1.0);
    }

    #[test]
    fn test_severity_badge() {
        use crate::drift::DriftSeverity;

        let badge = SeverityBadge::new(DriftSeverity::Critical);
        let span = badge.to_span();
        assert!(span.content.contains("CRITICAL"));
    }
}
