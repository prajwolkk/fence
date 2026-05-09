use std::error::Error;
use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

pub fn run_browse() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let result = browse_loop(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn browse_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), Box<dyn Error>> {
    let entries = fence::read_log_entries()?;
    let mut list_state = ListState::default();
    if !entries.is_empty() {
        list_state.select(Some(0));
    }
    let mut detail_focus = false;
    let mut hide_superseded = false;
    let log_status = fence::tracking_status_for_log();
    let md_status = fence::tracking_status_for_markdown();

    loop {
        let visible = visible_indices(&entries, hide_superseded);
        clamp_selection(&visible, &mut list_state);
        let render_state = BrowseRenderState {
            detail_focus,
            hide_superseded,
            log_status,
            md_status,
        };
        terminal.draw(|frame| {
            draw_browse_ui(frame, &entries, &visible, &mut list_state, &render_state)
        })?;

        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Down | KeyCode::Char('j') => {
                    move_selection(1, visible.len(), &mut list_state);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    move_selection(-1, visible.len(), &mut list_state);
                }
                KeyCode::Enter => {
                    detail_focus = !detail_focus;
                }
                KeyCode::Char('h') => {
                    hide_superseded = !hide_superseded;
                }
                KeyCode::Char('r') => {
                    jump_to_replacement(&entries, &visible, &mut list_state);
                }
                _ => {}
            }
        }
    }

    Ok(())
}

struct BrowseRenderState {
    detail_focus: bool,
    hide_superseded: bool,
    log_status: fence::TrackingStatus,
    md_status: fence::TrackingStatus,
}

fn draw_browse_ui(
    frame: &mut Frame,
    entries: &[fence::Decision],
    visible: &[usize],
    list_state: &mut ListState,
    state: &BrowseRenderState,
) {
    let area = frame.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if state.detail_focus {
            [Constraint::Percentage(25), Constraint::Percentage(75)]
        } else {
            [Constraint::Percentage(40), Constraint::Percentage(60)]
        })
        .split(layout[0]);

    let list_block = Block::default().borders(Borders::ALL).title("Decisions");
    let detail_block = Block::default().borders(Borders::ALL).title("Details");

    if entries.is_empty() || visible.is_empty() {
        let empty_message = Paragraph::new("No decisions yet. Run `fence log` to create one.")
            .block(list_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(empty_message, body[0]);

        let detail_copy = if entries.is_empty() {
            "Select a decision to view details.".to_string()
        } else {
            "No visible decisions. Press `h` to show superseded entries again.".to_string()
        };
        let detail_message = Paragraph::new(detail_copy)
            .block(detail_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(detail_message, body[1]);
    } else {
        let items: Vec<ListItem> = visible
            .iter()
            .map(|index| {
                let entry = &entries[*index];
                let status_style = status_style(entry);
                let indicator = status_indicator(entry);
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{indicator} "),
                        status_style.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{} ", entry_date(entry)),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(entry_title(entry), status_style),
                ]))
            })
            .collect();
        let list = List::new(items)
            .block(list_block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_stateful_widget(list, body[0], list_state);

        let detail_message = Paragraph::new(detail_text(entries, visible, list_state))
            .block(detail_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(detail_message, body[1]);
    }

    let help = Paragraph::new(format!(
        "q: quit  j/k: navigate  enter: toggle detail  h: hide superseded ({})  r: open replacement  [Log: {}] [MD: {}]",
        if state.hide_superseded { "on" } else { "off" },
        tracking_label(state.log_status),
        tracking_label(state.md_status)
    ))
    .style(Style::default().fg(Color::Gray))
    .alignment(Alignment::Center);
    frame.render_widget(help, layout[1]);
}

fn entry_date(entry: &fence::Decision) -> &str {
    entry
        .timestamp
        .split_whitespace()
        .next()
        .unwrap_or(&entry.timestamp)
}

fn entry_title(entry: &fence::Decision) -> String {
    let title = entry.message.lines().next().unwrap_or("").trim();
    let mut clipped = String::new();
    for (count, ch) in title.chars().enumerate() {
        if count >= 40 {
            clipped.push_str("...");
            break;
        }
        clipped.push(ch);
    }
    if clipped.is_empty() {
        "<untitled>".to_string()
    } else {
        clipped
    }
}

