//! Header and footer rendering functions

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::models::{App, View};

pub fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let title = match app.current_view {
        View::WorkflowList => "Workflow Manager v0.2.0 - Workflows",
        View::WorkflowDetail(_) => "Workflow Manager v0.2.0 - Workflow Detail",
        View::WorkflowEdit(_) => "Workflow Manager v0.2.0 - Configure Workflow",
        View::WorkflowRunning(_) => "Workflow Manager v0.2.0 - Running Workflow",
        View::Tabs => "Workflow Manager v0.2.0 - Running Workflows",
        View::Chat => "Workflow Manager v0.2.0 - AI Chat",
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("      "),
        Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("uit"),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, area);
}

pub fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let footer_text = match app.current_view {
        View::WorkflowList => Line::from(vec![
            Span::styled("[↑↓]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Navigate  "),
            Span::styled("[Enter/V]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" View  "),
            Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ]),
        View::WorkflowDetail(_) => Line::from(vec![
            Span::styled("[E]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Edit  "),
            Span::styled("[Esc/B]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Back  "),
            Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ]),
        View::WorkflowEdit(_) => {
            if app.is_editing {
                Line::from(vec![
                    Span::styled(
                        "TYPE",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" to edit  "),
                    Span::styled("[Enter]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Save  "),
                    Span::styled("[Esc]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Cancel  "),
                    Span::styled("[Backspace]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Delete"),
                ])
            } else {
                Line::from(vec![
                    Span::styled("[↑↓]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Navigate  "),
                    Span::styled("[Enter]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Edit  "),
                    Span::styled("[L]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Launch  "),
                    Span::styled("[Esc/B]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Back  "),
                    Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" Quit"),
                ])
            }
        }
        View::WorkflowRunning(_) => Line::from(vec![
            Span::styled("[↑↓/jk]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Navigate  "),
            Span::styled("[Enter]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Expand/Collapse  "),
            Span::styled("[Space]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Toggle All  "),
            Span::styled("[Esc/B]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Back  "),
            Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ]),
        View::Tabs => Line::from(vec![
            Span::styled("[↑↓/jk]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Navigate  "),
            Span::styled("[←→/hl]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Scroll Agent  "),
            Span::styled("[Enter]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Expand  "),
            Span::styled("[Space]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Toggle All  "),
            Span::styled("[Tab]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Switch  "),
            Span::styled("[E]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Edit  "),
            Span::styled("[R]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Rerun  "),
            Span::styled("[C]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Close  "),
            Span::styled("[A]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" AI Chat  "),
            Span::styled("[Q]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ]),
        View::Chat => Line::from(vec![
            Span::styled("[↑↓]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Scroll  "),
            Span::styled("[Enter]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Send  "),
            Span::styled("[Esc]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Back  "),
            Span::styled("[Ctrl+Q]", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" Quit"),
        ]),
    };

    let footer = Paragraph::new(footer_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, area);
}
