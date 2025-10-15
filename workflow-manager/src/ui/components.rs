//! Reusable UI components (dropdowns, file browser, helpers)

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};
use std::path::PathBuf;

use crate::models::App;

pub fn render_dropdown(f: &mut Frame, area: Rect, app: &App) {
    // Check if we're showing history or file paths
    let (item_count, title) = if !app.history_items.is_empty() {
        (app.history_items.len(), " History ")
    } else if !app.dropdown_items.is_empty() {
        (app.dropdown_items.len(), " Tab Completion ")
    } else {
        return;
    };

    // Calculate dropdown position (below current field)
    let field_offset = app.edit_field_index * 4; // Each field takes ~4 lines
    let dropdown_y = area.y + field_offset as u16 + 4;
    let dropdown_height = std::cmp::min(10, item_count as u16 + 2);

    let dropdown_area = Rect {
        x: area.x + 2,
        y: std::cmp::min(dropdown_y, area.bottom().saturating_sub(dropdown_height)),
        width: area.width.saturating_sub(4),
        height: dropdown_height,
    };

    let items: Vec<ListItem> = if !app.history_items.is_empty() {
        // History dropdown
        app.history_items
            .iter()
            .enumerate()
            .map(|(i, value)| {
                let is_selected = i == app.dropdown_selected;

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(value, style),
                ]))
            })
            .collect()
    } else {
        // File path dropdown
        app.dropdown_items
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let is_selected = i == app.dropdown_selected;
                let is_dir = path.is_dir();

                // Check if this is the parent directory (first item is always parent)
                let is_parent = i == 0;

                let name = if is_parent {
                    "../".to_string()
                } else {
                    let base_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    if is_dir {
                        format!("{}/", base_name)
                    } else {
                        base_name.to_string()
                    }
                };

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else if is_dir {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(Line::from(Span::styled(name, style)))
            })
            .collect()
    };

    // Calculate scroll offset to keep selected item visible
    let visible_items = (dropdown_height.saturating_sub(2)) as usize; // Subtract 2 for borders
    let scroll_offset = if app.dropdown_selected >= visible_items {
        app.dropdown_selected.saturating_sub(visible_items - 1)
    } else {
        0
    };

    // Only show items in the visible window
    let visible_items: Vec<ListItem> = items
        .into_iter()
        .skip(scroll_offset)
        .take(visible_items)
        .collect();

    let list = List::new(visible_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(title)
            .style(Style::default().bg(Color::DarkGray)),
    );

    f.render_widget(ratatui::widgets::Clear, dropdown_area);
    f.render_widget(list, dropdown_area);
}

pub fn render_file_browser(f: &mut Frame, area: Rect, app: &App) {
    // Create centered overlay
    let popup_area = centered_rect(80, 80, area);

    // Filter items by fuzzy search
    let matcher = SkimMatcherV2::default();
    let filtered_items: Vec<(usize, &PathBuf)> = if app.file_browser_search.is_empty() {
        app.file_browser_items.iter().enumerate().collect()
    } else {
        app.file_browser_items
            .iter()
            .enumerate()
            .filter(|(_, path)| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|name| matcher.fuzzy_match(name, &app.file_browser_search))
                    .is_some()
            })
            .collect()
    };

    let items: Vec<ListItem> = filtered_items
        .iter()
        .enumerate()
        .map(|(display_idx, (original_idx, path))| {
            let is_selected = *original_idx == app.file_browser_selected;
            let is_dir = path.is_dir();

            let icon = if is_dir { "📁" } else { "📄" };
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("..");

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if is_dir {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::raw(if is_selected { "▶ " } else { "  " }),
                Span::raw(format!("{} ", icon)),
                Span::styled(name, style),
            ]))
        })
        .collect();

    let title = if app.file_browser_search.is_empty() {
        format!(" File Browser: {} ", app.current_dir.display())
    } else {
        format!(" File Browser [search: {}] ", app.file_browser_search)
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(list, popup_area);
}

/// Helper to create a centered rect
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