fn visible_indices(entries: &[fence::Decision], hide_superseded: bool) -> Vec<usize> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| !hide_superseded || entry.status != fence::DecisionStatus::Superseded)
        .map(|(index, _)| index)
        .collect()
}

fn tracking_label(status: fence::TrackingStatus) -> &'static str {
    match status {
        fence::TrackingStatus::Tracked => "Tracked",
        fence::TrackingStatus::Local => "Local",
    }
}

fn clamp_selection(visible: &[usize], list_state: &mut ListState) {
    if visible.is_empty() {
        list_state.select(None);
        return;
    }

    let current = list_state.selected().unwrap_or(0);
    let clamped = current.min(visible.len().saturating_sub(1));
    list_state.select(Some(clamped));
}

fn move_selection(delta: isize, visible_len: usize, list_state: &mut ListState) {
    if visible_len == 0 {
        list_state.select(None);
        return;
    }

    let current = list_state.selected().unwrap_or(0) as isize;
    let next = (current + delta).clamp(0, visible_len.saturating_sub(1) as isize);
    list_state.select(Some(next as usize));
}

fn jump_to_replacement(entries: &[fence::Decision], visible: &[usize], list_state: &mut ListState) {
    let Some(selected) = list_state.selected() else {
        return;
    };
    let Some(entry_index) = visible.get(selected) else {
        return;
    };
    let Some(replacement_id) = entries
        .get(*entry_index)
        .and_then(|entry| entry.superseded_by.as_deref())
    else {
        return;
    };

    if let Some(next_visible_index) = visible.iter().position(|index| {
        entries
            .get(*index)
            .map(|entry| entry.id == replacement_id)
            .unwrap_or(false)
    }) {
        list_state.select(Some(next_visible_index));
    }
}

fn detail_text(entries: &[fence::Decision], visible: &[usize], list_state: &ListState) -> String {
    let Some(index) = list_state.selected() else {
        return "Select a decision to view details.".to_string();
    };
    let Some(entry_index) = visible.get(index) else {
        return "Select a decision to view details.".to_string();
    };
    let Some(entry) = entries.get(*entry_index) else {
        return "Select a decision to view details.".to_string();
    };

    let tags = if entry.optional_tags.is_empty() {
        "Tags: -".to_string()
    } else {
        format!("Tags: {}", entry.optional_tags.join(", "))
    };

    let review = format!("Review Due: {}", entry.review_due);
    let status = format!("Status: {}", fence::decision_status_label(entry));
    let lifecycle_link = match entry.superseded_by.as_deref() {
        Some(replacement_id) => format!("View Replacement: {replacement_id} (press r)"),
        None => match entry.supersedes.as_deref() {
            Some(previous_id) => format!("Supersedes: {previous_id}"),
            None => "Lifecycle Link: -".to_string(),
        },
    };

    format!(
        "Category: {} {}\nAuthor: {}\nTimestamp: {}\n{}\n{}\n{}\n{}\n\n{}",
        category_icon(entry.category),
        fence::decision_category_label(entry.category),
        entry.author,
        entry.timestamp,
        status,
        review,
        tags,
        lifecycle_link,
        entry.message
    )
}

fn status_indicator(entry: &fence::Decision) -> &'static str {
    match entry.status {
        fence::DecisionStatus::Deprecated => "[x]",
        fence::DecisionStatus::Superseded => "[s]",
        fence::DecisionStatus::Accepted if fence::is_stale(entry) => "[!]",
        _ => "[*]",
    }
}

fn status_style(entry: &fence::Decision) -> Style {
    match entry.status {
        fence::DecisionStatus::Deprecated => Style::default().fg(Color::Red),
        fence::DecisionStatus::Superseded => Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
        fence::DecisionStatus::Accepted if fence::is_stale(entry) => {
            Style::default().fg(Color::Yellow)
        }
        _ => Style::default().fg(Color::Green),
    }
}

fn category_icon(category: fence::DecisionCategory) -> &'static str {
    match category {
        fence::DecisionCategory::Architecture => "ARCH",
        fence::DecisionCategory::Technical => "TECH",
        fence::DecisionCategory::Product => "PROD",
        fence::DecisionCategory::Security => "SEC",
        fence::DecisionCategory::General => "GEN",
    }
}
