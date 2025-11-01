//! Notification rendering for user-visible feedback

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, NotificationLevel};

/// Render active notifications as an overlay at the bottom of the screen
pub fn render_notifications(f: &mut Frame, app: &App, area: Rect) {
    let notifications = app.notifications.get_active();

    if notifications.is_empty() {
        return;
    }

    // Take bottom 3 lines per notification (max 3 notifications visible)
    let notification_height = (notifications.len() * 3).min(9);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(notification_height as u16),
        ])
        .split(area);

    let notification_area = chunks[1];

    // Render each notification
    let notification_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            notifications
                .iter()
                .take(3)
                .map(|_| Constraint::Length(3))
                .collect::<Vec<_>>(),
        )
        .split(notification_area);

    for (idx, notification) in notifications.iter().take(3).enumerate() {
        let (bg_color, fg_color, icon) = match notification.level {
            NotificationLevel::Error => (Color::Red, Color::White, "✗"),
            NotificationLevel::Warning => (Color::Yellow, Color::Black, "⚠"),
            NotificationLevel::Info => (Color::Blue, Color::White, "ℹ"),
            NotificationLevel::Success => (Color::Green, Color::White, "✓"),
        };

        let text = vec![
            Line::from(vec![Span::styled(
                format!("{} {} ", icon, notification.title),
                Style::default()
                    .fg(fg_color)
                    .bg(bg_color)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(notification.message.clone()),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(bg_color));

        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });

        f.render_widget(paragraph, notification_chunks[idx]);
    }
}
